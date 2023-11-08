//! XML builder macro letting you write XML inside Rust code (similar to `serde_json::json!`).
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
//!         <dog>"Barbara"</dog>
//!     </zoo>
//! );
//!
//! println!("{}", doc.as_str()); // Print XML
//! ```
//!
//! For more information and examples, see [`xml`].
//!
//!
//! # Limitations
//!
//! - This crate only leds you build `UTF-8` encoded XML documents.
//! - ...
//!

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
///         <guest age="65">"Peter Lustig"</guest>
///         <guest age="3">{cat_name}</guest> // Interpolated content from variable
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
///
/// # Create new document (entry point)
///
/// To create a new document, simply do *not* pass an existing one as first
/// argument. In that case, the macro returns a `Document`.
///
/// ```rust
/// use ogrim::xml;
///
/// let doc = xml!(
///     #[indentation = "  "]   // Optional: specify meta/formatting attributes
///     <?xml version="1.0" encoding="UTF-8" ?>   // XML prolog
///     <foo bar="baz">    // root element
///         // ...
///     </foo>
/// );
///
/// println!("{}", doc.as_str()); // Print XML
/// ```
///
/// Currently the only supported meta attribute is `indentation`. If that's
/// specified, the XML is printed in pretty mode with the specified
/// indentation. If not specified, terse XML is output.
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
pub use ogrim_macros::xml;




/// A document, potentially still under construction.
///
/// This is little more than just a `String` inside. The only way to create a
/// value of this type is by using [`xml!`]. That macro can also append to an
/// existing `Document`. The only thing you can do on a document is get the
/// string out of it.
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
        assert!(is_name(name), "'{name}' is not a valid XML 'Name'");
        wr!(self.buf, "<{name}");
    }

    #[doc(hidden)]
    pub fn attr(&mut self, name: &str, value: &dyn fmt::Display) {
        assert!(is_name(name), "'{name}' is not a valid XML 'Name'");

        wr!(self.buf, r#" {name}=""#);
        escape_into(&mut self.buf, value, true);
        self.buf.push('"');
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
        assert!(is_name(name), "'{name}' is not a valid XML 'Name'");
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

#[doc(hidden)]
pub enum Format {
    Terse,
    Pretty {
        indentation: &'static str,
    },
}

fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_name_start_char(first) && chars.all(is_name_char)
}

fn is_name_start_char(c: char) -> bool {
    matches!(c,
        ':'
        | 'A'..='Z'
        | '_'
        | 'a'..='z'
        | '\u{C0}'..='\u{D6}'
        | '\u{D8}'..='\u{F6}'
        | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}'
        | '\u{37F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}'
        | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}'
    )
}

fn is_name_char(c: char) -> bool {
    is_name_start_char(c) || matches!(c,
        '-'
        | '.'
        | '0'..='9'
        | '\u{B7}'
        | '\u{0300}'..='\u{036F}'
        | '\u{203F}'..='\u{2040}'
    )
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
