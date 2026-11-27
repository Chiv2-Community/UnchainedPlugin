use proc_macro::TokenStream;

mod resolver;

#[proc_macro]

pub fn define_pattern_resolver(input: TokenStream) -> TokenStream {

    resolver::expand(input.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
