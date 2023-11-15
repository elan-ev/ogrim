//! XML builder macro letting you write XML inside Rust code (similar to `serde_json::json!`).
//!
//! This library only builds a string, not some kind of tree representing the
//! XML document. Thus, you cannot introspect it after building. So this is
//! just a better `format!` for building XML.
//!
//! There are no memory allocations in this library except by the `String` that
//! is being built. Not even temporarily, not even for escaping values. This
//! should make it quite speedy and at least as fast as hand written string
//! building.
//!
//!
//! # Mini example
//!
//! ```rust
//! use ogrim::xml;
//!
//! let cat_name = "Tony";
//! let doc = xml!(
//!     <?xml version="1.0" ?>
//!     <zoo name="Lorem Ipsum" openingYear={2000 + 13}>
//!         <cat>{cat_name}</cat>
//!         <dog>"Barbara"</>   // omit name of closing tab for convenience
//!     </zoo>
//! );
//!
//! println!("{}", doc.as_str()); // Print XML
//! ```
//!
//! As you can see, the name of the closing tab can be omitted. This should help
//! a little bit with XML's verbosity. For more information and examples, see
//! [`xml`].
//!
//! Of course, values are escaped:
//!
//! ```rust
//! let doc = ogrim::xml!(
//!     <?xml version="1.0" ?>
//!     <foo name=r#"a"b<c&d"#>
//!         "Little Bobby </foo> Tables"
//!     </foo>
//! );
//!
//! assert_eq!(doc.as_str(), concat!(
//!     r#"<?xml version="1.0" encoding="UTF-8"?>"#,
//!     r#"<foo name="a&quot;b&lt;c&amp;d">Little Bobby &lt;/foo&gt; Tables</foo>"#,
//! ));
//! ```
//!
//!
//! # Limitations and notes
//!
//! - This crate only lets you build `UTF-8` encoded XML documents.
//! - Text content of nodes has to be quoted (e.g. `<foo>"hello"</foo>` instead
//!   of `<foo>hello</foo>`).
//! - Writing *names* (i.e. tag and attribute names) has some special cases. In
//!   short: if you only use valid ASCII Rust identifiers with `:` in the middle
//!   of the name (e.g. `atom:link`), you are fine. In other cases, see below or
//!   just use a string literal: `<"weird3.14exml-name:" />`.
//!
//!   <details>
//!     <summary>The gory details</summary>
//!
//!     First, talking about characters beyond ASCII, XML names allow some chars
//!     that Rust identifiers do not allow. Those are just not part of the Rust
//!     lexicographical grammar and hence, using a string literal is necessary
//!     in that case. But you likely won't run into this. For completeness,
//!     [here are all characters][1] you could legally write in XML names, but
//!     not in Rust identifiers.
//!
//!     Further, `- : .` are all not part of Rust identifier, but instead
//!     treated by Rust as "puncuation". And Rust macros have no information
//!     about whitespace at all, so these three inputs are the same:
//!     - `<foo:bar: baz="3">`
//!     - `<foo:bar :baz="3">`
//!     - `<foo: bar:baz="3">`
//!
//!     This library uses some best effort guesses to disambiguate this. If you
//!     don't use `- : .` at the end of an XML name it should work fine.
//!     Finally, due to these characters being treated as punctuation, digits
//!     after these puncuations are parsed as numeric literals, which brings a
//!     whole new bag of weird behavior. For example, `foo:27eels` fails to
//!     parse as `27e` is parsed as a floating point literal with exponent...
//!     but the actual exponent is missing.
//!
//!     Again: for most normal names, everything should just work. For
//!     everything else, know these rules or just use a string literal
//!     instead.
//!
//!   </details>
//!
//! [1]: https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=%5B%5BA-Z_%3A%5C-.a-z0-9%5Cu00B7%5Cu00C0-%5Cu00D6%5Cu00D8-%5Cu00F6%5Cu00F8-%5Cu036F%5Cu0370-%5Cu037D%5Cu037F-%5Cu1FFF%5Cu200C-%5Cu200D%5Cu203F-%5Cu2040%5Cu2070-%5Cu218F%5Cu2C00-%5Cu2FEF%5Cu3001-%5CuD7FF%5CuF900-%5CuFDCF%5CuFDF0-%5CuFFFD%5CU00010000-%5CU000EFFFF%5D-%5B%3AXID_Continue%3A%5D%5D&esc=on&g=&i=

