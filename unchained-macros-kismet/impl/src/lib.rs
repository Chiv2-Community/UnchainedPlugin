use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{ItemImpl, Type};
mod exec_bindings;

/// Declare observation-only VM wrappers with an explicitly verified Step contract.
#[proc_macro_attribute]
pub fn kismet_exec_bindings(args: TokenStream, item: TokenStream) -> TokenStream {
    exec_bindings::expand(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}

/// Bind native C++ methods by reflected class path through the host's Kismet backend.
/// This resolves native bodies, not Blueprint VM exec-thunk signatures.
#[proc_macro_attribute]
pub fn kismet_runtime_bindings(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}

fn cfg_attributes(attributes: &[syn::Attribute]) -> impl Iterator<Item = &syn::Attribute> {
    attributes.iter().filter(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let owner: syn::LitStr = syn::parse2(args)?;
    if !owner.value().starts_with("/Script/") || !owner.value().contains('.') || owner.value().contains('\0') {
        return Err(syn::Error::new_spanned(owner, "expected a /Script/Package.Class path without NUL"));
    }
    let mut implementation: ItemImpl = syn::parse2(item)?;
    if implementation.trait_.is_some() || !implementation.generics.params.is_empty() || implementation.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(&implementation, "kismet_runtime_bindings requires a non-generic inherent impl"));
    }
    let Type::Path(ty) = implementation.self_ty.as_ref() else {
        return Err(syn::Error::new_spanned(&implementation.self_ty, "expected a named native type"));
    };
    let class = &ty.path.segments.last().unwrap().ident;
    let implementation_cfg: Vec<_> = cfg_attributes(&implementation.attrs).cloned().collect();
    let mut declarations = Vec::new();
    for item in &mut implementation.items {
        let syn::ImplItem::Fn(method) = item else { continue };
        if method.attrs.iter().any(|attribute| attribute.path().segments.last().is_some_and(|segment|
            segment.ident == "bind_resolved_func"))
        {
            return Err(syn::Error::new_spanned(method, "Kismet methods cannot also declare an address binding"));
        }
        let method_name = method.sig.ident.to_string();
        let resolver = format_ident!("{}__{}", class, method.sig.ident);
        let visibility = &method.vis;
        let cfg: Vec<_> = cfg_attributes(&method.attrs).cloned().collect();
        declarations.push(quote! {
            #(#implementation_cfg)* #(#cfg)*
            #[allow(non_camel_case_types)]
            #visibility struct #resolver;
            #(#implementation_cfg)* #(#cfg)*
            impl ::unchained_macros_core::ResolverDeclaration for #resolver {
                type Backend = ::unchained_macros_kismet::KismetReflectionResolver;
                const NAME: &'static str = stringify!(#resolver);
                const UNRESTRICTED: bool = true;
                const DESCRIPTOR: &'static ::unchained_macros_core::Resolver<Self::Backend> = {
                    static DESCRIPTOR: ::unchained_macros_core::Resolver<::unchained_macros_kismet::KismetReflectionResolver> = ::unchained_macros_core::Resolver {
                        name: stringify!(#resolver),
                        implementation: ::unchained_macros_kismet::KismetReflectionResolver::new(#owner, #method_name),
                        filter: ::core::option::Option::None,
                    };
                    &DESCRIPTOR
                };
            }
            #(#implementation_cfg)* #(#cfg)*
            impl<E: ::unchained_macros_core::SelectionKey> ::unchained_macros_core::ResolverSelection<E> for #resolver {
                const SELECTIONS: &'static [(E, bool)] = &[];
            }
            #(#implementation_cfg)* #(#cfg)*
            ::unchained_macros_core::__private::inventory::submit! {
                <#resolver as ::unchained_macros_core::ResolverDeclaration>::DESCRIPTOR
                    as &dyn ::unchained_macros_core::__private::ResolverInfo
            }
        });
        method.attrs.push(syn::parse_quote!(#[bind_resolved_func(#resolver)]));
    }
    Ok(quote! {
        #(#declarations)*
        #[::unchained_macros_core::runtime_bindings]
        #implementation
    })
}
