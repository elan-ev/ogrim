use err::Error;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;


mod ast;
mod emit;
mod err;
mod parse;



#[proc_macro]
pub fn xml(input: TokenStream) -> TokenStream {
    run(input.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

fn run(input: TokenStream2) -> Result<TokenStream2, Error> {
    let input = ast::Input::parse_input(input)?;
    emit::emit(input)
}
