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
        let mut indentation = None;

        loop {
            match buf.curr()? {
                // A meta attribute
                TokenTree::Punct(p) if p.as_char() == '#' => {
                    let _ = buf.expect_punct('#')?;
                    let g = buf.expect_group(Delimiter::Bracket)?;
                    let mut inner = ParseBuf::from_group(g);
                    let key = inner.expect_ident()?;
                    match key.to_string().as_str() {
                        "indentation" => {
                            let _ = inner.expect_punct('=')?;
                            let v = inner.expect_string_lit()?;
                            indentation = Some(v.into_value().into_owned());
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
            indentation,
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

        let ending_name: ast::Name = buf.parse()?;
        if ending_name.0 != name.0 {
            return Err(err!(@end_span, "end tag does not match start tag"));
        }
        buf.expect_punct('>')?;

        Ok(Self { name, attrs, children, empty: false })
    }
}

impl Parse for ast::Name {
    fn parse(buf: &mut ParseBuf) -> Result<Self, Error> {
        use std::fmt::Write;

        let mut out = String::new();
        let mut last_was_ident = false;
        loop {
            // TODO: Rust's definition of identifier is different from the XML
            // definition of "word". There are differences with non-ASCII
            // characters in particular that this code does not address.
            match buf.curr() {
                Ok(TokenTree::Ident(i)) => {
                    // If the last one was an ident then the only reason why it
                    // was lexed as two idents if there was whitespace between.
                    // So we will stop.
                    if last_was_ident {
                        break;
                    }
                    last_was_ident = true;
                    write!(out, "{i}").unwrap();
                    let _ = buf.bump();
                }
                Ok(TokenTree::Punct(p)) if p.as_char() == ':' => {
                    last_was_ident = false;
                    let _ = buf.bump();
                    out.push(':');
                }
                _ => break,
            }
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
