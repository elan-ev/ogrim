use proc_macro2::{TokenStream, Ident};



#[derive(Debug)]
pub(crate) struct Input {
    pub(crate) buffer: Option<TokenStream>,
    pub(crate) format: Option<TokenStream>,
    pub(crate) prolog: Option<Prolog>,
    pub(crate) root: Element,
}

#[derive(Debug)]
pub(crate) struct Prolog {
    pub(crate) version: String,
    pub(crate) standalone: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Element {
    pub(crate) name: Name,
    pub(crate) attrs: Vec<(Name, AttrValue)>,
    pub(crate) children: Vec<Child>,
    pub(crate) empty: bool,
}

#[derive(Debug)]
pub(crate) enum Child {
    Text(String),
    TextExpr(TokenStream),
    Closure {
        arg: Ident,
        body: TokenStream,
    },
    Element(Element),
}

#[derive(Debug)]
pub(crate) enum AttrValue {
    Literal(String),
    Expr(TokenStream),
}

#[derive(Debug)]
pub(crate) struct Name(pub(crate) String);
