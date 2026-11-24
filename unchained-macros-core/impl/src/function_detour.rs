use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote, ToTokens};
use syn::{FnArg, ItemFn, Pat, Type};
use crate::{
    common::{cfg_attributes, CallbackArguments, fresh_local},
    docs::attach_native_documentation,
    signature::{validate_native_signature, native_type,
        normalized_mutability, parameter_type},
};

pub(super) fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let mut function: ItemFn = syn::parse2(item)?;
    let CallbackArguments { mut target, condition } = syn::parse2(args)?;
    if target.segments.len() < 2 || target.segments.iter().any(|s| !s.arguments.is_empty()) {
        return Err(syn::Error::new_spanned(
            target,
            "detour target must be a non-generic Type::method binding path",
        ));
    }
    let native_target = target.clone();
    let mut listener_target = target.clone();
    let listener_method = listener_target.segments.last_mut().unwrap();
    listener_method.ident = format_ident!("__UNCHAINED_LISTENERS_{}", listener_method.ident, span = Span::mixed_site());
    let mut hook_target = target.clone();
    let hook_method = hook_target.segments.last_mut().unwrap();
    hook_method.ident = format_ident!("__UNCHAINED_HOOK_{}", hook_method.ident, span = Span::mixed_site());
    let mut normalized_target = target.clone();
    let normalized_method = normalized_target.segments.last_mut().unwrap();
    normalized_method.ident = format_ident!("__UNCHAINED_MUTABILITY_{}", normalized_method.ident, span = Span::mixed_site());
    let mut parameter_target = target.clone();
    let parameter_method = parameter_target.segments.last_mut().unwrap();
    parameter_method.ident = format_ident!("__UNCHAINED_PARAMETERS_{}", parameter_method.ident, span = Span::mixed_site());
    // Keep the user's target span on the native method, not its internal descriptor.
    for segment in &mut target.segments {
        segment.ident.set_span(Span::mixed_site());
    }
    let method = target.segments.last_mut().unwrap();
    method.ident = format_ident!("__UNCHAINED_BINDING_{}", method.ident, span = Span::mixed_site());
    validate_native_signature(&function.sig)?;
    if function
        .sig
        .inputs
        .iter()
        .any(|a| matches!(a, FnArg::Receiver(_)))
    {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "detours must be free functions with explicit native receiver arguments",
        ));
    }
    let native_link = native_target.to_token_stream().to_string().replace(' ', "");
    let documentation = syn::LitStr::new(
        &format!(
            "## Native detour\n\n\
             Overrides in-game calls to [`{native_link}`]({native_link}).\n\n\
             Invoking `{native_link}` in this project directly skips this detour."
        ),
        function.sig.ident.span(),
    );
    let key = format!("detour-{}", function.sig.ident);
    attach_native_documentation(&mut function.attrs, &key, &documentation);
    let mut has_override = false;
    let mut checked_parameters = Vec::new();
    for (index, input) in function.sig.inputs.iter_mut().enumerate() {
        let FnArg::Typed(local) = input else { unreachable!() };
        let annotations: Vec<_> = local.attrs.iter().filter(|attr| attr.path().is_ident("mutability_override")).collect();
        if annotations.is_empty() {
            checked_parameters.push(index);
            continue;
        }
        if annotations.len() != 1 || !matches!(annotations[0].meta, syn::Meta::Path(_)) {
            return Err(syn::Error::new_spanned(annotations[0], "use one bare #[mutability_override] annotation"));
        }
        if !matches!(&*local.ty, Type::Ptr(_) | Type::Reference(_)) {
            return Err(syn::Error::new_spanned(&local.ty, "#[mutability_override] requires a raw pointer or reference detour parameter"));
        };
        has_override = true;
        local.attrs.retain(|attr| !attr.path().is_ident("mutability_override"));
    }
    let signature = &function.sig;
    let native = native_type(signature)?;
    let ident = &function.sig.ident;
    let registration = format_ident!("__UNCHAINED_REGISTRATION_{}", ident, span = Span::mixed_site());
    let replacement = format_ident!("__unchained_replacement_{}", ident, span = Span::mixed_site());
    let activate = fresh_local(signature, "activate");
    let native_replacement = fresh_local(signature, "replacement");
    let condition_fn = format_ident!("__unchained_condition_{}", ident, span = Span::mixed_site());
    let cfg: Vec<_> = cfg_attributes(&function.attrs).collect();
    let (names, types): (Vec<_>, Vec<_>) = signature
        .inputs
        .iter()
        .map(|input| {
            let FnArg::Typed(arg) = input else {
                unreachable!()
            };
            let Pat::Ident(pat) = &*arg.pat else {
                unreachable!()
            };
            (&pat.ident, &arg.ty)
        })
        .unzip();
    let output = &signature.output;
    let generics = &signature.generics;
    let signature_check = if has_override {
        let normalized = native_type(&normalized_mutability(signature))?;
        let parameter_checks = checked_parameters.into_iter().map(|index| {
            let parameter = parameter_type(signature, index)?;
            let index = syn::Index::from(index);
            Ok(quote! {
                let _: ::unchained_macros_core::NativeSignature<#parameter> = #parameter_target.#index;
            })
        }).collect::<syn::Result<Vec<_>>>()?;
        quote! {
            // Only outer pointer/reference mutability is erased. The complete
            // invariant signature still checks every lifetime relationship.
            let _: ::unchained_macros_core::NativeSignature<#normalized> = #normalized_target;
            #(#parameter_checks)*
        }
    } else {
        quote! { let _: ::unchained_macros_core::NativeFunction<#native, _> = #target; }
    };
    Ok(quote! {
        #function
        #(#cfg)*
        const _: () = {
            let _ = #native_target;
            // Invariant signature metadata makes mismatches compiler errors.
            #signature_check
        };
        #(#cfg)*
        #[allow(non_snake_case)]
        fn #condition_fn() -> bool { (#condition)() }
        #(#cfg)*
        #[allow(non_snake_case)]
        unsafe extern "C" fn #replacement #generics (#(#names: #types),*) #output {
            unsafe { #ident(#(#names),*) }
        }
        #(#cfg)*
        #[allow(non_upper_case_globals)]
        static #registration: ::unchained_macros_core::HookRegistration =
            ::unchained_macros_core::HookRegistration::new(
            stringify!(#ident), #target.binding(), #hook_target,
            |#activate| {
            // Checked outer mutability differences preserve the native ABI.
            // Reference validity and aliasing remain the detour author's obligation.
            let #native_replacement: #native = #replacement;
            if #hook_target.has_replacement() {
                return Err("Only one behavior-changing detour is supported per method".into());
            }
            unsafe { #listener_target.ensure_attached()?; }
            let #activate = #listener_target.activation(#activate, #condition_fn);
            unsafe { #hook_target.register_replacement(#native_replacement as usize, #activate)?; }
            Ok(())
            },
            || #listener_target.activation(true, #condition_fn),
        );
        #(#cfg)*
        ::unchained_macros_core::__private::inventory::submit! {
            &#registration
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::documentation;

    #[test]
    fn attachment_delegates_to_core_without_consumer_dependency_paths() {
        let expanded = expand(
            quote!(Native::Call), quote!(unsafe fn replacement(value: i32) -> i32 { value }),
        ).unwrap().to_string();
        assert!(expanded.contains("ensure_attached"));
        assert!(expanded.contains(":: unchained_macros_core :: __private :: inventory :: submit"));
        assert!(expanded.contains("HookRegistration :: new"));
        assert!(expanded.contains("__UNCHAINED_REGISTRATION_replacement"));
        assert!(!expanded.contains("OnceLock"));
        assert!(!expanded.contains("__unchained_attach_replacement"));
        assert!(!expanded.contains("claim_detour_address"));
        assert!(!expanded.contains("publish_function_detours"));
        assert!(!expanded.contains("RawDetour :: new"));
        assert!(expanded.contains("unsafe extern \"C\" fn (i32) -> i32"));
    }

    #[test]
    fn mutability_override_checks_only_the_annotated_raw_pointer() {
        let expanded = expand(
            quote!(Native::Call),
            quote!(unsafe fn replacement(#[mutability_override] value: *mut u32, other: *mut u8) {}),
        ).unwrap();
        let file = syn::parse2::<syn::File>(expanded).unwrap();
        let functions: Vec<_> = file.items.iter().filter_map(|item| {
            if let syn::Item::Fn(function) = item { Some(function) } else { None }
        }).collect();
        let local = functions.iter().find(|f| f.sig.ident == "replacement").unwrap();
        let native = functions.iter().find(|f| f.sig.ident == "__unchained_replacement_replacement").unwrap();
        let FnArg::Typed(local_arg) = &local.sig.inputs[0] else { unreachable!() };
        let FnArg::Typed(native_arg) = &native.sig.inputs[0] else { unreachable!() };
        assert!(local_arg.attrs.is_empty());
        assert_eq!(local_arg.ty.to_token_stream().to_string(), "* mut u32");
        assert_eq!(native_arg.ty.to_token_stream().to_string(), "* mut u32");
        assert!(native.block.to_token_stream().to_string().contains("replacement (value , other)"));
        assert_eq!(native.sig.inputs[1].to_token_stream().to_string(), "other : * mut u8");
        let text = file.to_token_stream().to_string();
        assert!(text.contains("__UNCHAINED_MUTABILITY_Call"));
        assert!(text.contains("__UNCHAINED_PARAMETERS_Call . 1"));
        assert!(!text.contains("__UNCHAINED_PARAMETERS_Call . 0"));
    }

    #[test_case::test_case(quote!(*const u32); "const pointer")]
    #[test_case::test_case(quote!(*mut u32); "mutable pointer")]
    #[test_case::test_case(quote!(&u32); "shared reference")]
    #[test_case::test_case(quote!(&mut u32); "mutable reference")]
    fn multiple_overrides_accept_both_pointer_and_reference_spellings(ty: Tokens) {
        let expanded = expand(
            quote!(Native::Call),
            quote!(unsafe fn replacement(
                #[mutability_override] first: #ty,
                #[mutability_override] second: #ty,
            ) {}),
        ).unwrap();
        let text = expanded.to_string();
        assert!(text.contains("__UNCHAINED_MUTABILITY_Call"));
        assert!(!text.contains("__UNCHAINED_PARAMETERS_Call"));
        assert!(!text.contains("# [mutability_override]"));
        syn::parse2::<syn::File>(expanded).unwrap();
    }

    #[test_case::test_case(quote!(unsafe fn replacement(#[mutability_override] value: u32) {}); "value")]
    #[test_case::test_case(quote!(unsafe fn replacement(#[mutability_override()] value: *mut u32) {}); "arguments")]
    #[test_case::test_case(quote!(unsafe fn replacement(#[mutability_override] #[mutability_override] value: *mut u32) {}); "duplicate")]
    fn mutability_override_rejects_values_and_invalid_annotations(function: Tokens) {
        assert!(expand(quote!(Native::Call), function).is_err());
    }

    #[test_case::test_case(quote!(Native::call), quote!(|| true); "default")]
    #[test_case::test_case(quote!(Native::call,), quote!(|| true); "trailing comma")]
    #[test_case::test_case(quote!(Native::call, || true), quote!(|| true); "enabled")]
    #[test_case::test_case(quote!(Native::call, || false), quote!(|| false); "disabled")]
    #[test_case::test_case(quote!(Native::call, || enabled()), quote!(|| enabled()); "closure")]
    #[test_case::test_case(quote!(Native::call, enabled), quote!(enabled); "function")]
    fn conditions_default_to_enabled_and_preserve_explicit_predicates(args: Tokens, expected: Tokens) {
        let expanded = expand(args, quote!(unsafe fn replacement() {})).unwrap();
        let file = syn::parse2::<syn::File>(expanded).unwrap();
        let condition = file.items.iter().find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "__unchained_condition_replacement" =>
                Some(&function.block),
            _ => None,
        }).unwrap();
        let expected: syn::Block = syn::parse2(quote!({ (#expected)() })).unwrap();
        assert_eq!(quote!(#condition).to_string(), quote!(#expected).to_string());
    }

    #[test]
    fn detour_hover_keeps_existing_docs_and_links_to_the_native_binding() {
        let expanded = expand(
            quote!(model::Native::Call),
            quote! {
                /// Existing safety note.
                unsafe fn replacement(value: i32) -> i32 { value }
            },
        ).unwrap();
        let file = syn::parse2::<syn::File>(expanded).unwrap();
        let function = file.items.iter().find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "replacement" => Some(function),
            _ => None,
        }).unwrap();
        let docs = documentation(&function.attrs);
        assert!(docs.starts_with("## Native detour"));
        assert!(docs.contains(" Existing safety note."));
        assert!(!docs.contains("or hover"));
        assert!(docs.contains("[`model::Native::Call`](model::Native::Call)"));
        assert_eq!(docs.matches("](model::Native::Call)").count(), 1);
        assert!(docs.contains("Overrides in-game calls to"));
        assert!(docs.contains("Invoking `model::Native::Call` in this project directly skips this detour."));
    }

    #[test_case::test_case(quote!(Resolver); "non method")]
    #[test_case::test_case(quote!(Type::<u8>::call); "generic type")]
    #[test_case::test_case(quote!(Type::call::<u8>); "generic method")]
    fn rejects_non_method_targets(target: Tokens) {
        assert!(expand(target, quote!(unsafe fn replacement() {})).is_err());
    }

    #[test_case::test_case(quote!(fn replacement() {}); "safe")]
    #[test_case::test_case(quote!(unsafe fn replacement<T>(value: T) {}); "generic")]
    #[test_case::test_case(quote!(async unsafe fn replacement() {}); "async")]
    #[test_case::test_case(quote!(unsafe extern "C" fn replacement() {}); "explicit ABI")]
    #[test_case::test_case(quote!(unsafe fn replacement(&self) {}); "receiver")]
    #[test_case::test_case(quote!(unsafe fn replacement<'a: 'static>(value: &'a i32) {}); "lifetime bound")]
    fn rejects_invalid_replacement_declarations(declaration: Tokens) {
        assert!(expand(quote!(Type::call), declaration).is_err());
    }
}
