use proc_macro2::TokenStream as Tokens;
use syn::Path;

pub(crate) fn cfg_attributes(attributes: &[syn::Attribute]) -> impl Iterator<Item = &syn::Attribute> {
    attributes.iter().filter(|attribute|
        attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

pub(crate) fn is_binding_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().segments.last().is_some_and(|segment|
        segment.ident == "bind_resolved_func")
}

pub(crate) fn resolver_path(args: Tokens) -> syn::Result<Path> {
    let resolver: Path = syn::parse2(args)?;
    if resolver.segments.iter().any(|segment| !segment.arguments.is_empty()) {
        return Err(syn::Error::new_spanned(&resolver, "resolver paths cannot have generic arguments"));
    }
    Ok(resolver)
}
