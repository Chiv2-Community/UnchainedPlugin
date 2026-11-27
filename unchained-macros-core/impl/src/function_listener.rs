use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, Type};
use crate::common::{cfg_attributes, CallbackArguments, fresh_local};

pub(crate) fn expand(args: Tokens, item: Tokens, exit: bool) -> syn::Result<Tokens> {
    let function: ItemFn = syn::parse2(item)?;
    let CallbackArguments { mut target, condition } = syn::parse2(args)?;
    if target.segments.len() < 2 || target.segments.iter().any(|s| !s.arguments.is_empty()) {
        return Err(syn::Error::new_spanned(target, "listener target must be a non-generic Type::method descriptor path"));
    }
    crate::signature::validate_native_signature(&function.sig)?;
    if !function.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(&function.sig.generics, "listeners use call-scoped elided borrows, not explicit lifetime parameters"));
    }
    if !matches!(function.sig.output, syn::ReturnType::Default)
        && !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(&**ty, Type::Tuple(t) if t.elems.is_empty()))
    {
        return Err(syn::Error::new_spanned(&function.sig.output, "listeners must return ()"));
    }
    let mut names = Vec::new();
    let mut types = Vec::new();
    for input in &function.sig.inputs {
        let FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(input, "listeners are free functions with explicit borrowed receiver arguments"));
        };
        let Type::Reference(reference) = &*arg.ty else {
            return Err(syn::Error::new_spanned(&arg.ty, "listener parameters must be immutable borrowed observations (&T)"));
        };
        if reference.mutability.is_some() || reference.lifetime.is_some() {
            return Err(syn::Error::new_spanned(&arg.ty, "listener observations require call-scoped immutable &T borrows"));
        }
        let Pat::Ident(name) = &*arg.pat else { unreachable!() };
        names.push(name.ident.clone());
        types.push((*reference.elem).clone());
    }
    if exit && types.is_empty() {
        return Err(syn::Error::new_spanned(&function.sig, "exit listeners require a final borrowed result parameter (including &() for void)"));
    }
    let result = exit.then(|| types.pop().unwrap());
    if exit { names.pop(); }
    if exit && types.iter().any(|ty| matches!(ty, Type::Reference(_))) {
        return Err(syn::Error::new_spanned(&function.sig, "exit input snapshots require ObserverSnapshot, not native reference inputs"));
    }
    let method = target.segments.last_mut().unwrap();
    method.ident = format_ident!("__UNCHAINED_TARGET_{}", method.ident, span = Span::mixed_site());
    let ident = &function.sig.ident;
    let registration = format_ident!("__UNCHAINED_REGISTRATION_{}", ident);
    let condition_fn = format_ident!("__unchained_condition_{}", ident);
    let prepare = format_ident!("__unchained_prepare_{}", ident);
    let finish = format_ident!("__unchained_finish_{}", ident);
    let cfg: Vec<_> = cfg_attributes(&function.attrs).collect();
    let indices: Vec<_> = (0..types.len()).map(syn::Index::from).collect();
    let snapshot_types: Vec<_> = types.iter().map(|ty|
        quote!(<#ty as ::unchained_macros_core::ObserverSnapshot>::Snapshot)).collect();
    let phase = if exit { quote!(exit) } else { quote!(entry) };
    let action = if exit { quote!((#target, #ident, #prepare)) } else { quote!((#target, #ident)) };
    let snapshot = fresh_local(&function.sig, "snapshot");
    let state = fresh_local(&function.sig, "state");
    let returned = fresh_local(&function.sig, "result");
    let helpers = result.map(|result| quote! {
        #(#cfg)*
        #[allow(non_snake_case)]
        unsafe fn #prepare(#(#names: &#types),*) -> ::unchained_macros_core::ExitObservation {
            let #snapshot: (#(#snapshot_types,)*) =
                (#(<#types as ::unchained_macros_core::ObserverSnapshot>::snapshot(#names),)*);
            unsafe { ::unchained_macros_core::ExitObservation::from_snapshot(#snapshot, #finish) }
        }
        #(#cfg)*
        #[allow(non_snake_case)]
        unsafe fn #finish(#state: *mut (), #returned: *const ()) {
            let #snapshot = unsafe { &*#state.cast::<(#(#snapshot_types,)*)>() };
            let #returned = unsafe { &*#returned.cast::<#result>() };
            unsafe { #ident(
                #(<#types as ::unchained_macros_core::ObserverSnapshot>::borrow(&#snapshot.#indices),)*
                #returned,
            ); }
        }
    });
    Ok(quote! {
        #function
        #helpers
        #(#cfg)*
        #[allow(non_snake_case)]
        fn #condition_fn() -> bool { (#condition)() }
        #(#cfg)*
        #[allow(non_upper_case_globals)]
        static #registration: ::unchained_macros_core::ListenerRegistration =
            ::unchained_macros_core::ListenerRegistration::#phase(stringify!(#ident), &#action, #condition_fn);
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
    #[test_case::test_case(quote!(unsafe fn observe() {}), true, "final borrowed result"; "missing result")]
    #[test_case::test_case(quote!(unsafe fn observe(value: &&u32, result: &u32) {}), true, "not native reference"; "reference snapshots")]
    #[test_case::test_case(quote!(fn observe(value: &u32) {}), false, "unsafe fn"; "safe observer")]
    fn rejects_invalid_observers(item: Tokens, exit: bool, message: &str) {
        let error = expand(quote!(Native::Call), item, exit).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
    #[test]
    fn exit_snapshots_require_owned_snapshot_contract_and_include_void_result() {
        let expanded = expand(quote!(Native::Call, || enabled()),
            quote!(unsafe fn observe(value: &Owned, result: &()) {}), true).unwrap();
        let text = expanded.to_string();
        assert!(text.contains("Owned as :: unchained_macros_core :: ObserverSnapshot"));
        assert!(text.contains("ExitObservation :: from_snapshot"));
        assert!(text.contains("ListenerRegistration :: exit"));
        assert!(!text.contains("__unchained_release_"));
        assert!(!text.contains("Box ::"));
        assert!(!text.contains("AtomicBool"));
        assert!(!text.contains("__unchained_attach_"));
        assert!(text.contains("__UNCHAINED_TARGET_Call"));
        assert!(!text.contains("__UNCHAINED_BINDING"));
        syn::parse2::<syn::File>(expanded).unwrap();
    }
    #[test]
    fn entry_does_not_require_a_callable_native_binding_or_owned_copy() {
        let expanded = expand(quote!(VmOnly::Call),
            quote!(unsafe fn observe(message: &FString) {}), false).unwrap();
        let text = expanded.to_string();
        assert!(!text.contains("Copy"));
        assert!(!text.contains("__UNCHAINED_BINDING"));
        assert!(!text.contains("VmOnly :: Call"));
        syn::parse2::<syn::File>(expanded).unwrap();
    }
}
