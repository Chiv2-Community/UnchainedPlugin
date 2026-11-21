use proc_macro2::TokenStream;
use quote::quote;

pub(super) fn expand(item: TokenStream) -> syn::Result<TokenStream> {
    let declaration: syn::DeriveInput = syn::parse2(item)?;
    if !declaration.generics.params.is_empty() || declaration.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(&declaration.generics, "selection keys must be non-generic"));
    }
    let syn::Data::Enum(data) = &declaration.data else {
        return Err(syn::Error::new_spanned(&declaration.ident, "selection keys must be fieldless enums"));
    };
    for variant in &data.variants {
        if !matches!(variant.fields, syn::Fields::Unit) {
            return Err(syn::Error::new_spanned(variant, "selection variants must have no fields"));
        }
    }
    let name = &declaration.ident;
    Ok(quote!(impl ::unchained_macros::SelectionKey for #name {}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test_case::test_case(quote!(enum Key { First, Second = 7 }); "enum")]
    #[test_case::test_case(quote!(enum Key {}); "empty enum")]
    fn accepts_fieldless_enums(item: TokenStream) {
        syn::parse2::<syn::ItemImpl>(expand(item).unwrap()).unwrap();
    }

    #[test_case::test_case(quote!(struct Key;); "struct")]
    #[test_case::test_case(quote!(union Key { value: u8 }); "union")]
    #[test_case::test_case(quote!(enum Key { Value(u8) }); "tuple variant")]
    #[test_case::test_case(quote!(enum Key { Value { value: u8 } }); "struct variant")]
    #[test_case::test_case(quote!(enum Key { Value() }); "empty tuple variant")]
    #[test_case::test_case(quote!(enum Key { Value {} }); "empty struct variant")]
    #[test_case::test_case(quote!(enum Key<T> { Value(T) }); "type generic")]
    #[test_case::test_case(quote!(enum Key<const N: usize> { Value }); "const generic")]
    #[test_case::test_case(quote!(enum Key<'a> { Value(&'a u8) }); "lifetime generic")]
    fn rejects_non_fieldless_or_generic_keys(item: TokenStream) {
        assert!(expand(item).is_err());
    }
}
