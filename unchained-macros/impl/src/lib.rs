use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::quote;
use syn::{FnArg, GenericParam, ItemFn, Pat, Path, Type};

/// Bind an unsafe Rust method/function to a separately declared named resolver.
///
/// The empty body is replaced by a direct `extern "C"` call. Receiver types
/// and declared per-call lifetime relationships are preserved.
#[proc_macro_attribute]
pub fn bind_resolver_func(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}

fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let resolver: Path = syn::parse2(args)?;
    if resolver.segments.iter().any(|segment| !segment.arguments.is_empty()) {
        return Err(syn::Error::new_spanned(&resolver, "resolver paths cannot have generic arguments"));
    }
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
    function.block = Box::new(syn::parse2(quote!({
        // Check that the resolver path names a type.
        let _: ::core::marker::PhantomData<#resolver> = ::core::marker::PhantomData;
        static #binding: crate::resolvers::FunctionBinding =
            crate::resolvers::FunctionBinding::new(#resolver_name);
        ::inventory::submit! {
            crate::resolvers::FunctionBindingRegistration { binding: &#binding }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
