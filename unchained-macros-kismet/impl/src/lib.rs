use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{ItemImpl, Type};
mod exec_bindings;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_bindings_emit_typed_descriptors_not_resolver_declarations() {
        let expanded = expand(
            quote!("/Script/Engine.KismetSystemLibrary"),
            quote! {
                #[cfg(feature = "fixture")]
                impl UKismetSystemLibrary {
                    #[cfg(any())]
                    pub(crate) unsafe fn IsDedicatedServer(value: *const ()) -> bool {}
                    pub unsafe fn IsServer(value: *const ()) -> bool {}
                    unsafe fn IsStandalone(value: *const ()) -> bool {}
                }
            },
        )
        .unwrap();
        syn::parse2::<syn::File>(expanded.clone()).unwrap();
        let text = expanded.to_string();
        for method in ["IsDedicatedServer", "IsServer", "IsStandalone"] {
            assert!(text.contains(&format!("\"UKismetSystemLibrary__{method}\"")));
            assert!(text.contains(&format!("static __UNCHAINED_KISMET_RESOLVER_{method}")));
        }

        assert_eq!(text.matches("static __UNCHAINED_KISMET_RESOLVER_").count(), 3);
        assert_eq!(text.matches("bind_resolved_func").count(), 3);
        assert_eq!(
            text.matches("as & dyn :: unchained_macros_core :: __private :: ResolverInfo")
                .count(),
            3
        );
        assert!(!text.contains("struct "));
        assert!(!text.contains("ResolverDeclaration"));
        assert!(!text.contains("ResolverSelection"));
        assert!(!text.contains("doc (hidden)"));
        assert!(text.matches("cfg (any ())").count() >= 3);
        assert!(text.contains("pub (crate) unsafe fn IsDedicatedServer"));
        assert!(text.contains("cfg (feature = \"fixture\")"));
    }

    #[test]
    fn runtime_bindings_reject_a_second_address_source() {
        assert!(expand(
            quote!("/Script/Fixture.Native"),
            quote! {
                impl Native {
                    #[bind_resolved_func(Backend, &DESCRIPTOR)]
                    unsafe fn call() {}
                }
            }
        )
        .is_err());
        assert!(expand(
            quote!("/Script/Fixture.Native"),
            quote! {
                impl Native {
                    #[dummy]
                    #[bind_resolved_func(Backend, &DESCRIPTOR)]
                    unsafe fn call() {}
                }
            }
        )
        .is_err());
    }

    #[test]
    fn dummy_metadata_cannot_collide_with_authored_items() {
        assert!(expand(
            quote!("/Script/Fixture.Native"),
            quote! {
                impl Native {
                    const __UNCHAINED_TARGET_Call: () = ();
                    #[dummy]
                    unsafe fn Call() {}
                }
            }
        )
        .unwrap_err()
        .to_string()
        .contains("conflicts"));
    }

    #[test]
    fn mixed_impl_preserves_constants_visibility_and_native_lifetimes() {
        let output = expand(
            quote!("/Script/Fixture.Native"),
            quote! {
                impl Native {
                    pub(crate) const AUTHORED: u32 = 42;
                    pub unsafe fn Call<'a>(&'a self) -> &'a Self {}
                    #[dummy]
                    pub(crate) unsafe fn Observe(value: bool) {}
                }
            },
        )
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        let text = output.to_string();
        assert_eq!(text.matches("runtime_bindings").count(), 1);
        assert_eq!(text.matches("KismetReflectionResolver :: new").count(), 1);
        assert_eq!(text.matches("KismetExecResolver :: new").count(), 1);
        assert!(text.contains("pub (crate) const AUTHORED"));
        assert!(text.contains("pub (crate) const Observe"));
        assert!(text.contains("pub unsafe fn Call < 'a > (& 'a self) -> & 'a Self"));
    }
}

