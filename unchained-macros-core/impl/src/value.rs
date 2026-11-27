use crate::common::resolver_path;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::quote;
use syn::{ItemFn, Path, ReturnType, Type};

pub(crate) fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    use syn::parse::Parser;

    let args = syn::punctuated::Punctuated::<Path, syn::Token![,]>::parse_terminated.parse2(args)?;

    let mut args = args.into_iter();

    let resolver = args
        .next()
        .ok_or_else(|| syn::Error::new(Span::call_site(), "expected resolver path"))?;

    let resolver = resolver_path(quote!(#resolver))?;

    let address = match args.next() {
        Some(mode) if mode.is_ident("Value") => false,
        Some(mode) if mode.is_ident("Address") => true,
        Some(mode) => return Err(syn::Error::new_spanned(mode, "expected Value or Address binding mode")),
        None => {
            return Err(syn::Error::new(
                Span::call_site(),
                "explicit Value or Address binding mode is required",
            ))
        }
    };

    if args.next().is_some() {
        return Err(syn::Error::new(
            Span::call_site(),
            "unexpected native value binding arguments",
        ));
    }

    let resolver_name = resolver.segments.last().unwrap().ident.to_string();

    if let Ok(declaration) = syn::parse2::<syn::ForeignItemStatic>(item.clone()) {
        if !matches!(declaration.mutability, syn::StaticMutability::None) {
            return Err(syn::Error::new_spanned(
                declaration,
                "native global bindings must not be static mut",
            ));
        }

        let Type::Ptr(pointer) = declaration.ty.as_ref() else {
            return Err(syn::Error::new_spanned(
                &declaration.ty,
                "static global bindings require a raw native pointer type",
            ));
        };

        if pointer.mutability.is_none() {
            return Err(syn::Error::new_spanned(
                pointer,
                "static native globals require *mut T; use an unsafe accessor for read-only values",
            ));
        }

        let target = &pointer.elem;

        let name = &declaration.ident;

        let vis = &declaration.vis;

        let attributes = &declaration.attrs;

        let constructor = if address { quote!(inline) } else { quote!(pointer_slot) };

        return Ok(quote! {
            #(#attributes)*
            #[allow(non_upper_case_globals)]
            #vis static #name: ::unchained_macros_core::NativeGlobal<#target> = {
                let _: ::core::marker::PhantomData<#resolver> = ::core::marker::PhantomData;
                static BINDING: ::unchained_macros_core::ValueBinding =
                    ::unchained_macros_core::ValueBinding::new(#resolver_name);
                ::unchained_macros_core::__private::inventory::submit! {
                    &BINDING as &'static ::unchained_macros_core::ValueBinding
                }

                ::unchained_macros_core::NativeGlobal::#constructor(&BINDING)
            };
        });
    }

    let mut function: ItemFn = syn::parse2(item)?;

    let signature = &function.sig;

    if signature.unsafety.is_none() {
        return Err(syn::Error::new_spanned(
            signature,
            "native value bindings require unsafe fn",
        ));
    }

    if !signature.inputs.is_empty()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.asyncness.is_some()
        || signature.constness.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
    {
        return Err(syn::Error::new_spanned(
            signature,
            "native value bindings require a non-generic, non-async zero-argument Rust accessor",
        ));
    }

    if !function.block.stmts.is_empty() {
        return Err(syn::Error::new_spanned(
            &function.block,
            "native binding bodies must be empty",
        ));
    }

    let ReturnType::Type(_, ty) = &signature.output else {
        return Err(syn::Error::new_spanned(
            signature,
            "native value bindings require an explicit Copy value return type",
        ));
    };

    if matches!(ty.as_ref(), Type::Reference(_))
        || matches!(ty.as_ref(), Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Option" || segment.ident == "Result"))
    {
        return Err(syn::Error::new_spanned(
            ty,
            "return the native Copy value directly, not a reference, Option or Result",
        ));
    }

    if address && !matches!(ty.as_ref(), Type::Ptr(_)) {
        return Err(syn::Error::new_spanned(
            ty,
            "Address bindings require a raw pointer return type",
        ));
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
        static #binding: ::unchained_macros_core::ValueBinding =
            ::unchained_macros_core::ValueBinding::new(#resolver_name);
        ::unchained_macros_core::__private::inventory::submit! {
            &#binding as &'static ::unchained_macros_core::ValueBinding
        }

        fn __require_copy<T: ::core::marker::Copy>() {}
        __require_copy::<#ty>();
        let #value: #ty = #read;
        #check_pointer
        #value
    }))?);

    if !function
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("inline"))
    {
        function.attrs.push(syn::parse_quote!(#[inline(always)]));
    }

    Ok(quote!(#function))
}

#[cfg(test)]

mod tests {

    use super::*;

    #[test_case::test_case(quote!(Objects, Address), quote!(unsafe fn objects() -> u32 {}), "raw pointer return type"; "scalar address")]
    #[test_case::test_case(quote!(Objects, unknown), quote!(unsafe fn objects() -> *const u32 {}), "expected Value or Address"; "unknown mode")]
    #[test_case::test_case(quote!(Objects, Address, Address), quote!(unsafe fn objects() -> *const u32 {}), "unexpected native value binding arguments"; "extra mode")]
    #[test_case::test_case(quote!(Objects), quote!(unsafe fn objects() -> *const u32 {}), "explicit Value or Address"; "missing mode")]
    #[test_case::test_case(quote!(Objects, address), quote!(unsafe fn objects() -> *const u32 {}), "expected Value or Address"; "lowercase mode")]

    fn rejects_invalid_value_binding_modes(args: Tokens, item: Tokens, message: &str) {
        let error = expand(args, item).unwrap_err();

        assert!(error.to_string().contains(message), "{error}");
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
        assert!(expand(quote!(NativeValue, Value), item)
            .unwrap_err()
            .to_string()
            .contains(message));
    }
}
