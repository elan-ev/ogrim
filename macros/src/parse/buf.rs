use std::iter;

use proc_macro2::{
    token_stream::IntoIter, TokenStream, TokenTree, Span, Group, Punct, Ident, Delimiter,
};
use litrs::StringLit;

use crate::err::{Error, err};
use super::Parse;



pub(crate) struct ParseBuf {
    iter: IntoIter,
    curr: Option<TokenTree>,
    next: Option<TokenTree>,
    span: Option<Span>,
}

impl ParseBuf {
    pub(crate) fn from_stream(tokens: TokenStream) -> Self {
        Self::new_impl(tokens.into_iter(), None)
    }

    pub(crate) fn from_group(group: Group) -> Self {
        Self::new_impl(group.stream().into_iter(), Some(group.span()))
    }

    fn new_impl(mut iter: IntoIter, span: Option<Span>) -> Self {
        let curr = iter.next();
        let next = iter.next();
        Self { iter, curr, next, span }
    }

    /// Returns a reference to the current token.
    pub(crate) fn curr(&self) -> Result<&TokenTree, Error> {
        let e = self.eof_err();
        self.curr.as_ref().ok_or(e)
    }

    pub(crate) fn next(&self) -> Result<&TokenTree, Error> {
        let e = self.eof_err();
        self.next.as_ref().ok_or(e)
    }

    /// Advances one token, returning the current one by value.
    pub(crate) fn bump(&mut self) -> Result<TokenTree, Error> {
        let out = self.curr.take();
        self.curr = self.next.take();
        self.next = self.iter.next();
        out.ok_or_else(|| self.eof_err())
    }

    pub(crate) fn expect_punct(&mut self, c: char) -> Result<Punct, Error> {
        match self.bump()? {
            TokenTree::Punct(p) if p.as_char() == c => Ok(p),
            other => Err(Error {
                span: Some(other.span()),
                msg: format!("expected '{c}'"),
            }),
        }
    }

    pub(crate) fn expect_ident(&mut self) -> Result<Ident, Error> {
        match self.bump()? {
            TokenTree::Ident(i) => Ok(i),
            other => Err(Error {
                span: Some(other.span()),
                msg: format!("expected identifier"),
            }),
        }
    }

    pub(crate) fn expect_string_lit(&mut self) -> Result<StringLit<String>, Error> {
        let token = self.bump()?;
        StringLit::try_from(&token).map_err(|_| Error {
            span: Some(token.span()),
            msg: format!("expected string literal"),
        })
    }

    pub(crate) fn expect_group(&mut self, delim: Delimiter) -> Result<Group, Error> {
        match self.bump()? {
            TokenTree::Group(g) if g.delimiter() == delim => Ok(g),
            other => Err(err!(@other.span(), "expected {delim:?} delimited group")),
        }
    }

    pub(crate) fn expect_eof(&self) -> Result<(), Error> {
        if let Some(tt) = &self.curr {
            return Err(err!(@tt.span(), "unexpected extra token"));
        }
        Ok(())
    }

    pub(crate) fn eof_err(&self) -> Error {
        Error {
            span: self.span,
            msg: "unexpected end of input".into(),
        }
    }

    pub(crate) fn collect_rest(mut self) -> TokenStream {
        TokenStream::from_iter(iter::from_fn(|| self.bump().ok()))
    }

    pub(crate) fn parse<T: Parse>(&mut self) -> Result<T, Error> {
        T::parse(self)
    }
}
