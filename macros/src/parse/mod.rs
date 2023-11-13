use std::iter;

use proc_macro2::{TokenStream, TokenTree, Delimiter};
use litrs::StringLit;

use crate::{ast, err::{Error, err}};


mod buf;

use self::buf::ParseBuf;



pub(crate) trait Parse {
    fn parse(buf: &mut ParseBuf) -> Result<Self, Error>
    where
        Self: Sized;
}

impl ast::Input {
    pub(crate) fn parse_input(tokens: TokenStream) -> Result<Self, Error> {
        let mut buf = ParseBuf::from_stream(tokens);
        let out = ast::Input::parse(&mut buf)?;
        buf.expect_eof()?;
        Ok(out)
    }
}

fn is_punct(tt: &TokenTree, c: char) -> bool {
    matches!(tt, TokenTree::Punct(p) if p.as_char() == c)
}

impl Parse for ast::Input {
    fn parse(buf: &mut ParseBuf) -> Result<Self, Error> {
        let mut buffer = None;
        let mut format = None;

        loop {
            match buf.curr()? {
                // A meta attribute
                TokenTree::Punct(p) if p.as_char() == '#' => {
                    let _ = buf.expect_punct('#')?;
                    let g = buf.expect_group(Delimiter::Bracket)?;
                    let mut inner = ParseBuf::from_group(g);
                    let key = inner.expect_ident()?;
                    match key.to_string().as_str() {
                        "format" => {
                            let _ = inner.expect_punct('=')?;
                            let expr = TokenStream::from_iter(iter::from_fn(|| inner.bump().ok()));
                            format = Some(expr);
                        }
                        other => return Err(err!(
                            @key.span(),
                            "unsupported global attribute '{other}'",
                        )),
                    }
                }

                // The XML portion starts
                TokenTree::Punct(p) if p.as_char() == '<' => break,

                // Something else which we treat as an expression defining the
                // buffer to append to.
                _ => {
                    let mut tokens = vec![];
                    while buf.curr().is_ok_and(|tt| !is_punct(tt, ',')) {
                        tokens.push(buf.bump()?);
                    }
                    let _ = buf.bump()?; // Eat `,`
                    buffer = Some(TokenStream::from_iter(tokens));
                }
            }
        }

        // At this point, the XML part starts
        let prolog = if is_punct(buf.next()?, '?') {
            Some(buf.parse()?)
        } else {
            None
        };

        buf.expect_punct('<')?;
        let root = buf.parse()?;

        Ok(Self {
            buffer,
            format,
            prolog,
            root,
        })
    }
}

// Assumes `<` is already eaten.
impl Parse for ast::Prolog {
    fn parse(buf: &mut ParseBuf) -> Result<Self, Error> {
        let _ = buf.bump(); // Eat '<'
        let _ = buf.bump(); // Eat '?'
        let ident = buf.expect_ident()?;
        if ident.to_string() != "xml" {
            return Err(err!(@ident.span(), "expected 'xml'"));
        }

        let next_span = buf.curr().unwrap().span();
        let mut parse_attr = |name: &str| -> Result<Option<String>, Error> {
            if is_punct(buf.curr()?, '?') {
                return Ok(None);
            }
            let ident = buf.expect_ident()?;
            if ident.to_string() != name {
                return Err(err!(
                    @ident.span(),
                    "expected '{name}' (prolog attributes have a fixed order)",
                ));
            }

            buf.expect_punct('=')?;
            let s = buf.expect_string_lit()?;
            Ok(Some(s.into_value().into_owned()))
        };

        let version = parse_attr("version")?.ok_or_else(|| err!(@next_span, "expected 'xml'"))?;
        let encoding = parse_attr("encoding")?;
        let standalone = if encoding.is_some() {
            parse_attr("standalone")?
        } else {
            None
        };

        buf.expect_punct('?')?;
        buf.expect_punct('>')?;

        if encoding.as_ref().is_some_and(|enc| enc != "UTF-8") {
            // TODO: span would be nice
            return Err(err!("only encoding 'UTF-8' is allowed"));
        };

        Ok(Self { version, standalone })
    }
}