use core::fmt;
use std::{fmt::Write, matches, unreachable};



/// Builds or appends to an XML [`Document`] by writing XML in your Rust code.
///
///
/// # Basic example
///
/// ```rust
/// use ogrim::xml;
///
/// let cat_name = "Felix";
/// let doc = xml!(
///     <?xml version="1.0" ?>
///     // Rust comments are still just comments here
///     <guests
///         event="Relaxing dinner"   // attribute with literal value
///         limit={3 * 10}            // attribute with interpolated Rust expression
///     >
///         // Text content of elements has to be quoted, i.e. string literals.
///         <guest age="65">"Peter Lustig"</>
///         <guest age="3">{cat_name}</> // Interpolated content from variable
///     </guests>
/// );
///
/// println!("{}", doc.as_str()); // Print XML
/// ```
///
///
/// # Interpolations
///
/// You can interpolate variables and other Rust expressions in the XML literal,
/// as shown in the previous example. Writing `{...}` in place of an attribute
/// value or a element's content treats the inner part as Rust expression,
/// which must evaluate to something that implements [`fmt::Display`].
///
/// There is one special form of interpolations that look like a closure:
/// `{|doc| ...}`. As if this were a closure, your code can access `doc` which
/// is the partial [`Document`] at that point in the build process. But as this
/// is not actually implemented as a closure, you are still in the context of
/// your outer function and can use `.await` or `?` as appropriate. See below
/// for a useful example.
///
/// ## Fill attribute syntax `{..iter}`
///
/// You can dynamically add attributes to an element by using the `<foo
/// {..iter}>` syntax. There, `iter` must be an expression that implements
/// `IntoIterator<Item = (N, V)>` where `N` and `V` must implement
/// `fmt::Display`. This allows you to interpolate hash maps or lists,
/// and also to easily model optional attributes (as `Option` does implement
/// `IntoIterator`). Examples:
///
/// ```rust
/// use std::{collections::BTreeMap, path::Path};
/// use ogrim::xml;
///
/// let description = Some("Lorem Ipsum");
/// let map = BTreeMap::from([
///     ("cat", Path::new("/usr/bin/cat").display()),
///     ("dog", Path::new("/home/goodboy/image.jpg").display()),
/// ]);
///
/// let doc = xml!(
///     <?xml version="1.1" ?>
///     <root
///         // Optional attributes via `Option`
///         {..description.map(|v| ("description", v))}
///         // Can be mixed with normal attributes, retaining source order
///         bar="green"
///         // Naturally, maps work as well. Remember that hash map has a random
///         // iteration order, so consider using `BTreeMap` instead.
///         {..map}
///         // Arrays can also be useful, as they also implement `IntoIterator`
///         {..["Alice", "Bob"].map(|name| (name.to_lowercase(), "invited"))}
///     >
///     </>
/// );
///
/// # assert_eq!(doc.as_str(), concat!(
/// #     r#"<?xml version="1.1" encoding="UTF-8"?>"#,
/// #     r#"<root description="Lorem Ipsum" bar="green" "#,
/// #     r#"cat="/usr/bin/cat" dog="/home/goodboy/image.jpg" "#,
/// #     r#"alice="invited" bob="invited"></root>"#,
/// # ));
/// ```
///
/// Note: as the attribute names cannot be checked at compile time, the check
/// has to be performed at runtime. If passed invalid XML names, this will
/// panic.
///
///
/// # Create new document (entry point)
///
/// To create a new document, simply do *not* pass an existing one as first
/// argument. In that case, the macro returns a `Document`.
///
/// ```rust
/// use ogrim::{xml, Format};
///
/// let doc = xml!(
///     // Optional: specify meta/formatting attributes
///     #[format = Format::Pretty { indentation: "  " }]
///     <?xml version="1.0" encoding="UTF-8" ?>   // XML prolog
///     <foo bar="baz">    // root element
///         // ...
///     </foo>
/// );
///
/// println!("{}", doc.as_str()); // Print XML
/// ```
///
/// Currently the only supported meta attribute is `format`. See [`Format`].
///
/// The XML prolog is required. Specifying `encoding` is optional and if
/// specified, must be `"UTF-8"`.
///
///
/// # Append to existing document & split up logic
///
/// Just specify the document as first argument, like `write!`. It has to be of
/// type `&mut Document`. In that case, the macro evaluates to `()` and just
/// appends to the given document.
///
/// ```rust
/// use ogrim::xml;
///
/// fn foo(doc: &mut ogrim::Document) {
///     xml!(doc, <foo>"Peter"</foo>);
/// }
/// ```
///
/// However, since each `xml` invocation has balanced tags, i.e. every opened
/// element must also be closed, you might be wondering how to make use of
/// this. For that, you can use the special interpolation form `{|doc| ...}`.
/// This can be used wherever an element's child is expected. Example:
///
/// ```rust
/// use ogrim::xml;
///
/// let doc = xml!(
///     <?xml version="1.1" ?>
///     <items>
///         {|doc| make_items(doc)}
///     </items>
/// );
///
///
/// fn make_items(doc: &mut ogrim::Document) {
///     let data = ["foo", "bar", "baz"];
///     for s in data {
///         xml!(doc, <item length="3">{s}</item>);
///     }
/// }
/// ```
///
/// The `make_items` function receives an incomplete XML document that you can
/// append to. While the syntax seems to imply the usage of a closure, that's
/// just syntax. `make_items` could be async and return a `Result` and you
/// could write `{|doc| make_items(doc).await?}` as long as the outer function
/// is also async and returns `Result`.
///
/// This also allows you to model optional elements:
///
/// ```rust
/// use ogrim::xml;
///
/// let some_condition = true;
/// let doc = xml!(
///     <?xml version="1.1" ?>
///     <foo>
///         {|doc| if some_condition {
///             xml!(doc, <bar />);
///         }}
///     </>
/// );
/// ```
///
pub use ogrim_macros::xml;




