use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    Ident, Path, Token,
};

enum Patterns {
    List(TokenStream),
    Dictionary(Path, Vec<(Path, TokenStream)>),
}

struct Declaration {
    name: Ident,
    mode: Ident,
    patterns: Patterns,
    body: Option<(Ident, Ident, syn::Block)>,
}

impl Parse for Declaration {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {

        let name = input.parse()?;

        input.parse::<Token![,]>()?;

        let explicit_mode = input.peek(Ident);

        let mode = if explicit_mode {

            let mode: Ident = input.parse()?;

            input.parse::<Token![,]>()?;

            mode
        } else {

            syn::parse_quote!(Simple)
        };

        let patterns = if input.peek(syn::token::Bracket) {

            let content;

            syn::bracketed!(content in input);

            Patterns::List(content.parse()?)
        } else {

            let content;

            syn::braced!(content in input);

            let mut entries = Vec::new();

            while !content.is_empty() {

                let key: Path = content.parse()?;

                content.parse::<Token![:]>()?;

                let value = if content.peek(syn::token::Bracket) {

                    let values;

                    syn::bracketed!(values in content);

                    let values: TokenStream = values.parse()?;

                    if values.is_empty() {

                        return Err(content.error("pattern entries must not be empty; omit an optional key for fallback or use None to disable it"));
                    }

                    quote!([#values])
                } else {

                    let value: Ident = content.parse()?;

                    if value != "None" {

                        return Err(syn::Error::new_spanned(value, "expected a pattern list or None"));
                    }

                    quote!(None)
                };

                entries.push((key, value));

                if !content.is_empty() {

                    content.parse::<Token![,]>()?;
                }
            }

            let mut selector = entries
                .iter()
                .find(|(key, _)| key.segments.len() > 1)
                .map(|(key, _)| key.clone())
                .ok_or_else(|| input.error("qualify a dictionary key with its selector enum, e.g. Key::Variant"))?;

            selector.segments.pop();

            selector.segments.pop_punct();

            if selector
                .segments
                .iter()
                .any(|segment| !matches!(segment.arguments, syn::PathArguments::None))
            {

                return Err(syn::Error::new_spanned(selector, "selector enums must be non-generic"));
            }

            Patterns::Dictionary(selector, entries)
        };

        let body = if input.is_empty() {

            None
        } else {

            input.parse::<Token![,]>()?;

            if input.is_empty() {

                None
            } else {

                if explicit_mode {

                    return Err(input.error("inline resolver bodies do not take a scan mode"));
                }

                input.parse::<Token![|]>()?;

                let ctx = input.parse()?;

                input.parse::<Token![,]>()?;

                let patterns = input.parse()?;

                input.parse::<Token![|]>()?;

                let body = input.parse()?;

                if input.peek(Token![,]) {

                    input.parse::<Token![,]>()?;
                }

                Some((ctx, patterns, body))
            }
        };

        Ok(Self {
            name,
            mode,
            patterns,
            body,
        })
    }
}

pub(super) fn expand(input: TokenStream) -> syn::Result<TokenStream> {

    let Declaration {
        name,
        mode,
        patterns,
        body,
    } = syn::parse2(input)?;

    Ok(match (patterns, body) {
        (Patterns::List(values), None) => {

            quote!(::unchained_macros_patternsleuth::__pattern_resolver!(#name, #mode, [#values]);)
        }

        (Patterns::List(values), Some((ctx, patterns, body))) => {

            quote!(::unchained_macros_patternsleuth::__pattern_resolver!(#name, [#values], |#ctx, #patterns| #body);)
        }

        (Patterns::Dictionary(selector, entries), body) => {

            let (keys, values): (Vec<_>, Vec<_>) = entries.into_iter().unzip();

            match body {
                None => {

                    quote!(::unchained_macros_patternsleuth::__pattern_resolver!(#name, #mode, #selector, { #(#keys: #values),* });)
                }

                Some((ctx, patterns, body)) => quote!(
                    ::unchained_macros_patternsleuth::__pattern_resolver!(#name, #selector, { #(#keys: #values),* }, |#ctx, #patterns| #body);
                ),
            }
        }
    })
}

#[cfg(test)]

mod tests {

    use super::*;

    #[test_case::test_case(quote!(Target, ["AA BB"]); "list")]
    #[test_case::test_case(quote!(Target, Call, { Key::First: ["AA BB"], Key::Second: None }); "dictionary")]
    #[test_case::test_case(quote!(Target, { module::Key::First: ["AA"] }, |ctx, patterns| { Ok(1) }); "inline dictionary")]

    fn accepts_resolvers(input: TokenStream) {

        syn::parse2::<syn::File>(expand(input).unwrap()).unwrap();
    }

    #[test_case::test_case(quote!(Target, { First: ["AA BB"] }); "unqualified selector")]
    #[test_case::test_case(quote!(Target, { Key::First: false }); "invalid entry")]
    #[test_case::test_case(quote!(Target, {}); "empty dictionary")]
    #[test_case::test_case(quote!(Target, { Key::First: [] }); "empty entry")]

    fn rejects_invalid_resolvers(input: TokenStream) {

        assert!(expand(input).is_err());
    }
}