// Assumes `<` is already eaten.
impl Parse for ast::Element {
    fn parse(buf: &mut ParseBuf) -> Result<Self, Error> {
        let name = buf.parse()?;
        let mut attrs = Vec::new();
        loop {
            match buf.curr()? {
                TokenTree::Punct(p) if p.as_char() == '>' => {
                    let _ = buf.bump();
                    break;
                }
                TokenTree::Punct(p) if p.as_char() == '/' => {
                    let _ = buf.bump();
                    buf.expect_punct('>')?;
                    return Ok(Self {
                        name,
                        attrs,
                        children: vec![],
                        empty: true,
                    })
                }
                _ => {
                    let name = buf.parse()?;
                    buf.expect_punct('=')?;
                    let value = buf.parse()?;
                    attrs.push((name, value));
                }
            }
        }

        let mut children = vec![];
        while !(is_punct(buf.curr()?, '<') && is_punct(buf.next()?, '/')) {
            children.push(buf.parse()?);
        }

        let end_span = buf.expect_punct('<')?.span();
        buf.expect_punct('/')?;
        if is_punct(buf.curr()?, '>') {
            let _ = buf.bump();
        } else {
            let ending_name: ast::Name = buf.parse()?;
            if ending_name.0 != name.0 {
                return Err(err!(@end_span,
                    "end tag '{}' does not match start tag '{}'",
                    ending_name.0,
                    name.0,
                ));
            }
            buf.expect_punct('>')?;
        }


        Ok(Self { name, attrs, children, empty: false })
    }
}

impl Parse for ast::Name {
    fn parse(buf: &mut ParseBuf) -> Result<Self, Error> {
        // It is impossible for us to exactly parse XML names, unfortunately:
        //
        // For one, some characters allowed in XML names are not part of
        // XID_Continue (and thus not allowed in Rust idents) and are also not
        // allowed as punctuation. In other words: those characters just aren't
        // part of the Rust lexicographical grammar outside of string
        // literals.
        //
        // Further, from Rusts lexicographical grammar and proc macro
        // perspective, the following inputs are all the same:
        // - <foo:bar: baz="3">
        // - <foo:bar :baz="3">
        // - <foo: bar:baz="3">
        //
        // So since we have no knowledge about "spaces" between tokens, we have
        // to use some custom logic to decide whether additional tokens
        // are "eaten", i.e. being considered part of the XML name. This
        // depends on the previous token kind that was eaten. The first token
        // is always part of the XML name. We distinguish three kinds of tokens:
        // - ident
        // - num_lit: float or integer lit
        // - :-. punctuation
        //
        // These three kinds are enough to cover all valid XML names, ignoring
        // the characters mentioned above. A single `_` is actually parsed as
        // ident.
        //
        // These are the rules to eat the next token or not, depending on the
        // previous one:
        //
        // :-.  :-.          -> eat [^1]
        // :-.  ident        -> eat: XML names can start with `:-.` and an ident
        //                           could start a new XML name -> main problem.
        // :-.  num_lit      -> eat: a num lit always starts with 0-9, but an
        //                           XML name cannot start like that.
        // ident  :-.        -> eat [^1]
        // ident  ident      -> stop: there must be a space between as otherwise
        //                            it would be parsed as one ident.
        // ident  num_lit    -> stop: num lit starts with 0-9 but that would
        //                            still be part of the ident before.
        // num_lit  :-.      -> eat [^1]
        // num_lit  ident    -> stop: ident would be part of literal suffix
        //                            -> thus there is a space.
        // num_lit  num_lit  -> stop: as far as I can tell, this also can only
        //                            happen if there is a space.
        //
        // [^1]: An XML name is allowed to start with `:`. By using this rule,
        // we make it impossible to use a leading `:` in an XML name that
        // follows another XML name.
        //
        // So in summary, we stop when a non-punct follows a non-punct.
        let mut eat_non_punct = true;

        // Because of all the weirdness explained above, we allow a single
        // string literal to define the name.
        if let Ok(lit) = StringLit::try_from(buf.curr()?) {
            let token = buf.bump().unwrap();
            let s = lit.into_value().into_owned();
            if !is_name(&s) {
                return Err(err!(@token.span(),
                    "string contains characters that are not allowed in XML names",
                ));
            }
            return Ok(Self(s));
        }

        let mut out = String::new();
        loop {
            match buf.curr() {
                // Identifier including _
                Ok(TokenTree::Ident(i)) => {
                    if !eat_non_punct {
                        break;
                    }
                    eat_non_punct = false;
                    let s = i.to_string();

                    // There are four characters that are allowed as part of
                    // Rust literals but that is not allowed in XML names.
                    let invalid_char = |c| matches!(c, '\u{AA}' | '\u{B5}' | '\u{BA}' | '\u{2054}');
                    if let Some(p) = s.find(invalid_char) {
                        return Err(err!(
                            "Character '{}' is not allowed in XML names",
                            s[p..].chars().next().unwrap(),
                        ));
                    }

                    out.push_str(&s);
                    let _ = buf.bump();
                }

                // Numeric literals
                Ok(TokenTree::Literal(lit)) => {
                    let s = lit.to_string();
                    if s.starts_with(|c: char| c.is_digit(10)) {
                        let _ = buf.bump();
                        out.push_str(&s);
                    } else {
                        break;
                    }
                }

                // : . -
                Ok(TokenTree::Punct(p)) => {
                    let c = p.as_char();
                    if c == ':' || (!out.is_empty() && (c == '.' || c == '-')) {
                        eat_non_punct = true;
                        let _ = buf.bump();
                        out.push(c);
                    } else {
                        break;
                    }
                }

                _ => break,
            }
        }

        if out.is_empty() {
            let unexpected = buf.curr().unwrap();
            return Err(err!(@unexpected.span(), "expected name, found {unexpected}"));
        }

        Ok(Self(out))
    }
}

