use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::quote;
use syn::{FnArg, GenericParam, ItemFn, Pat, Path, ReturnType, Type};
mod detour;
mod function_detour;

/// Bind an unsafe Rust method/function to a separately declared named resolver.
///
/// The empty body calls the native entry or its original trampoline. Receiver
/// types and declared per-call lifetime relationships are preserved.
#[proc_macro_attribute]
pub fn bind_resolved_func(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}

/// Bind an unsafe method/function to a `GetPost_*` offset-registry key.
/// Unlike a resolver binding, the key does not name a Rust type.
#[proc_macro_attribute]
pub fn bind_resolved_request_function(args: TokenStream, item: TokenStream) -> TokenStream {
    expand_request(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error()).into()
}

/// Register a `_detour` function and generate its suffix-stripped trampoline caller.
#[proc_macro_attribute]
pub fn bind_resolved_func_detour(args: TokenStream, item: TokenStream) -> TokenStream {
    detour::expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error()).into()
}

/// Register a replacement for a typed method binding, not a resolver.
///
/// Hover the target method in this attribute to inspect its native declaration,
/// resolver link, and original-call behavior. The replacement's hover links
/// back to that binding and explains game-side interception.
#[proc_macro_attribute]
pub fn bind_func_detour(args: TokenStream, item: TokenStream) -> TokenStream {
    function_detour::expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error()).into()
}

/// Register a request-key detour and generate its suffix-stripped trampoline.
#[proc_macro_attribute]
pub fn bind_resolved_request_function_detour(args: TokenStream, item: TokenStream) -> TokenStream {
    detour::expand_request(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error()).into()
}

/// Expose typed direct bindings and expand legacy detours in a concrete impl.
#[proc_macro_attribute]
pub fn runtime_bindings(args: TokenStream, item: TokenStream) -> TokenStream {
    detour::expand_impl(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error()).into()
}

fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    expand_binding(args, item, true)
}

fn request_key(path: &Path) -> syn::Result<()> {
    let valid = path.get_ident().is_some_and(|name| {
        name.to_string().strip_prefix("GetPost_").is_some_and(|suffix| !suffix.is_empty())
    });
    if !valid {
        return Err(syn::Error::new_spanned(path, "request function keys must be unqualified GetPost_* identifiers"));
    }
    Ok(())
}

