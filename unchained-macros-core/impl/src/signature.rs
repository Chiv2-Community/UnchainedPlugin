use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::fold::Fold;
use syn::visit_mut::VisitMut;
use syn::{FnArg, GenericParam, ItemFn, Pat, Token, Type};

pub(crate) fn validate_native_signature(signature: &syn::Signature) -> syn::Result<()> {

    if signature.unsafety.is_none() {

        return Err(syn::Error::new_spanned(
            signature,
            "native resolver bindings require unsafe fn",
        ));
    }

    if signature.asyncness.is_some()
        || signature.constness.is_some()
        || signature.abi.is_some()
        || signature
            .generics
            .params
            .iter()
            .any(|parameter| !matches!(parameter, GenericParam::Lifetime(_)))
        || signature.generics.where_clause.is_some()
        || signature.variadic.is_some()
    {

        return Err(syn::Error::new_spanned(
            signature,
            "native bindings must be non-generic (except lifetimes), non-async Rust functions",
        ));
    }

    for argument in &signature.inputs {

        match argument {
            FnArg::Receiver(receiver) => {

                let element = match receiver.ty.as_ref() {
                    Type::Reference(reference) => &*reference.elem,
                    Type::Ptr(pointer) => &*pointer.elem,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            receiver,
                            "use a borrowed or raw-pointer Self receiver",
                        ))
                    }
                };

                if !matches!(element, Type::Path(path) if path.path.is_ident("Self")) {

                    return Err(syn::Error::new_spanned(receiver, "native receivers must point to Self"));
                }
            }

            FnArg::Typed(argument) => {

                let Pat::Ident(name) = argument.pat.as_ref() else {

                    return Err(syn::Error::new_spanned(
                        &argument.pat,
                        "native parameters must use plain identifiers",
                    ));
                };

                if name.by_ref.is_some() || name.subpat.is_some() {

                    return Err(syn::Error::new_spanned(
                        name,
                        "native parameters must use plain identifiers",
                    ));
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn validate_binding(function: &ItemFn) -> syn::Result<()> {

    validate_native_signature(&function.sig)?;

    if !function.block.stmts.is_empty() {

        return Err(syn::Error::new_spanned(
            &function.block,
            "native binding bodies must be empty",
        ));
    }

    Ok(())
}

pub(crate) fn consume_native_parameter_metadata(signature: &mut syn::Signature) -> syn::Result<()> {

    for argument in &mut signature.inputs {

        let FnArg::Typed(argument) = argument else {

            continue;
        };

        let annotations: Vec<_> = argument
            .attrs
            .iter()
            .filter(|attribute| attribute.path().is_ident("native_by_value"))
            .collect();

        if annotations.is_empty() {

            continue;
        }

        if annotations.len() != 1
            || !matches!(annotations[0].meta, syn::Meta::Path(_))
            || !matches!(argument.ty.as_ref(), Type::Ptr(pointer) if pointer.mutability.is_some())
        {

            return Err(syn::Error::new_spanned(
                annotations[0],
                "#[native_by_value] requires one bare annotation on verified *mut T ABI storage",
            ));
        }

        argument
            .attrs
            .retain(|attribute| !attribute.path().is_ident("native_by_value"));
    }

    Ok(())
}

struct ConcreteSelf<'a>(&'a Type);

impl Fold for ConcreteSelf<'_> {
    fn fold_expr_path(&mut self, path: syn::ExprPath) -> syn::ExprPath {

        if path.qself.is_none()
            && path.path.segments.len() > 1
            && path.path.segments.first().is_some_and(|s| s.ident == "Self")
        {

            let tail: syn::punctuated::Punctuated<_, Token![::]> = path.path.segments.iter().skip(1).cloned().collect();

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

            if path.qself.is_none() && path.path.segments.first().is_some_and(|s| s.ident == "Self") {

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

    fn visit_parenthesized_generic_arguments_mut(&mut self, _: &mut syn::ParenthesizedGenericArguments) {}

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {

        if lifetime.ident == "_" {

            *lifetime = self.0.clone();
        }
    }

    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {

        if reference.lifetime.is_none() || reference.lifetime.as_ref().is_some_and(|l| l.ident == "_") {

            reference.lifetime = Some(self.0.clone());
        }

        syn::visit_mut::visit_type_reference_mut(self, reference);
    }
}

pub(super) fn concrete_signature(function: &ItemFn, self_ty: &Type) -> syn::Signature {

    let mut native_signature = function.sig.clone();

    let ident = &function.sig.ident;

    // Preserve receiver-based output lifetime elision before making it explicit.
    if let Some(FnArg::Receiver(receiver)) = native_signature.inputs.first_mut() {

        if let Type::Reference(reference) = receiver.ty.as_mut() {

            let lifetime = reference
                .lifetime
                .clone()
                .filter(|l| l.ident != "_")
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

        let ty = &receiver.ty;

        let mut receiver_name = "__unchained_this".to_string();

        while function.sig.inputs.iter().any(|input| {

            matches!(input,
            FnArg::Typed(arg) if matches!(arg.pat.as_ref(), Pat::Ident(p) if p.ident == receiver_name))
        }) {

            receiver_name.push('_');
        }

        let receiver_name = format_ident!("{}", receiver_name);

        native_signature.inputs[0] = syn::parse_quote!(#receiver_name: #ty);
    }

    ConcreteSelf(self_ty).fold_signature(native_signature)
}

pub(crate) fn native_type(signature: &syn::Signature) -> syn::Result<Tokens> {

    if signature.generics.lifetimes().any(|p| !p.bounds.is_empty()) {

        return Err(syn::Error::new_spanned(
            &signature.generics,
            "native binding lifetimes cannot have bounds",
        ));
    }

    let lifetimes: Vec<_> = signature.generics.lifetimes().map(|p| &p.lifetime).collect();

    let binder = (!lifetimes.is_empty()).then(|| quote!(for<#(#lifetimes),*>));

    let types = signature.inputs.iter().map(|input| match input {
        FnArg::Typed(arg) => &arg.ty,
        _ => unreachable!(),
    });

    let output = &signature.output;

    Ok(quote!(#binder unsafe extern "C" fn(#(#types),*) #output))
}

pub(crate) fn normalized_mutability(signature: &syn::Signature) -> syn::Signature {

    let mut signature = signature.clone();

    for input in &mut signature.inputs {

        let FnArg::Typed(argument) = input else {

            unreachable!()
        };

        match &mut *argument.ty {
            Type::Ptr(pointer) => {

                pointer.mutability = None;

                pointer.const_token = Some(Default::default());
            }

            Type::Reference(reference) => reference.mutability = None,
            _ => {}
        }
    }

    signature
}

pub(crate) fn parameter_type(signature: &syn::Signature, index: usize) -> syn::Result<Tokens> {

    let mut parameter = signature.clone();

    parameter.inputs = std::iter::once(signature.inputs[index].clone()).collect();

    parameter.output = syn::ReturnType::Default;

    native_type(&parameter)
}

#[cfg(test)]

mod tests {

    use super::*;
    use quote::ToTokens;

    #[test]

    fn indirect_by_value_metadata_is_checked_and_consumed() {

        let mut signature: syn::Signature = syn::parse_quote! {
            unsafe fn Call(#[native_by_value] callback: *mut Callback)
        };

        super::consume_native_parameter_metadata(&mut signature).unwrap();

        let syn::FnArg::Typed(argument) = &signature.inputs[0] else {

            unreachable!()
        };

        assert!(argument.attrs.is_empty());

        assert!(matches!(argument.ty.as_ref(), syn::Type::Ptr(pointer) if pointer.mutability.is_some()));
    }

    #[test_case::test_case(syn::parse_quote!(unsafe fn Call(#[native_by_value] value: *const Callback)); "const pointer")]
    #[test_case::test_case(syn::parse_quote!(unsafe fn Call(#[native_by_value] value: &mut Callback)); "reference")]
    #[test_case::test_case(syn::parse_quote!(unsafe fn Call(#[native_by_value()] value: *mut Callback)); "arguments")]
    #[test_case::test_case(syn::parse_quote!(unsafe fn Call(#[native_by_value] #[native_by_value] value: *mut Callback)); "duplicate")]

    fn indirect_by_value_metadata_rejects_invalid_annotations(mut signature: syn::Signature) {

        assert!(super::consume_native_parameter_metadata(&mut signature).is_err());
    }

    #[test]

    fn normalization_preserves_nested_mutability_lifetimes_and_returns() {

        let signature = syn::parse_quote! {
            unsafe fn replacement<'a>(
                raw: *mut &'a mut u32,
                shared: &'a &'a mut u32,
                exclusive: &'a mut *mut u32,
            ) -> &'a mut u32
        };

        let normalized = normalized_mutability(&signature);

        let expected: syn::Signature = syn::parse_quote! {
            unsafe fn replacement<'a>(
                raw: *const &'a mut u32,
                shared: &'a &'a mut u32,
                exclusive: &'a *mut u32,
            ) -> &'a mut u32
        };

        assert_eq!(
            normalized.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
    }

    #[test]

    fn preserves_receiver_lifetimes_and_nested_callable_elision() {

        let signature = concrete_signature(
            &syn::parse_quote! {
                unsafe fn call(&self, callback: fn(&u8) -> &u8)
                    -> (&Self, fn(&u8) -> &u8) {}
            },
            &syn::parse_quote!(Native),
        );

        let expected: syn::Signature = syn::parse_quote! {
            unsafe fn call<'__unchained_receiver>(
                __unchained_this: &'__unchained_receiver Native,
                callback: fn(&u8) -> &u8
            ) -> (&'__unchained_receiver Native, fn(&u8) -> &u8)
        };

        use quote::ToTokens;

        assert_eq!(
            signature.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
    }
}
