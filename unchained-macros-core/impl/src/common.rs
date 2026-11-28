use proc_macro2::{Span, TokenStream as Tokens};
use syn::parse::{Parse, ParseStream};
use syn::Path;

pub(crate) struct CallbackArguments {
    pub target: Path,
    pub condition: syn::Expr,
}

pub(crate) fn fresh_local(signature: &syn::Signature, role: &str) -> syn::Ident {
    let mut name = format!("__unchained_{role}");

    while signature.ident == name
        || signature.inputs.iter().any(|input| {
            matches!(input,
        syn::FnArg::Typed(arg) if matches!(&*arg.pat, syn::Pat::Ident(pat) if pat.ident == name))
        })
    {
        name.push('_');
    }

    syn::Ident::new(&name, Span::mixed_site())
}

impl Parse for CallbackArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let target: Path = input.parse()?;
        if target.segments.len() < 2 || target.segments.iter().any(|s| !s.arguments.is_empty()) {
            return Err(syn::Error::new_spanned(
                target,
                "callback target must be a non-generic Type::method path",
            ));
        }

        if !input.is_empty() {
            input.parse::<syn::Token![,]>()?;
        }

        let condition = if input.is_empty() {
            syn::parse_quote!(|| true)
        } else {
            let expr: syn::Expr = input.parse()?;

            input.parse::<Option<syn::Token![,]>>()?;

            match expr {
                syn::Expr::Closure(_) => expr,
                _ => syn::parse_quote!(|| (#expr)()),
            }
        };

        Ok(Self { target, condition })
    }
}

pub(crate) fn cfg_attributes(attributes: &[syn::Attribute]) -> impl Iterator<Item = &syn::Attribute> {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

pub(crate) fn is_binding_attribute(attribute: &syn::Attribute) -> bool {
    attribute
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "bind_resolved_func")
}

pub(crate) fn resolver_path(args: Tokens) -> syn::Result<Path> {
    let resolver: Path = syn::parse2(args)?;

    if resolver.segments.iter().any(|segment| !segment.arguments.is_empty()) {
        return Err(syn::Error::new_spanned(
            &resolver,
            "resolver paths cannot have generic arguments",
        ));
    }

    Ok(resolver)
}