/// Bind native C++ methods by reflected class path through the host's Kismet backend.
/// Native methods resolve C++ bodies; `#[dummy]` methods observe exec entry only.
#[proc_macro_attribute]
pub fn kismet_runtime_bindings(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

fn cfg_attributes(attributes: &[syn::Attribute]) -> impl Iterator<Item = &syn::Attribute> {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

fn class_path(args: Tokens) -> syn::Result<syn::LitStr> {
    let owner: syn::LitStr = syn::parse2(args)?;
    if !owner.value().starts_with("/Script/") || !owner.value().contains('.') || owner.value().contains('\0') {
        return Err(syn::Error::new_spanned(
            owner,
            "expected a /Script/Package.Class path without NUL",
        ));
    }

    Ok(owner)
}

fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let owner = class_path(args)?;
    let mut implementation: ItemImpl = syn::parse2(item)?;
    if implementation.trait_.is_some()
        || !implementation.generics.params.is_empty()
        || implementation.generics.where_clause.is_some()
    {
        return Err(syn::Error::new_spanned(
            &implementation,
            "kismet_runtime_bindings requires a non-generic inherent impl",
        ));
    }

    let Type::Path(ty) = implementation.self_ty.as_ref() else {
        return Err(syn::Error::new_spanned(
            &implementation.self_ty,
            "expected a named native type",
        ));
    };

    let class = &ty.path.segments.last().unwrap().ident;
    let implementation_cfg: Vec<_> = cfg_attributes(&implementation.attrs).cloned().collect();
    let mut declarations = Vec::new();
    let mut names: std::collections::HashSet<_> = implementation
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(method) => Some(method.sig.ident.to_string()),
            syn::ImplItem::Const(item) => Some(item.ident.to_string()),
            _ => None,
        })
        .collect();
    let mut items = Vec::new();
    for mut item in std::mem::take(&mut implementation.items) {
        let syn::ImplItem::Fn(method) = &mut item else {
            items.push(item);
            continue;
        };

        if method.attrs.iter().any(|attribute| {
            attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "bind_resolved_func")
        }) {
            return Err(syn::Error::new_spanned(
                method,
                "Kismet methods cannot also declare an address binding",
            ));
        }

        if method.attrs.iter().any(|attribute| attribute.path().is_ident("dummy")) {
            if !names.insert(format!("__UNCHAINED_TARGET_{}", method.sig.ident)) {
                return Err(syn::Error::new_spanned(
                    method,
                    "generated binding metadata conflicts with an associated item",
                ));
            }

            let (metadata, helpers) = exec_bindings::expand_method(&owner, class, method)?;
            items.extend(metadata);
            declarations.push(helpers);
            continue;
        }

        let method_name = method.sig.ident.to_string();
        let resolver_name = format!("{}__{}", class, method.sig.ident);
        let resolver = format_ident!("__UNCHAINED_KISMET_RESOLVER_{}", method.sig.ident);
        let cfg: Vec<_> = cfg_attributes(&method.attrs).cloned().collect();
        declarations.push(quote! {
            #(#cfg)*
            #[allow(non_upper_case_globals)]
            static #resolver: ::unchained_macros_core::Resolver<::unchained_macros_kismet::KismetReflectionResolver> =
                ::unchained_macros_core::Resolver {
                    name: #resolver_name,
                    implementation: ::unchained_macros_kismet::KismetReflectionResolver::new(#owner, #method_name),
                    filter: ::core::option::Option::None,
                };

            #(#cfg)*
            ::unchained_macros_core::__private::inventory::submit! {
                &#resolver as &dyn ::unchained_macros_core::__private::ResolverInfo
            }
        });

        method.attrs.push(syn::parse_quote!(#[bind_resolved_func(
            ::unchained_macros_kismet::KismetReflectionResolver, &#resolver
        )]));
        items.push(item);
    }

    implementation.items = items;
    Ok(quote! {
        #(#implementation_cfg)*
        const _: () = {
            #(#declarations)*
            #[::unchained_macros_core::runtime_bindings]
            #implementation
        };
    })
}
