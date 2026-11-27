pub(crate) fn attach_native_documentation(attributes: &mut Vec<syn::Attribute>, key: &str, fallback: &syn::LitStr) {
    let generated = std::env::var_os("UNCHAINED_NATIVE_DOCS_DIR")
        .map(std::path::PathBuf::from)
        .map(|directory| directory.join(format!("{key}.md")))
        .filter(|path| path.is_file());
    if let Some(path) = generated {
        // The generated document already contains the declaration's safety/source notes.
        attributes.retain(|attribute| {
            !matches!(&attribute.meta,
            syn::Meta::NameValue(value) if attribute.path().is_ident("doc")
                && matches!(&value.value, syn::Expr::Lit(value) if matches!(value.lit, syn::Lit::Str(_))))
        });

        let path = syn::LitStr::new(&path.to_string_lossy(), fallback.span());
        attributes.insert(0, syn::parse_quote!(#[doc = ::core::include_str!(#path)]));
    } else {
        attributes.insert(0, syn::parse_quote!(#[doc = #fallback]));
    }
}

#[cfg(test)]
pub(crate) fn documentation(attributes: &[syn::Attribute]) -> String {
    attributes
        .iter()
        .filter_map(|attribute| {
            if !attribute.path().is_ident("doc") {
                return None;
            }

            let syn::Meta::NameValue(value) = &attribute.meta else {
                return None;
            };

            let syn::Expr::Lit(value) = &value.value else {
                return None;
            };

            let syn::Lit::Str(value) = &value.lit else { return None };
            Some(value.value())
        })
        .collect::<Vec<_>>()
        .join("\n")
}
