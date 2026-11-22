use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::ItemFn;
use crate::{common::{cfg_attributes, is_binding_attribute}, docs::attach_native_documentation, function};

pub(crate) fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(args, "runtime_bindings takes no arguments"));
    }
    let mut declaration: syn::Item = syn::parse2(item.clone())?;
    let (name, attributes) = match &mut declaration {
        syn::Item::Impl(_) => return expand_impl(args, item),
        syn::Item::Struct(item) => (&item.ident, &mut item.attrs),
        syn::Item::Enum(item) => (&item.ident, &mut item.attrs),
        syn::Item::Union(item) => (&item.ident, &mut item.attrs),
        syn::Item::Type(item) => (&item.ident, &mut item.attrs),
        _ => return Err(syn::Error::new_spanned(declaration, "runtime_bindings requires a native type or concrete inherent impl")),
    };
    let documentation = syn::LitStr::new(
        &format!("Native type `{name}`. See the module's native source header."),
        name.span(),
    );
    attach_native_documentation(attributes, &format!("type-{name}"), &documentation);
    Ok(quote!(#declaration))
}

pub(super) fn expand_impl(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(args, "runtime_bindings takes no arguments"));
    }
    let mut implementation: syn::ItemImpl = syn::parse2(item)?;
    if implementation.trait_.is_some()
        || !implementation.generics.params.is_empty()
        || implementation.generics.where_clause.is_some()
    {
        return Err(syn::Error::new_spanned(
            &implementation, "runtime_bindings requires a non-generic inherent impl",
        ));
    }
    let self_ty = &implementation.self_ty;
    let mut names: std::collections::HashSet<_> = implementation.items.iter().filter_map(|item| {
        match item {
            syn::ImplItem::Fn(method) => Some(method.sig.ident.to_string()),
            syn::ImplItem::Const(item) => Some(item.ident.to_string()),
            _ => None,
        }
    }).collect();
    let mut descriptors = Vec::new();
    let mut helpers = Vec::new();
    for item in &mut implementation.items {
        let syn::ImplItem::Fn(method) = item else { continue };
        let attributes: Vec<_> = method.attrs.iter().enumerate()
            .filter(|(_, attribute)| is_binding_attribute(attribute))
            .map(|(index, _)| index).collect();
        if attributes.is_empty() { continue; }
        if attributes.len() != 1 {
            return Err(syn::Error::new_spanned(method, "duplicate or conflicting native binding attributes"));
        }
        let attribute = method.attrs.remove(attributes[0]);
        let args = attribute.meta.require_list()?.tokens.clone();
        let function = ItemFn {
            attrs: method.attrs.clone(),
            vis: method.vis.clone(),
            sig: method.sig.clone(),
            block: Box::new(method.block.clone()),
        };
        let (expanded, descriptor, helper) = function::expand_method(
            args, &function, self_ty,
        )?;
        let normalized_name = format!("__UNCHAINED_MUTABILITY_{}", method.sig.ident);
        let parameter_name = format!("__UNCHAINED_PARAMETERS_{}", method.sig.ident);
        if !names.insert(descriptor.ident.to_string())
            || !names.insert(normalized_name)
            || !names.insert(parameter_name)
        {
            return Err(syn::Error::new_spanned(descriptor, "generated binding metadata conflicts with an associated item"));
        }
        method.attrs = expanded.attrs;
        method.vis = expanded.vis;
        method.sig = expanded.sig;
        method.block = *expanded.block;
        descriptors.push(syn::ImplItem::Const(descriptor));
        helpers.push(helper);
    }
    implementation.items.extend(descriptors);
    let cfg: Vec<_> = cfg_attributes(&implementation.attrs).collect();
    // Isolate helper names without changing module paths or method visibility.
    Ok(quote! {
        #(#cfg)*
        const _: () = {
            #implementation
            #(#helpers)*
        };
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;
    #[test]
    fn unbound_methods_keep_only_authored_docs_and_unchanged_code() {
        let original: syn::ItemImpl = syn::parse_quote! {
            impl Native {
                /// Existing safety notes.
                pub unsafe fn read(&self) -> u32 { self.value }
                fn helper() -> u32 { 7 }
            }
        };
        let expanded = expand_impl(quote!(), quote!(#original)).unwrap();
        let file: syn::File = syn::parse2(expanded).unwrap();
        let syn::Item::Const(container) = &file.items[0] else { panic!("expected impl container") };
        let syn::Expr::Block(block) = container.expr.as_ref() else { panic!("expected block") };
        let syn::Stmt::Item(syn::Item::Impl(implementation)) = &block.block.stmts[0] else { panic!("expected impl") };
        assert_eq!(implementation.to_token_stream().to_string(), original.to_token_stream().to_string());
    }

    #[test_case::test_case(quote!(), quote!(impl Native {
                #[bind_resolved_func(Resolver)]
                #[bind_resolved_func(OtherResolver)]
                unsafe fn call(&self) {}
            }); "duplicate bindings")]
    #[test_case::test_case(quote!(), quote!(impl Native {
                const __UNCHAINED_BINDING_call: () = ();
                #[bind_resolved_func(Resolver)]
                unsafe fn call(&self) {}
            }); "binding collision")]
    #[test_case::test_case(quote!(), quote!(impl Native {
                const __UNCHAINED_MUTABILITY_call: () = ();
                #[bind_resolved_func(Resolver)]
                unsafe fn call(&self) {}
            }); "mutability collision")]
    #[test_case::test_case(quote!(), quote!(impl Native {
                const __UNCHAINED_PARAMETERS_call: () = ();
                #[bind_resolved_func(Resolver)]
                unsafe fn call(&self) {}
            }); "parameters collision")]
    #[test_case::test_case(quote!(), quote!(impl<T> Native<T> {}); "generic impl")]
    #[test_case::test_case(quote!(), quote!(impl Trait for Native {}); "trait impl")]
    #[test_case::test_case(quote!(unexpected), quote!(impl Native {}); "arguments")]
    fn rejects_invalid_binding_impls(args: Tokens, implementation: Tokens) {
        assert!(expand_impl(args, implementation).is_err());
    }

    #[test_case::test_case(quote!(#[repr(C)] pub struct Native<T, const N: usize> where T: Copy { value: [T; N] }); "struct")]
    #[test_case::test_case(quote!(#[repr(u8)] pub enum Native { First, Second }); "enum")]
    #[test_case::test_case(quote!(#[repr(C)] pub union Native<T: Copy> { value: T, raw: usize }); "union")]
    #[test_case::test_case(quote!(pub type Native<T> = *mut T;); "alias")]
    fn runtime_type_markers_preserve_declarations_and_generate_no_registration(tokens: Tokens) {
        use quote::ToTokens;
        let original: syn::Item = syn::parse2(tokens.clone()).unwrap();
        let expanded = expand(quote!(), tokens).unwrap();
        assert!(!expanded.to_string().contains("inventory"));
        let mut actual: syn::Item = syn::parse2(expanded).unwrap();
        let attributes = match &mut actual {
            syn::Item::Struct(item) => &mut item.attrs,
            syn::Item::Enum(item) => &mut item.attrs,
            syn::Item::Union(item) => &mut item.attrs,
            syn::Item::Type(item) => &mut item.attrs,
            _ => unreachable!(),
        };
        assert!(attributes.remove(0).to_token_stream().to_string().contains("Native type"));
        assert_eq!(actual.to_token_stream().to_string(), original.to_token_stream().to_string());
    }

    #[test_case::test_case(quote!(Other), quote!(struct Native;); "arguments")]
    #[test_case::test_case(quote!(global), quote!(struct Native;); "removed global forwarding")]
    #[test_case::test_case(quote!(), quote!(fn helper() {}); "function")]
    #[test_case::test_case(quote!(), quote!(trait Native {}); "trait")]
    fn runtime_type_markers_reject_arguments_and_non_type_items(args: Tokens, item: Tokens) {
        assert!(expand(args, item).is_err());
    }

    #[test]
    fn runtime_binding_descriptors_carry_visibility_and_cfg_restrictions() {
        let expanded = expand_impl(
            quote!(),
            quote! {
                impl Native {
                    #[cfg(feature = "example")]
                    #[bind_resolved_func(Resolver)]
                    pub(crate) unsafe fn call(&self) -> &Self {}
                }
            },
        )
        .unwrap();
        let text = expanded.to_string();
        assert!(text.contains("pub (crate) const __UNCHAINED_BINDING_call"));
        assert!(text.contains("for < '__unchained_receiver >"));
        assert!(text.matches("cfg (feature = \"example\")").count() >= 3);
        syn::parse2::<syn::File>(expanded).unwrap();
    }
}
