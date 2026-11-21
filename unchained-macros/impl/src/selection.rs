use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, Parser};

pub(super) fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let mut declaration: syn::DeriveInput = syn::parse2(item)?;
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
    let mut required = None;
    syn::meta::parser(|meta| {
        if !meta.path.is_ident("require") {
            return Err(meta.error("expected require = [Variant, ...]"));
        }
        if required.is_some() {
            return Err(meta.error("duplicate require option"));
        }
        let content;
        let value = meta.value()?;
        syn::bracketed!(content in value);
        let keys = content.parse_terminated(syn::Ident::parse, syn::Token![,])?;
        let mut checked = Vec::new();
        for key in keys {
            if !data.variants.iter().any(|variant| variant.ident == key) {
                return Err(syn::Error::new_spanned(key, "unknown required selection variant"));
            }
            if checked.contains(&key) {
                return Err(syn::Error::new_spanned(key, "duplicate required selection"));
            }
            checked.push(key);
        }
        required = Some(checked);
        Ok(())
    }).parse2(args)?;
    let required = required.unwrap_or_default();
    let equality_arms = data.variants.iter().map(|variant| {
        let name = &variant.ident;
        let cfg = super::cfg_attributes(&variant.attrs);
        quote!(#(#cfg)* Self::#name => matches!(other, Self::#name),)
    }).collect::<Vec<_>>();
    let name_arms = data.variants.iter().map(|variant| {
        let name = &variant.ident;
        let cfg = super::cfg_attributes(&variant.attrs);
        quote!(#(#cfg)* Self::#name => stringify!(#name),)
    }).collect::<Vec<_>>();
    let duplicate_checks = data.variants.iter().map(|variant| {
        let name = &variant.ident;
        let cfg = super::cfg_attributes(&variant.attrs);
        quote! {
            #(#cfg)*
            {
                let mut index = 0;
                let mut count = 0;
                while index < keys.len() {
                    if matches!(keys[index], Self::#name) {
                        count += 1;
                    }
                    index += 1;
                }
                assert!(count <= 1, concat!("duplicate selection ", stringify!(#name)));
            }
        }
    }).collect::<Vec<_>>();
    let existing = declaration.attrs.iter().filter(|attribute| attribute.path().is_ident("derive"))
        .map(|attribute| attribute.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated))
        .collect::<syn::Result<Vec<_>>>()?;
    let missing: Vec<syn::Path> = ["Clone", "Copy", "PartialEq", "Eq"].into_iter()
        .filter(|name| !existing.iter().flatten().any(|path| path.is_ident(name)
            || path.segments.last().is_some_and(|segment| segment.ident == *name)))
        .map(|name| syn::parse_str(name).unwrap()).collect();
    if !missing.is_empty() {
        declaration.attrs.push(syn::parse_quote!(#[derive(#(#missing),*)]));
    }
    let name = &declaration.ident;
    Ok(quote! {
        #declaration
        impl ::unchained_macros::SelectionKey for #name {
            const REQUIRED: &'static [Self] = &[#(Self::#required),*];
            fn name(self) -> &'static str { match self { #(#name_arms)* } }
        }
        impl #name {
            #[doc(hidden)]
            pub const fn __unchained_validate_patch(self, resolver: &[(Self, bool)], patch: &[(Self, bool)]) {
                let mut index = 0;
                while index < patch.len() {
                    if patch[index].1 {
                        let mut target = 0;
                        while target < resolver.len() {
                            assert!(
                                !patch[index].0.__unchained_key_eq(resolver[target].0) || resolver[target].1,
                                "patch enables a selection disabled by its resolver"
                            );
                            target += 1;
                        }
                    }
                    index += 1;
                }
                let _ = self;
            }

            #[doc(hidden)]
            pub const fn __unchained_key_eq(self, other: Self) -> bool {
                match self { #(#equality_arms)* }
            }

            #[doc(hidden)]
            pub const fn __unchained_validate_selection(self, keys: &[Self]) {
                #(#duplicate_checks)*
                #(
                    let mut index = 0;
                    let mut found = false;
                    while index < keys.len() {
                        if matches!(keys[index], Self::#required) {
                            found = true;
                            break;
                        }
                        index += 1;
                    }
                    assert!(found, concat!(
                        "missing required selection ", stringify!(#required),
                        "; specify patterns or None"
                    ));
                )*
                let _ = (self, keys);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test_case::test_case(quote!(), quote!(enum Key { First, Second = 7 }); "enum")]
    #[test_case::test_case(quote!(), quote!(enum Key {}); "empty enum")]
    #[test_case::test_case(quote!(require = [First, Second]), quote!(enum Key { First, Second }); "required variants")]
    #[test_case::test_case(quote!(require = []), quote!(enum Key { First }); "no required variants")]
    fn accepts_fieldless_enums(args: TokenStream, item: TokenStream) {
        syn::parse2::<syn::File>(expand(args, item).unwrap()).unwrap();
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
        assert!(expand(quote!(), item).is_err());
    }

    #[test_case::test_case(quote!(require = [Missing]); "unknown variant")]
    #[test_case::test_case(quote!(require = [First, First]); "duplicate variant")]
    #[test_case::test_case(quote!(require = [First], require = [First]); "duplicate option")]
    #[test_case::test_case(quote!(other = [First]); "unknown option")]
    #[test_case::test_case(quote!(require = First); "not an array")]
    fn rejects_invalid_required_selections(args: TokenStream) {
        assert!(expand(args, quote!(enum Key { First })).is_err());
    }
}
