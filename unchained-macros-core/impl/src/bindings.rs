use super::*;
use quote::format_ident;
use syn::fold::Fold;
use syn::visit_mut::VisitMut;
use syn::Token;

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
        let (expanded, descriptor, helper) = super::function_detour::expand_binding(
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

struct ConcreteSelf<'a>(&'a Type);

impl Fold for ConcreteSelf<'_> {
    fn fold_expr_path(&mut self, path: syn::ExprPath) -> syn::ExprPath {
        if path.qself.is_none()
            && path.path.segments.len() > 1
            && path.path.segments.first().is_some_and(|s| s.ident == "Self")
        {
            let tail: syn::punctuated::Punctuated<_, Token![::]> =
                path.path.segments.iter().skip(1).cloned().collect();
            let concrete = self.0;
            return syn::parse_quote!(<#concrete>::#tail);
        }
        syn::fold::fold_expr_path(self, path)
    }

    fn fold_type(&mut self, ty: Type) -> Type {
        if matches!(&ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident("Self")) {
            return self.0.clone();
        }
        if let Type::Path(path) = &ty {
            if path.qself.is_none()
                && path.path.segments.first().is_some_and(|s| s.ident == "Self")
            {
                let tail: syn::punctuated::Punctuated<_, Token![::]> =
                    path.path.segments.iter().skip(1).cloned().collect();
                let concrete = self.0;
                return syn::parse_quote!(<#concrete>::#tail);
            }
        }
        syn::fold::fold_type(self, ty)
    }
}

struct ElidedOutput(syn::Lifetime);

impl VisitMut for ElidedOutput {
    fn visit_type_bare_fn_mut(&mut self, _: &mut syn::TypeBareFn) {}

    fn visit_parenthesized_generic_arguments_mut(
        &mut self, _: &mut syn::ParenthesizedGenericArguments,
    ) {}

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_" {
            *lifetime = self.0.clone();
        }
    }

    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {
        if reference.lifetime.is_none()
            || reference.lifetime.as_ref().is_some_and(|l| l.ident == "_")
        {
            reference.lifetime = Some(self.0.clone());
        }
        syn::visit_mut::visit_type_reference_mut(self, reference);
    }
}

pub(super) fn concrete_signature(function: &ItemFn, self_ty: Option<&Type>) -> syn::Result<syn::Signature> {
    let mut native_signature = function.sig.clone();
    let ident = &function.sig.ident;
    // Preserve receiver-based output lifetime elision before making it explicit.
    if let Some(FnArg::Receiver(receiver)) = native_signature.inputs.first_mut() {
        let self_ty = self_ty.ok_or_else(|| {
            syn::Error::new_spanned(
                &*receiver, "detour methods require #[unchained_macros_core::runtime_bindings] on the impl",
            )
        })?;
        if let Type::Reference(reference) = receiver.ty.as_mut() {
            let lifetime = reference.lifetime.clone().filter(|l| l.ident != "_")
                .unwrap_or_else(|| {
                    let mut name = "__unchained_receiver".to_string();
                    while function.sig.generics.lifetimes().any(|p| p.lifetime.ident == name) {
                        name.push('_');
                    }
                    let lifetime = syn::Lifetime::new(&format!("'{name}"), ident.span());
                    native_signature.generics.params.push(syn::parse_quote!(#lifetime));
                    lifetime
                });
            reference.lifetime = Some(lifetime.clone());
            ElidedOutput(lifetime).visit_return_type_mut(&mut native_signature.output);
        }
        let ty = ConcreteSelf(self_ty).fold_type(*receiver.ty.clone());
        let mut receiver_name = "__unchained_this".to_string();
        while function.sig.inputs.iter().any(|input| matches!(input,
            FnArg::Typed(arg) if matches!(arg.pat.as_ref(), Pat::Ident(p) if p.ident == receiver_name)))
        {
            receiver_name.push('_');
        }
        let receiver_name = format_ident!("{}", receiver_name);
        native_signature.inputs[0] = syn::parse_quote!(#receiver_name: #ty);
    }
    if let Some(self_ty) = self_ty {
        native_signature = ConcreteSelf(self_ty).fold_signature(native_signature);
    }
    Ok(native_signature)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn preserves_receiver_lifetimes_and_nested_callable_elision() {
        let signature = concrete_signature(&syn::parse_quote! {
            unsafe fn call(&self, callback: fn(&u8) -> &u8)
                -> (&Self, fn(&u8) -> &u8) {}
        }, Some(&syn::parse_quote!(Native))).unwrap();
        let expected: syn::Signature = syn::parse_quote! {
            unsafe fn call<'__unchained_receiver>(
                __unchained_this: &'__unchained_receiver Native,
                callback: fn(&u8) -> &u8
            ) -> (&'__unchained_receiver Native, fn(&u8) -> &u8)
        };
        use quote::ToTokens;
        assert_eq!(signature.to_token_stream().to_string(), expected.to_token_stream().to_string());
    }
}