fn expand_request(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let key = resolver_path(args)?;
    request_key(&key)?;
    expand_binding(quote!(#key), item, false)
}

fn expand_binding(args: Tokens, item: Tokens, require_resolver_type: bool) -> syn::Result<Tokens> {
    let resolver = resolver_path(args)?;
    let resolver_name = resolver.segments.last().unwrap().ident.to_string();
    let mut function: ItemFn = syn::parse2(item)?;
    let signature = &function.sig;
    if signature.unsafety.is_none() {
        return Err(syn::Error::new_spanned(signature, "native resolver bindings require unsafe fn"));
    }
    if signature.asyncness.is_some() || signature.constness.is_some() || signature.abi.is_some()
        || signature.generics.params.iter().any(|parameter| !matches!(parameter, GenericParam::Lifetime(_)))
        || signature.generics.where_clause.is_some()
        || signature.variadic.is_some()
    {
        return Err(syn::Error::new_spanned(signature, "native bindings must be non-generic (except lifetimes), non-async Rust functions"));
    }
    if !function.block.stmts.is_empty() {
        return Err(syn::Error::new_spanned(&function.block, "native binding bodies must be empty"));
    }

    let mut types = Vec::new();
    let mut arguments = Vec::new();
    for argument in &signature.inputs {
        match argument {
            FnArg::Receiver(receiver) => {
                let element = match receiver.ty.as_ref() {
                    Type::Reference(reference) => &*reference.elem,
                    Type::Ptr(pointer) => &*pointer.elem,
                    _ => return Err(syn::Error::new_spanned(receiver, "use a borrowed or raw-pointer Self receiver")),
                };
                if !matches!(element, Type::Path(path) if path.path.is_ident("Self")) {
                    return Err(syn::Error::new_spanned(receiver, "native receivers must point to Self"));
                }
                let ty = &receiver.ty;
                arguments.push(quote!(self));
                types.push(quote!(#ty));
            }
            FnArg::Typed(argument) => {
                let Pat::Ident(name) = argument.pat.as_ref() else {
                    return Err(syn::Error::new_spanned(&argument.pat, "native parameters must use plain identifiers"));
                };
                if name.by_ref.is_some() || name.subpat.is_some() {
                    return Err(syn::Error::new_spanned(name, "native parameters must use plain identifiers"));
                }
                let name = &name.ident;
                let ty = &argument.ty;
                arguments.push(quote!(#name));
                types.push(quote!(#ty));
            }
        }
    }
    let output = &signature.output;
    let binding = syn::Ident::new("__UNCHAINED_BINDING", Span::mixed_site());
    let native = syn::Ident::new("__unchained_native", Span::mixed_site());
    let resolver_type_check = require_resolver_type.then(|| quote! {
        let _: ::core::marker::PhantomData<#resolver> = ::core::marker::PhantomData;
    });
    function.block = Box::new(syn::parse2(quote!({
        #resolver_type_check
        static #binding: ::unchained_macros::FunctionBinding =
            ::unchained_macros::FunctionBinding::new(#resolver_name);
        ::inventory::submit! {
            ::unchained_macros::FunctionBindingRegistration { binding: &#binding }
        }
        let #native: unsafe extern "C" fn(#(#types),*) #output = unsafe {
            ::core::mem::transmute(#binding.address()
                .expect(concat!("Native binding ", #resolver_name, " is not initialized")).get())
        };
        unsafe { #native(#(#arguments),*) }
    }))?);
    if !function.attrs.iter().any(|attribute| attribute.path().is_ident("inline")) {
        function.attrs.push(syn::parse_quote!(#[inline(always)]));
    }
    Ok(quote!(#function))
}

fn resolver_path(args: Tokens) -> syn::Result<Path> {
    let resolver: Path = syn::parse2(args)?;
    if resolver.segments.iter().any(|segment| !segment.arguments.is_empty()) {
        return Err(syn::Error::new_spanned(&resolver, "resolver paths cannot have generic arguments"));
    }
    Ok(resolver)
}

/// Bind a required native global to a zero-argument unsafe accessor.
/// `Value` reads a Copy value from its slot; `Address` returns an inline object's
/// raw address without copying it. An explicit mode is required. Pointers must be non-null.
#[proc_macro_attribute]
pub fn bind_resolver_value(args: TokenStream, item: TokenStream) -> TokenStream {
    expand_value(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}

fn expand_value(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    use syn::parse::Parser;
    let args = syn::punctuated::Punctuated::<Path, syn::Token![,]>::parse_terminated.parse2(args)?;
    let mut args = args.into_iter();
    let resolver = args.next().ok_or_else(|| syn::Error::new(Span::call_site(), "expected resolver path"))?;
    let resolver = resolver_path(quote!(#resolver))?;
    let address = match args.next() {
        Some(mode) if mode.is_ident("Value") => false,
        Some(mode) if mode.is_ident("Address") => true,
        Some(mode) => return Err(syn::Error::new_spanned(mode, "expected Value or Address binding mode")),
        None => return Err(syn::Error::new(Span::call_site(), "explicit Value or Address binding mode is required")),
    };
    if args.next().is_some() {
        return Err(syn::Error::new(Span::call_site(), "unexpected native value binding arguments"));
    }
    let resolver_name = resolver.segments.last().unwrap().ident.to_string();
    let mut function: ItemFn = syn::parse2(item)?;
    let signature = &function.sig;
    if signature.unsafety.is_none() {
        return Err(syn::Error::new_spanned(signature, "native value bindings require unsafe fn"));
    }
    if !signature.inputs.is_empty() || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some() || signature.asyncness.is_some()
        || signature.constness.is_some() || signature.abi.is_some() || signature.variadic.is_some()
    {
        return Err(syn::Error::new_spanned(signature, "native value bindings require a non-generic, non-async zero-argument Rust accessor"));
    }
    if !function.block.stmts.is_empty() {
        return Err(syn::Error::new_spanned(&function.block, "native binding bodies must be empty"));
    }
    let ReturnType::Type(_, ty) = &signature.output else {
        return Err(syn::Error::new_spanned(signature, "native value bindings require an explicit Copy value return type"));
    };
    if matches!(ty.as_ref(), Type::Reference(_))
        || matches!(ty.as_ref(), Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Option" || segment.ident == "Result"))
    {
        return Err(syn::Error::new_spanned(ty, "return the native Copy value directly, not a reference, Option or Result"));
    }
    if address && !matches!(ty.as_ref(), Type::Ptr(_)) {
        return Err(syn::Error::new_spanned(ty, "Address bindings require a raw pointer return type"));
    }
    let binding = syn::Ident::new("__UNCHAINED_VALUE_BINDING", Span::mixed_site());
    let value = syn::Ident::new("__unchained_value", Span::mixed_site());
    let check_pointer = matches!(ty.as_ref(), Type::Ptr(_)).then(|| quote! {
        assert!(!#value.is_null(), concat!("Required native value ", #resolver_name, " is null; the engine must initialize it before access"));
    });
    let read = if address {
        quote!(#binding.address() as #ty)
    } else {
        quote!(unsafe { (#binding.address() as *const #ty).read() })
    };
    function.block = Box::new(syn::parse2(quote!({
        let _: ::core::marker::PhantomData<#resolver> = ::core::marker::PhantomData;
        static #binding: ::unchained_macros::ValueBinding =
            ::unchained_macros::ValueBinding::new(#resolver_name);
        ::inventory::submit! {
            ::unchained_macros::ValueBindingRegistration { binding: &#binding }
        }
        fn __require_copy<T: ::core::marker::Copy>() {}
        __require_copy::<#ty>();
        let #value: #ty = #read;
        #check_pointer
        #value
    }))?);
    if !function.attrs.iter().any(|attribute| attribute.path().is_ident("inline")) {
        function.attrs.push(syn::parse_quote!(#[inline(always)]));
    }
    Ok(quote!(#function))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_bindings_use_registry_keys_without_resolver_types() {
        let expanded = expand_request(quote!(GetPost_GetMotd),
            quote!(pub unsafe fn get_post_get_motd(self: *mut Self, value: i32) -> i32 {})).unwrap();
        let text = expanded.to_string();
        assert!(text.contains("\"GetPost_GetMotd\""));
        assert!(text.contains("FunctionBindingRegistration"));
        assert!(!text.contains("PhantomData"));
        syn::parse2::<ItemFn>(expanded).unwrap();
    }

    #[test]
    fn request_bindings_reuse_native_signature_validation() {
        for declaration in [
            quote!(pub fn request() {}),
            quote!(pub unsafe fn request() { panic!() }),
            quote!(pub unsafe fn request<T>(value: T) {}),
        ] {
            assert!(expand_request(quote!(GetPost_GetMotd), declaration).is_err());
        }
    }

    #[test]
    fn request_bindings_reject_non_request_keys() {
        for key in [quote!(GetPost_), quote!(CreateHttpRequest), quote!(module::GetPost_GetMotd),
            quote!("GetPost_GetMotd"), quote!(GetPost_GetMotd<u8>)]
        {
            assert!(expand_request(key, quote!(pub unsafe fn request() {})).is_err());
        }
    }

    #[test_case::test_case(quote!(fn release(&mut self) {}), "require unsafe"; "safe function")]
    #[test_case::test_case(quote!(unsafe fn release(self) {}), "borrowed or raw-pointer"; "by value receiver")]
    #[test_case::test_case(quote!(unsafe fn release(&mut self) { panic!() }), "must be empty"; "nonempty body")]
    #[test_case::test_case(quote!(unsafe fn release<T>(&mut self, value: T) {}), "non-generic"; "generic method")]
    #[test_case::test_case(quote!(async unsafe fn release(&mut self) {}), "non-async"; "async method")]
    #[test_case::test_case(quote!(unsafe fn release((a, b): (u8, u8)) {}), "plain identifiers"; "destructured argument")]
    fn rejects_unsupported_native_declarations(item: Tokens, message: &str) {
        assert!(expand(quote!(Release), item).unwrap_err().to_string().contains(message));
    }

    #[test]
    fn rejects_generic_resolver_paths() {
        assert!(expand(quote!(Release<u8>), quote!(unsafe fn release() {})).is_err());
        assert!(expand_value(quote!(NativeValue<u8>, Value), quote!(unsafe fn value() -> u32 {})).is_err());
    }

    #[test]
    fn rejects_invalid_value_binding_modes() {
        assert!(expand_value(quote!(Objects, Address), quote!(unsafe fn objects() -> u32 {}))
            .unwrap_err().to_string().contains("raw pointer return type"));
        assert!(expand_value(quote!(Objects, unknown), quote!(unsafe fn objects() -> *const u32 {})).is_err());
        assert!(expand_value(quote!(Objects, Address, Address), quote!(unsafe fn objects() -> *const u32 {})).is_err());
        assert!(expand_value(quote!(Objects), quote!(unsafe fn objects() -> *const u32 {}))
            .unwrap_err().to_string().contains("explicit Value or Address"));
        assert!(expand_value(quote!(Objects, address), quote!(unsafe fn objects() -> *const u32 {}))
            .unwrap_err().to_string().contains("expected Value or Address"));
    }

    #[test_case::test_case(quote!(fn value() -> u32 {}), "require unsafe"; "safe accessor")]
    #[test_case::test_case(quote!(unsafe fn value(arg: u32) -> u32 {}), "zero-argument"; "argument")]
    #[test_case::test_case(quote!(unsafe fn value(&self) -> u32 {}), "zero-argument"; "receiver")]
    #[test_case::test_case(quote!(unsafe fn value<T>() -> T {}), "non-generic"; "generic value")]
    #[test_case::test_case(quote!(unsafe fn value<'a>() -> u32 {}), "non-generic"; "generic lifetime")]
    #[test_case::test_case(quote!(async unsafe fn value() -> u32 {}), "non-async"; "async value")]
    #[test_case::test_case(quote!(const unsafe fn value() -> u32 {}), "zero-argument Rust accessor"; "const value")]
    #[test_case::test_case(quote!(unsafe extern "C" fn value() -> u32 {}), "Rust accessor"; "explicit ABI")]
    #[test_case::test_case(quote!(unsafe fn value() -> u32 { 0 }), "must be empty"; "nonempty value body")]
    #[test_case::test_case(quote!(unsafe fn value() {}), "explicit Copy value"; "implicit return")]
    #[test_case::test_case(quote!(unsafe fn value() -> &'static u32 {}), "not a reference"; "reference")]
    #[test_case::test_case(quote!(unsafe fn value() -> Option<u32> {}), "Option or Result"; "optional value")]
    #[test_case::test_case(quote!(unsafe fn value() -> Result<u32, ()> {}), "Option or Result"; "fallible value")]
    fn rejects_unsupported_native_value_declarations(item: Tokens, message: &str) {
        assert!(expand_value(quote!(NativeValue, Value), item).unwrap_err().to_string().contains(message));
    }
}