impl Parse for ast::Child {
    fn parse(buf: &mut ParseBuf) -> Result<Self, Error> {
        match buf.bump()? {
            TokenTree::Literal(l) => {
                let slit = StringLit::try_from(&l)
                    .map_err(|_| err!(@l.span(), "expected string literal"))?;

                let v = slit.into_value().into_owned();
                Ok(Self::Text(v))
            }
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                let inner = g.stream();
                if inner.clone().into_iter().next().is_some_and(|tt| is_punct(&tt, '|')) {
                    let mut inner = ParseBuf::from_group(g);
                    let _ = inner.expect_punct('|')?;
                    let arg = inner.expect_ident()?;
                    let _ = inner.expect_punct('|')?;
                    let body = TokenStream::from_iter(std::iter::from_fn(|| inner.bump().ok()));
                    Ok(Self::Closure { arg, body })
                } else {
                    Ok(Self::TextExpr(inner))
                }
            }
            TokenTree::Punct(p) if p.as_char() == '<' => {
                Ok(Self::Element(buf.parse()?))
            }
            other => Err(err!(
                @other.span(),
                "expected element child: string literal, {{...}} or '<'",
            )),
        }
    }
}

impl Parse for ast::AttrValue {
    fn parse(buf: &mut ParseBuf) -> Result<Self, Error> {
        match buf.bump()? {
            TokenTree::Literal(l) => {
                let slit = StringLit::try_from(&l)
                    .map_err(|_| err!(@l.span(), "expected string literal"))?;

                let v = slit.into_value().into_owned();
                Ok(Self::Literal(v))
            }
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                Ok(Self::Expr(g.stream()))
            }
            other => Err(err!(
                @other.span(),
                "expected attribute value: string literal or {{...}}",
            )),
        }
    }
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