/// A document, potentially still under construction.
///
/// This is basically just a `String` inside. The only way to create a value of
/// this type is by using [`xml!`]. That macro can also append to an existing
/// `Document`. The only thing you can do on a document is get the string out
/// of it.
pub struct Document {
    buf: String,
    depth: u32,
    format: Format,
}

/// Just a wrapper around `write!().unwrap()` as writing to a string cannot fail.
macro_rules! wr {
    ($($t:tt)*) => {
        write!($($t)*).unwrap()
    };
}

impl Document {
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    pub fn into_string(self) -> String {
        self.buf
    }

    // ----- Private -----

    #[doc(hidden)]
    pub fn new(version: Version, standalone: Option<bool>, format: Format) -> Self {
        let version = match version {
            Version::V1_0 => "1.0",
            Version::V1_1 => "1.1",
        };

        // The XML prolog with encoding is 38 bytes long. There will very
        // likely be added more to the string, so 64 seems like a good starting
        // point.
        let mut buf = String::with_capacity(64);
        wr!(buf, r#"<?xml version="{version}" encoding="UTF-8""#);
        if let Some(standalone) = standalone {
            wr!(buf, " standalone={}", if standalone { "yes" } else { "no" });
        }
        wr!(buf, "?>");

        let mut out = Self { buf, format, depth: 0 };
        out.newline();
        out
    }


    #[doc(hidden)]
    pub fn open_tag(&mut self, name: &str) {
        wr!(self.buf, "<{name}");
    }

    #[doc(hidden)]
    pub fn attr(&mut self, name: &str, value: &dyn fmt::Display) {
        wr!(self.buf, r#" {name}=""#);
        escape_into(&mut self.buf, value, true);
        self.buf.push('"');
    }

    #[doc(hidden)]
    pub fn attrs<I, N, V>(&mut self, attrs: I)
    where
        I: IntoIterator<Item = (N, V)>,
        V: fmt::Display,
        N: fmt::Display,
    {
        for (name, value) in attrs {
            // To check whether the name is valid, we first just write it to the
            // buffer to avoid temporary heap allocations.
            let len_before = self.buf.len();
            wr!(self.buf, r#" {name}=""#);
            let written_name = &self.buf[len_before + 1..self.buf.len() - 2];
            if !is_name(written_name) {
                panic!("attribute name '{written_name}' is not a valid XML name");
            }

            escape_into(&mut self.buf, &value, true);
            self.buf.push('"');
        }
    }

    #[doc(hidden)]
    pub fn close_start_tag(&mut self) {
        self.buf.push('>');
        self.depth += 1;
        self.newline();
    }

    #[doc(hidden)]
    pub fn close_empty_elem_tag(&mut self) {
        self.buf.push_str(if matches!(self.format, Format::Terse) { "/>" } else { " />" });
        self.newline();
    }

    #[doc(hidden)]
    pub fn end_tag(&mut self, name: &str) {
        assert!(self.depth > 0);

        if let Format::Pretty { indentation } = self.format {
            assert!(self.buf.ends_with(indentation));
            self.buf.truncate(self.buf.len() - indentation.len());
        }
        self.depth -= 1;
        wr!(self.buf, "</{name}>");
        self.newline();
    }

    #[doc(hidden)]
    pub fn text(&mut self, text: &dyn fmt::Display) {
        escape_into(&mut self.buf, text, false);
        self.newline();
    }

    /// Appends a newline and proper indentation according to `self.depth` to
    /// the buffer.
    fn newline(&mut self) {
        if let Format::Pretty { indentation } = self.format {
            self.buf.reserve(1 + indentation.len() * self.depth as usize);
            self.buf.push('\n');
            for _ in 0..self.depth {
                self.buf.push_str(indentation);
            }
        }
    }
}


#[doc(hidden)]
pub enum Version {
    V1_0,
    V1_1,
}

/// Specifies how the XML should be formatted.
///
/// Pass to [`xml`] like this:
///
/// ```
/// use ogrim::{xml, Format};
///
/// let doc = xml!(
///     #[format = Format::Pretty { indentation: "  " }]
///     <?xml version="1.0" ?>
///     <foo></>
/// );
/// ```
///
/// After `format = ` you can pass any Rust expression, also referencing
/// variables, for example to make formatting conditional. If not specified,
/// terse formatting is used.
pub enum Format {
    /// Minimized, as short as possible.
    Terse,

    /// Pretty printed for human consumption.
    Pretty {
        /// String with which to indent, e.g. `"  "`.
        indentation: &'static str,
    },
}


/// Writes the escaped `v` into `buf`. We do that without temporary heap
/// allocations via `EscapedWriter`, which is a layer between the
/// `fmt::Display` logic of `v` and our final buffer.
fn escape_into(buf: &mut String, v: &dyn fmt::Display, escape_quote: bool) {
    wr!(EscapedWriter { buf, escape_quote }, "{}", v);
}

struct EscapedWriter<'a> {
    buf: &'a mut String,
    escape_quote: bool,
}

impl fmt::Write for EscapedWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // We always use `"` to quote attribute values, so we don't need to
        // escape `'`. `>` does not necessarily need to be escaped, but it is
        // strongly recommended.
        let escape_quote = self.escape_quote;
        let needs_escape = |c: char| matches!(c, '<' | '>' | '&') || (escape_quote && c == '"');

        let mut remaining = s;
        while let Some(pos) = remaining.find(needs_escape) {
            self.buf.push_str(&remaining[..pos]);
            match remaining.as_bytes()[pos] {
                b'<' => self.buf.push_str("&lt;"),
                b'>' => self.buf.push_str("&gt;"),
                b'&' => self.buf.push_str("&amp;"),
                b'"' => self.buf.push_str("&quot;"),
                _ => unreachable!(),
            }
            remaining = &remaining[pos + 1..];
        }
        self.buf.push_str(remaining);
        Ok(())
    }
}

include!("shared.rs");
