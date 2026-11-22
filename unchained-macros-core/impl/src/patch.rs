use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::{Parse, ParseStream}, Path, Token};

struct Arguments {
    resolver: Path,
    selection: Option<Path>,
}

impl Parse for Arguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let resolver: Path = input.parse()?;
        if resolver.segments.iter().any(|segment| !matches!(segment.arguments, syn::PathArguments::None)) {
            return Err(syn::Error::new_spanned(resolver, "patch targets must be non-generic resolver types"));
        }
        let selection = if input.is_empty() {
            None
        } else {
            input.parse::<Token![,]>()?;
            if input.is_empty() { None } else {
                let selection = input.parse()?;
                if input.peek(Token![,]) { input.parse::<Token![,]>()?; }
                Some(selection)
            }
        };
        Ok(Self { resolver, selection })
    }
}

pub(super) fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let Arguments { resolver, selection } = syn::parse2(args)?;
    let declaration: syn::Item = syn::parse2(item)?;
    let (name, attributes, factory) = match &declaration {
        syn::Item::Fn(function) => {
            let signature = &function.sig;
            if signature.asyncness.is_some() || signature.constness.is_some()
                || signature.unsafety.is_some() || signature.abi.is_some()
                || !signature.generics.params.is_empty() || signature.generics.where_clause.is_some()
                || signature.variadic.is_some() || signature.inputs.len() != 1
                || matches!(signature.inputs.first(), Some(syn::FnArg::Receiver(_)))
            {
                return Err(syn::Error::new_spanned(signature,
                    "patch factories must be safe, non-generic Rust functions taking one u64 address and returning Patch"));
            }
            let name = &signature.ident;
            (name, &function.attrs, quote!(#name))
        }
        syn::Item::Static(value) => {
            if matches!(value.mutability, syn::StaticMutability::Mut(_)) {
                return Err(syn::Error::new_spanned(value, "registered patches must be immutable statics"));
            }
            let name = &value.ident;
            (name, &value.attrs, quote!(|_| #name.clone()))
        }
        _ => return Err(syn::Error::new_spanned(declaration, "register_patch requires a function or immutable Patch static")),
    };
    if attributes.iter().any(|attribute| attribute.path().segments.last()
        .is_some_and(|segment| segment.ident == "register_patch"))
    {
        return Err(syn::Error::new_spanned(&declaration, "register each patch item only once"));
    }
    let cfg: Vec<_> = super::cfg_attributes(attributes).collect();
    let (validation, provider, selector, applicability, coverage) = match selection {
        None => (
            quote!(const _: () = assert!(
                <#resolver as ::unchained_macros_core::ResolverDeclaration>::UNRESTRICTED,
                "universal patch targets a restricted resolver; declare its selection explicitly"
            );),
            quote!(),
            quote!(::core::option::Option::None),
            quote!(|_| true),
            quote!(|_| Ok(())),
        ),
        Some(key) => (
            quote! {
                const _: () = (#key).__unchained_validate_patch(
                    ::unchained_macros_core::__private::resolver_selections::<#resolver, _>(#key),
                    &[(#key, true)],
                );
            },
            quote! {
                ::unchained_macros_core::__private::inventory::submit! {
                    ::unchained_macros_core::__private::SelectorProvider::for_key(#key)
                }
            },
            quote!(::core::option::Option::Some(::unchained_macros_core::__private::selector_type(#key))),
            quote!(|selection| ::unchained_macros_core::__private::selection_matches(selection, #key)),
            quote!(|patches| ::unchained_macros_core::__private::validate_patch_coverage::<#resolver, _>(#key, patches)),
        ),
    };
    Ok(quote! {
        #declaration
        #(#cfg)* #validation
        #(#cfg)* #provider
        #(#cfg)*
        ::unchained_macros_core::__private::inventory::submit! {
            ::unchained_macros_core::__private::ErasedPatchRegistration::new(&::unchained_macros_core::PatchRegistration {
                name: stringify!(#name),
                resolver: <#resolver as ::unchained_macros_core::ResolverDeclaration>::DESCRIPTOR,
                factory: #factory,
                selector: #selector,
                applies_to: #applicability,
                validate_group: #coverage,
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case::test_case(quote!(Target), quote!(pub fn patch(address: u64) -> Patch { make(address) }); "factory")]
    #[test_case::test_case(quote!(Target, Key::First), quote!(static PATCH: Patch = Patch::nop(0, 1);); "selected static")]
    fn accepts_patch_items(args: TokenStream, item: TokenStream) {
        syn::parse2::<syn::File>(expand(args, item).unwrap()).unwrap();
    }

    #[test_case::test_case(quote!(Target<u8>), quote!(static PATCH: Patch = value();); "generic resolver")]
    #[test_case::test_case(quote!(Target, Key::First, Key::Second), quote!(static PATCH: Patch = value();); "multiple selections")]
    #[test_case::test_case(quote!(Target), quote!(static mut PATCH: Patch = value();); "mutable static")]
    #[test_case::test_case(quote!(Target), quote!(fn patch() -> Patch {}); "no address")]
    #[test_case::test_case(quote!(Target), quote!(async fn patch(address: u64) -> Patch {}); "async")]
    #[test_case::test_case(quote!(Target), quote!(unsafe fn patch(address: u64) -> Patch {}); "unsafe")]
    #[test_case::test_case(quote!(Target), quote!(fn patch<T>(address: u64) -> Patch {}); "generic")]
    #[test_case::test_case(quote!(Target), quote!(#[register_patch(Other)] fn patch(address: u64) -> Patch {}); "duplicate attribute")]
    fn rejects_invalid_patches(args: TokenStream, item: TokenStream) {
        assert!(expand(args, item).is_err());
    }
}
