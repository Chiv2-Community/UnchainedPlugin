use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote, ToTokens};
use syn::{FnArg, ItemFn, Pat, Path, Type};
use crate::{
    common::{cfg_attributes, resolver_path},
    docs::attach_native_documentation,
    signature::{validate_binding, consume_native_parameter_metadata, concrete_signature,
        native_type, normalized_mutability, parameter_type},
};

pub(crate) fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let resolver = resolver_path(args)?;
    let function: ItemFn = syn::parse2(item)?;
    validate_binding(&function)?;
    let binding = format_ident!("__UNCHAINED_BINDING", span = Span::mixed_site());
    let storage = binding_storage(&binding, &resolver, &function.sig.ident.to_string(), &[]);
    let documentation = syn::LitStr::new(
        &format!("Native binding `{}`. Direct calls invoke the original implementation, bypassing detours.", function.sig.ident),
        function.sig.ident.span(),
    );
    let expanded = call_wrapper(&function, &resolver, &binding, storage, &documentation)?;
    Ok(quote!(#expanded))
}

fn binding_storage(
    name: &syn::Ident, resolver: &Path, target_name: &str, attributes: &[syn::Attribute],
) -> Tokens {
    let backend = quote!(<#resolver as ::unchained_macros_core::ResolverDeclaration>::Backend);
    let cfg: Vec<_> = cfg_attributes(attributes).collect();
    quote! {
        #(#cfg)*
        #[allow(non_upper_case_globals)]
        static #name: ::unchained_macros_core::FunctionBinding<#backend> =
            ::unchained_macros_core::FunctionBinding::new(
                #target_name, <#resolver as ::unchained_macros_core::ResolverDeclaration>::DESCRIPTOR,
            );
        #(#cfg)*
        ::unchained_macros_core::__private::inventory::submit! {
            ::unchained_macros_core::__private::ErasedFunctionBindingRegistration::new(
                &::unchained_macros_core::FunctionBindingRegistration { binding: &#name })
        }
    }
}

fn call_wrapper(
    function: &ItemFn, resolver: &Path, binding: &syn::Ident, setup: Tokens, documentation: &syn::LitStr,
) -> syn::Result<ItemFn> {
    let resolver_name = resolver.segments.last().unwrap().ident.to_string();
    let mut expanded = function.clone();
    consume_native_parameter_metadata(&mut expanded.sig)?;
    let signature = &expanded.sig;
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
    let native = format_ident!("__unchained_native_call", span = Span::mixed_site());
    expanded.block = Box::new(syn::parse2(quote!({
        #setup
        let #native: unsafe extern "C" fn(#(#types),*) #output = unsafe {
            ::core::mem::transmute(#binding.address()
                .expect(concat!("Native binding ", #resolver_name, " is not initialized")).get())
        };
        unsafe { #native(#(#arguments),*) }
    }))?);
    if !expanded.attrs.iter().any(|attribute| attribute.path().is_ident("inline")) {
        expanded.attrs.push(syn::parse_quote!(#[inline(always)]));
    }
    attach_native_documentation(&mut expanded.attrs, &format!("binding-{resolver_name}"), documentation);
    Ok(expanded)
}

fn binding_documentation(
    function: &ItemFn,
    self_ty: &Type,
    resolver: &Path,
) -> syn::LitStr {
    let name = &function.sig.ident;
    let native_name = format!("{}::{name}", self_ty.to_token_stream().to_string().replace(' ', ""));
    let key = &resolver.segments.last().unwrap().ident;
    let path = resolver.to_token_stream().to_string().replace(' ', "");
    let resolution = format!("[`{key}`]({path})");
    let documentation = format!(
        "Native binding to [`{native_name}`](Self::{name}).\n\
         Resolved by {resolution}."
    );
    syn::LitStr::new(&documentation, name.span())
}

pub(crate) fn expand_method(
    args: Tokens,
    function: &ItemFn,
    self_ty: &Type,
) -> syn::Result<(ItemFn, Vec<syn::ImplItemConst>, Tokens)> {
    validate_binding(function)?;
    let resolver = resolver_path(args)?;
    let documentation = binding_documentation(function, self_ty, &resolver);
    let signature = concrete_signature(function, Some(self_ty))?;
    let native = native_type(&signature)?;
    let normalized = native_type(&normalized_mutability(&signature))?;
    let parameters = (0..signature.inputs.len())
        .map(|index| parameter_type(&signature, index))
        .collect::<syn::Result<Vec<_>>>()?;
    let parameter_values = parameters.iter().map(|_| quote!(::core::marker::PhantomData));
    let name = &function.sig.ident;
    let descriptor = format_ident!("__UNCHAINED_BINDING_{}", name, span = Span::mixed_site());
    let normalized_descriptor = format_ident!("__UNCHAINED_MUTABILITY_{}", name, span = Span::mixed_site());
    let parameter_descriptor = format_ident!("__UNCHAINED_PARAMETERS_{}", name, span = Span::mixed_site());
    let storage = format_ident!("__UNCHAINED_NATIVE_BINDING_{}", name, span = Span::mixed_site());
    let vis = &function.vis;
    let cfg: Vec<_> = cfg_attributes(&function.attrs).collect();
    let target_name = format!("{}::{}", self_ty.to_token_stream().to_string().replace(' ', ""), function.sig.ident);
    let backend = quote!(<#resolver as ::unchained_macros_core::ResolverDeclaration>::Backend);
    let expanded = call_wrapper(function, &resolver, &storage, quote!(), &documentation)?;
    let registration = binding_storage(&storage, &resolver, &target_name, &function.attrs);
    let descriptors = [
        quote! {
            #(#cfg)*
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #vis const #descriptor: ::unchained_macros_core::NativeFunction<#native, #backend> =
                unsafe { ::unchained_macros_core::NativeFunction::new(&#storage) };
        },
        quote! {
            #(#cfg)*
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #vis const #normalized_descriptor: ::unchained_macros_core::NativeSignature<#normalized> =
                ::core::marker::PhantomData;
        },
        quote! {
            #(#cfg)*
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #vis const #parameter_descriptor: (#(::unchained_macros_core::NativeSignature<#parameters>,)*) =
                (#(#parameter_values,)*);
        },
    ].into_iter().map(syn::parse2).collect::<syn::Result<Vec<_>>>()?;
    Ok((expanded, descriptors, registration))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::documentation;
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
        assert!(crate::value::expand(quote!(NativeValue<u8>, Value), quote!(unsafe fn value() -> u32 {})).is_err());
    }

    #[test]
    fn binding_hover_identifies_native_method_and_resolver_without_assuming_a_detour() {
        let function = syn::parse_quote! {
            /// Existing native safety note.
            pub(crate) unsafe fn Call<'a>(
                self: *const Self,
                Result: &'a mut FString,
            ) -> &'a mut FString {}
        };
        let (binding, descriptors, _) = expand_method(
            quote!(self::Native__Call), &function, &syn::parse_quote!(Native),
        ).unwrap();
        let docs = documentation(&binding.attrs);
        assert!(descriptors.iter().all(|descriptor| documentation(&descriptor.attrs).is_empty()));
        assert!(docs.starts_with("Native binding to [`Native::Call`](Self::Call)."));
        assert!(docs.contains("](Self::Call).\nResolved by "));
        assert!(docs.contains("Resolved by [`Native__Call`](self::Native__Call)."));
        assert!(!docs.contains("detours"));
        assert!(docs.contains("Existing native safety note."));
        assert!(!docs.contains("impl Native"));
        assert!(!docs.contains("Overrides in-game calls"));
    }

    #[test]
    fn binding_hover_links_an_arbitrary_resolver_declaration() {
        let function = syn::parse_quote! {
            pub unsafe fn Call(value: i32) -> i32 {}
        };
        let (binding, _, _) = expand_method(
            quote!(OtherBackend__Call), &function, &syn::parse_quote!(Client),
        ).unwrap();
        let docs = documentation(&binding.attrs);
        assert!(docs.contains("Resolved by [`OtherBackend__Call`](OtherBackend__Call)."));
    }
}
