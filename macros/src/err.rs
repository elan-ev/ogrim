use proc_macro2::{TokenStream, TokenTree, Span, Group, Punct, Ident, Delimiter, Spacing, Literal};


#[derive(Debug)]
pub(crate) struct Error {
    pub(crate) span: Option<Span>,
    pub(crate) msg: String,
}

impl Error {
    pub(crate) fn to_compile_error(&self) -> TokenStream {
        let span = self.span.unwrap_or(Span::call_site());
        let tokens = vec![
            TokenTree::from(Ident::new("compile_error", span)),
            TokenTree::from(Punct::new('!', Spacing::Alone)),
            TokenTree::from(Group::new(
                Delimiter::Parenthesis,
                TokenTree::from(Literal::string(&self.msg)).into(),
            )),
        ];

        tokens.into_iter().map(|mut t| { t.set_span(span); t }).collect()
    }
}

macro_rules! err {
    (@ $span:expr, $($t:tt)*) => {
        Error {
            span: $span.into(),
            msg: format!($($t)*),
        }
    };
    ($($t:tt)*) => {
        Error {
            span: None,
            msg: format!($($t)*),
        }
    };
}

pub(crate) use err;
