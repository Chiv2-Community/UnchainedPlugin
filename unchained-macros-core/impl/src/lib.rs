use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::quote;
use syn::{FnArg, GenericParam, ItemFn, Pat, Path, ReturnType, Type};
mod bindings;
mod function_detour;
mod global;
mod selection;
mod patch;

/// Register a `fn(u64) -> Patch` factory or immutable `Patch` static.
/// An optional enum variant restricts applicability. Required coverage is grouped
/// by resolver and excludes selections explicitly disabled by that resolver.
#[proc_macro_attribute]
pub fn register_patch(args: TokenStream, item: TokenStream) -> TokenStream {
    patch::expand(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}

/// Declare a fieldless selection enum with optional `require = [Variant, ...]`
/// and `fallback = Variant`. The fallback variant itself terminates the chain.
/// Supplies `SelectionKey`, `Clone`, `Copy`, `PartialEq`, and `Eq`.
#[proc_macro_attribute]
pub fn resolver_selection_key(args: TokenStream, item: TokenStream) -> TokenStream {
    selection::expand(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}

/// Bind an unsafe Rust method/function to a separately declared named resolver.
///
/// The empty body calls the native entry or its original trampoline. Receiver
/// types and declared per-call lifetime relationships are preserved.
#[proc_macro_attribute]
pub fn bind_resolved_func(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
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

/// Mark a native type for source documentation, or expose bindings in a concrete impl.
///
/// Type annotations add documentation only unless `global` opts into static
/// forwarding from the existing vtable or public inherent method declarations.
#[proc_macro_attribute]
pub fn runtime_bindings(args: TokenStream, item: TokenStream) -> TokenStream {
    expand_runtime_bindings(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error()).into()
}

fn expand_runtime_bindings(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    if args.to_string() == "global" {
        if matches!(syn::parse2::<syn::Item>(item.clone())?, syn::Item::Struct(_)) {
            let item = expand_runtime_bindings(Tokens::new(), item)?;
            return global::expand(item);
        }
        return global::expand(item);
    }
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(args, "runtime_bindings takes no arguments"));
    }
    let mut declaration: syn::Item = syn::parse2(item.clone())?;
    let (name, attributes) = match &mut declaration {
        syn::Item::Impl(_) => return bindings::expand_impl(args, item),
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

fn attach_native_documentation(attributes: &mut Vec<syn::Attribute>, key: &str, fallback: &syn::LitStr) {
    let generated = std::env::var_os("UNCHAINED_NATIVE_DOCS_DIR")
        .map(std::path::PathBuf::from)
        .map(|directory| directory.join(format!("{key}.md")))
        .filter(|path| path.is_file());
    if let Some(path) = generated {
        // The generated document already contains the declaration's safety/source notes.
        attributes.retain(|attribute| !matches!(&attribute.meta,
            syn::Meta::NameValue(value) if attribute.path().is_ident("doc")
                && matches!(&value.value, syn::Expr::Lit(value) if matches!(value.lit, syn::Lit::Str(_)))));
        let path = syn::LitStr::new(&path.to_string_lossy(), fallback.span());
        attributes.insert(0, syn::parse_quote!(#[doc = ::core::include_str!(#path)]));
    } else {
        attributes.insert(0, syn::parse_quote!(#[doc = #fallback]));
    }
}

fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    expand_binding(args, item)
}

fn cfg_attributes(attributes: &[syn::Attribute]) -> impl Iterator<Item = &syn::Attribute> {
    attributes.iter().filter(|attribute|
        attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

fn is_binding_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().segments.last().is_some_and(|segment|
        segment.ident == "bind_resolved_func")
}

fn validate_native_signature(signature: &syn::Signature) -> syn::Result<()> {
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
            }
            FnArg::Typed(argument) => {
                let Pat::Ident(name) = argument.pat.as_ref() else {
                    return Err(syn::Error::new_spanned(&argument.pat, "native parameters must use plain identifiers"));
                };
                if name.by_ref.is_some() || name.subpat.is_some() {
                    return Err(syn::Error::new_spanned(name, "native parameters must use plain identifiers"));
                }
            }
        }
    }
    Ok(())
}

fn validate_binding(function: &ItemFn) -> syn::Result<()> {
    validate_native_signature(&function.sig)?;
    if !function.block.stmts.is_empty() {
        return Err(syn::Error::new_spanned(&function.block, "native binding bodies must be empty"));
    }
    Ok(())
}

fn consume_native_parameter_metadata(signature: &mut syn::Signature) -> syn::Result<()> {
    for argument in &mut signature.inputs {
        let FnArg::Typed(argument) = argument else { continue };
        let annotations: Vec<_> = argument.attrs.iter()
            .filter(|attribute| attribute.path().is_ident("native_by_value")).collect();
        if annotations.is_empty() { continue; }
        if annotations.len() != 1 || !matches!(annotations[0].meta, syn::Meta::Path(_))
            || !matches!(argument.ty.as_ref(), Type::Ptr(pointer) if pointer.mutability.is_some())
        {
            return Err(syn::Error::new_spanned(annotations[0],
                "#[native_by_value] requires one bare annotation on verified *mut T ABI storage"));
        }
        argument.attrs.retain(|attribute| !attribute.path().is_ident("native_by_value"));
    }
    Ok(())
}

fn expand_binding(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let resolver = resolver_path(args)?;
    let resolver_name = resolver.segments.last().unwrap().ident.to_string();
    let mut function: ItemFn = syn::parse2(item)?;
    validate_binding(&function)?;
    consume_native_parameter_metadata(&mut function.sig)?;
    let signature = &function.sig;
    let (arguments, types): (Vec<_>, Vec<_>) = signature.inputs.iter().map(|argument| {
        match argument {
            FnArg::Receiver(receiver) => (quote!(self), &receiver.ty),
            FnArg::Typed(argument) => {
                let Pat::Ident(name) = argument.pat.as_ref() else { unreachable!() };
                let name = &name.ident;
                (quote!(#name), &argument.ty)
            }
        }
    }).unzip();
    let output = &signature.output;
    let binding = syn::Ident::new("__UNCHAINED_BINDING", Span::mixed_site());
    let native = syn::Ident::new("__unchained_native", Span::mixed_site());
    let resolver_type_check = quote! {
        let _: ::core::marker::PhantomData<#resolver> = ::core::marker::PhantomData;
    };
    let target_name = signature.ident.to_string();
    let backend = quote!(<#resolver as ::unchained_macros_core::ResolverDeclaration>::Backend);
    let initializer = quote!(::unchained_macros_core::FunctionBinding::new(
        #target_name, <#resolver as ::unchained_macros_core::ResolverDeclaration>::DESCRIPTOR,
    ));
    function.block = Box::new(syn::parse2(quote!({
        #resolver_type_check
        static #binding: ::unchained_macros_core::FunctionBinding<#backend> =
            #initializer;
        ::inventory::submit! {
            ::unchained_macros_core::__private::ErasedFunctionBindingRegistration::new(
                &::unchained_macros_core::FunctionBindingRegistration { binding: &#binding })
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
    let documentation = syn::LitStr::new(
        &format!("Native binding `{}`. Direct calls invoke the original implementation, bypassing detours.", function.sig.ident),
        function.sig.ident.span(),
    );
    attach_native_documentation(&mut function.attrs, &format!("binding-{resolver_name}"), &documentation);
    Ok(quote!(#function))
}

fn resolver_path(args: Tokens) -> syn::Result<Path> {
    let resolver: Path = syn::parse2(args)?;
    if resolver.segments.iter().any(|segment| !segment.arguments.is_empty()) {
        return Err(syn::Error::new_spanned(&resolver, "resolver paths cannot have generic arguments"));
    }
    Ok(resolver)
}

/// Bind a native global as a static with generated unsafe method forwarding.
/// `Value` reloads the pointer slot on each call; `Address` uses inline storage.
/// Targets opt in with `runtime_bindings(global)`. Legacy scalar accessors remain
/// available; neither form borrows a mutable native pointer slot.
#[proc_macro_attribute]
pub fn bind_resolved_value(args: TokenStream, item: TokenStream) -> TokenStream {
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
    if let Ok(declaration) = syn::parse2::<syn::ForeignItemStatic>(item.clone()) {
        if !matches!(declaration.mutability, syn::StaticMutability::None) {
            return Err(syn::Error::new_spanned(declaration, "native global bindings must not be static mut"));
        }
        let Type::Ptr(pointer) = declaration.ty.as_ref() else {
            return Err(syn::Error::new_spanned(&declaration.ty, "static global bindings require a raw native pointer type"));
        };
        if pointer.mutability.is_none() {
            return Err(syn::Error::new_spanned(pointer, "static method forwarding requires *mut T; use an unsafe accessor for read-only values"));
        }
        let target = &pointer.elem;
        let name = &declaration.ident;
        let vis = &declaration.vis;
        let attributes = &declaration.attrs;
        let indirect = !address;
        return Ok(quote! {
            #(#attributes)*
            #[allow(non_upper_case_globals)]
            #vis static #name: <#target as ::unchained_macros_core::NativeGlobalTarget>::Global = {
                let _: ::core::marker::PhantomData<#resolver> = ::core::marker::PhantomData;
                static BINDING: ::unchained_macros_core::ValueBinding =
                    ::unchained_macros_core::ValueBinding::new(#resolver_name);
                ::inventory::submit! {
                    ::unchained_macros_core::ValueBindingRegistration { binding: &BINDING }
                }
                <#target as ::unchained_macros_core::NativeGlobalTarget>::Global::new(&BINDING, #indirect)
            };
        });
    }
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
        static #binding: ::unchained_macros_core::ValueBinding =
            ::unchained_macros_core::ValueBinding::new(#resolver_name);
        ::inventory::submit! {
            ::unchained_macros_core::ValueBindingRegistration { binding: &#binding }
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
    fn indirect_by_value_metadata_is_checked_and_consumed() {
        let mut signature: syn::Signature = syn::parse_quote! {
            unsafe fn Call(#[native_by_value] callback: *mut Callback)
        };
        super::consume_native_parameter_metadata(&mut signature).unwrap();
        let syn::FnArg::Typed(argument) = &signature.inputs[0] else { unreachable!() };
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

    #[test_case::test_case(quote!(#[repr(C)] pub struct Native<T, const N: usize> where T: Copy { value: [T; N] }); "struct")]
    #[test_case::test_case(quote!(#[repr(u8)] pub enum Native { First, Second }); "enum")]
    #[test_case::test_case(quote!(#[repr(C)] pub union Native<T: Copy> { value: T, raw: usize }); "union")]
    #[test_case::test_case(quote!(pub type Native<T> = *mut T;); "alias")]
    fn runtime_type_markers_preserve_declarations_and_generate_no_registration(tokens: Tokens) {
        use quote::ToTokens;
        let original: syn::Item = syn::parse2(tokens.clone()).unwrap();
        let expanded = super::expand_runtime_bindings(quote!(), tokens).unwrap();
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
    #[test_case::test_case(quote!(), quote!(fn helper() {}); "function")]
    #[test_case::test_case(quote!(), quote!(trait Native {}); "trait")]
    fn runtime_type_markers_reject_arguments_and_non_type_items(args: Tokens, item: Tokens) {
        assert!(super::expand_runtime_bindings(args, item).is_err());
    }

    #[test_case::test_matrix(
        [
            (quote!(fn release(&mut self) {}), "require unsafe"),
            (quote!(unsafe fn release(self) {}), "borrowed or raw-pointer"),
            (quote!(unsafe fn release(&mut self) { panic!() }), "must be empty"),
            (quote!(unsafe fn release<T>(&mut self, value: T) {}), "non-generic"),
            (quote!(async unsafe fn release(&mut self) {}), "non-async"),
            (quote!(unsafe fn release((a, b): (u8, u8)) {}), "plain identifiers"),
        ]
    )]
    fn rejects_unsupported_native_declarations((item, message): (Tokens, &str)) {
        let error = expand(quote!(Release), item).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }

    #[test]
    fn rejects_generic_resolver_paths() {
        assert!(expand(quote!(Release<u8>), quote!(unsafe fn release() {})).is_err());
        assert!(expand_value(quote!(NativeValue<u8>, Value), quote!(unsafe fn value() -> u32 {})).is_err());
    }

    #[test_case::test_case(quote!(Objects, Address), quote!(unsafe fn objects() -> u32 {}), "raw pointer return type"; "scalar address")]
    #[test_case::test_case(quote!(Objects, unknown), quote!(unsafe fn objects() -> *const u32 {}), "expected Value or Address"; "unknown mode")]
    #[test_case::test_case(quote!(Objects, Address, Address), quote!(unsafe fn objects() -> *const u32 {}), "unexpected native value binding arguments"; "extra mode")]
    #[test_case::test_case(quote!(Objects), quote!(unsafe fn objects() -> *const u32 {}), "explicit Value or Address"; "missing mode")]
    #[test_case::test_case(quote!(Objects, address), quote!(unsafe fn objects() -> *const u32 {}), "expected Value or Address"; "lowercase mode")]
    fn rejects_invalid_value_binding_modes(args: Tokens, item: Tokens, message: &str) {
        let error = expand_value(args, item).unwrap_err();
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
        assert!(expand_value(quote!(NativeValue, Value), item).unwrap_err().to_string().contains(message));
    }
}
