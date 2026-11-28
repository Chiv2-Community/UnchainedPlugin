use crate::common::{cfg_attributes, CallbackArguments};
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Type};

pub(crate) fn expand(args: Tokens, item: Tokens, exit: bool) -> syn::Result<Tokens> {
    let function: ItemFn = syn::parse2(item)?;

    let CallbackArguments { mut target, condition } = syn::parse2(args)?;

    crate::signature::validate_native_signature(&function.sig)?;

    if !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.sig.generics,
            "listeners use call-scoped elided borrows, not explicit lifetime parameters",
        ));
    }

    if !matches!(function.sig.output, syn::ReturnType::Default)
        && !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(&**ty, Type::Tuple(t) if t.elems.is_empty()))
    {
        return Err(syn::Error::new_spanned(
            &function.sig.output,
            "listeners must return ()",
        ));
    }

    for input in &function.sig.inputs {
        let FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "listeners are free functions with explicit borrowed receiver arguments",
            ));
        };

        let Type::Reference(reference) = &*arg.ty else {
            return Err(syn::Error::new_spanned(
                &arg.ty,
                "listener parameters must be immutable borrowed observations (&T)",
            ));
        };

        if reference.mutability.is_some() || reference.lifetime.is_some() {
            return Err(syn::Error::new_spanned(
                &arg.ty,
                "listener observations require call-scoped immutable &T borrows",
            ));
        }
    }
    if exit && function.sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "exit listeners take only the borrowed result (including &() for void)",
        ));
    }

    let method = target.segments.last_mut().unwrap();

    method.ident = format_ident!("__UNCHAINED_TARGET_{}", method.ident, span = Span::mixed_site());

    let ident = &function.sig.ident;

    let registration = format_ident!("__UNCHAINED_REGISTRATION_{}", ident);

    let cfg: Vec<_> = cfg_attributes(&function.attrs).collect();
    let phase = if exit { quote!(exit) } else { quote!(entry) };

    Ok(quote! {
        #function
        #(#cfg)*
        #[allow(non_upper_case_globals)]
        static #registration: ::unchained_macros_core::ListenerRegistration =
            ::unchained_macros_core::ListenerRegistration::new(stringify!(#ident),
                &::unchained_macros_core::ListenerAction::#phase(#target, #ident), #condition);
        #(#cfg)*
        ::unchained_macros_core::__private::inventory::submit! {
            &#registration
        }
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test_case::test_case(quote!(unsafe fn observe(value: u32) {}), false, "immutable borrowed"; "owned input")]
    #[test_case::test_case(quote!(unsafe fn observe(value: &mut u32) {}), false, "immutable"; "mutable input")]
    #[test_case::test_case(quote!(unsafe fn observe(value: &'static u32) {}), false, "call-scoped"; "static borrow")]
    #[test_case::test_case(quote!(unsafe fn observe<'a>(value: &'a u32) {}), false, "elided borrows"; "explicit lifetimes")]
    #[test_case::test_case(quote!(unsafe fn observe(value: &u32) -> u32 { 0 }), false, "return ()"; "result mutation")]
    #[test_case::test_case(quote!(unsafe fn observe() {}), true, "only the borrowed result"; "missing result")]
    #[test_case::test_case(quote!(unsafe fn observe(value: &&u32, result: &u32) {}), true, "only the borrowed result"; "exit with inputs")]
    #[test_case::test_case(quote!(fn observe(value: &u32) {}), false, "unsafe fn"; "safe observer")]

    fn rejects_invalid_observers(item: Tokens, exit: bool, message: &str) {
        let error = expand(quote!(Native::Call), item, exit).unwrap_err();

        assert!(error.to_string().contains(message), "{error}");
    }

    #[test]

    fn exit_registers_void_result_callback_directly() {
        let expanded = expand(
            quote!(Native::Call, || enabled()),
            quote!(
                unsafe fn observe(result: &()) {}
            ),
            true,
        )
        .unwrap();

        let text = expanded.to_string();

        assert!(text.contains("ListenerAction :: exit"));

        assert!(text.contains("__UNCHAINED_TARGET_Call"));

        assert!(!text.contains("__UNCHAINED_BINDING"));

        syn::parse2::<syn::File>(expanded).unwrap();
    }

    #[test]

    fn entry_does_not_require_a_callable_native_binding_or_owned_copy() {
        let expanded = expand(
            quote!(VmOnly::Call),
            quote!(
                unsafe fn observe(message: &FString) {}
            ),
            false,
        )
        .unwrap();

        let text = expanded.to_string();

        assert!(!text.contains("Copy"));

        assert!(!text.contains("__UNCHAINED_BINDING"));

        assert!(!text.contains("VmOnly :: Call"));

        syn::parse2::<syn::File>(expanded).unwrap();
    }
}
