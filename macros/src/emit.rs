use proc_macro2::{TokenStream, Span};
use quote::{quote, quote_spanned};

use crate::{ast, err::{err, Error}};


pub(crate) fn emit(input: ast::Input) -> Result<TokenStream, Error> {
    let buf_init = if let Some(expr) = &input.buffer {
        quote! { let mut buf = &mut *#expr; }
    } else {
        let prolog = input.prolog
            .ok_or(err!("you have to specify either a buffer to write into or an XML prolo"))?;

        let version = match prolog.version.as_str() {
            "1.0" => quote! { ogrim::Version::V1_0 },
            "1.1" => quote! { ogrim::Version::V1_1 },
            other => return Err(err!("invalid version '{other}'")),
        };
        let standalone = match prolog.standalone {
            None => quote! { None },
            Some(v) => quote! { Some(#v) },
        };
        let format = input.format.unwrap_or(quote! { ogrim::Format::Terse });


        quote! {
            let mut buf = ogrim::Document::new(#version, #standalone, #format);
        }
    };
    let ret = if input.buffer.is_some() { quote!{} } else { quote! { buf } };

    let root = emit_element(&input.root);


    Ok(quote! {
        {
            #buf_init
            #root
            #ret
        }
    })
}


fn emit_element(elem: &ast::Element) -> TokenStream {
    let name = &elem.name;
    let mut out = quote! {
        buf.open_tag(#name);
    };


    for attr in &elem.attrs {
        match attr {
            ast::Attr::Single(name, value) => {
                let (span, v) = match value {
                    ast::AttrValue::Literal(s) => (Span::call_site(), quote! { #s }),
                    ast::AttrValue::Expr(e) => (
                        span_of_tokenstream(&e),
                        quote! { (#e) },
                    ),
                };
                out.extend(quote_spanned!{span=>
                    buf.attr(#name, &#v);
                });
            }
            ast::Attr::Fill(expr) => {
                out.extend(quote! {
                    buf.attrs(#expr);
                });
            }
        }
    }

    if elem.empty {
        out.extend(quote! {
            buf.close_empty_elem_tag();
        });
    } else {
        let children = elem.children.iter().map(|child| {
            match child {
                ast::Child::Text(s) => quote! { buf.text(&#s); },
                ast::Child::TextExpr(e) => {
                    let span = span_of_tokenstream(&e);
                    quote_spanned! {span=> buf.text(&#e); }
                },
                ast::Child::Closure { arg, body } => quote! {
                    {
                        let #arg = &mut buf;
                        #body
                    }
                },
                ast::Child::Element(elem) => emit_element(elem),
            }
        });

        out.extend(quote! {
            buf.close_start_tag();
            #(#children)*
            buf.end_tag(#name);
        });
    }

    out
}

impl quote::ToTokens for ast::Name {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let s = &self.0;
        tokens.extend(quote! { #s });
    }
}

fn span_of_tokenstream(tokens: &TokenStream) -> Span {
    tokens.clone().into_iter().next().map(|tt| tt.span()).unwrap_or(Span::call_site())
}
