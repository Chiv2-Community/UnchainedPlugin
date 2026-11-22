use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{FnArg, GenericParam, Pat};
mod resolver;

#[proc_macro]
pub fn define_pattern_resolver(input: TokenStream) -> TokenStream {
    resolver::expand(input.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}

fn key(args: Tokens) -> syn::Result<syn::Ident> {
    let path: syn::Path = syn::parse2(args)?;
    path.get_ident().filter(|name| name.to_string().strip_prefix("GetPost_").is_some_and(|suffix| !suffix.is_empty()))
        .cloned().ok_or_else(|| syn::Error::new_spanned(path, "request function keys must be unqualified GetPost_* identifiers"))
}

fn declaration(name: &syn::Ident) -> Tokens {
    quote! {
        #[allow(non_camel_case_types)]
        pub struct #name;
        impl ::unchained_macros_core::ResolverDeclaration for #name {
            type Backend = ::unchained_macros_patternsleuth::PatternsleuthResolver;
            const NAME: &'static str = stringify!(#name);
            const UNRESTRICTED: bool = true;
            const DESCRIPTOR: &'static ::unchained_macros_core::Resolver<Self::Backend> = {
                static DESCRIPTOR: ::unchained_macros_core::Resolver<::unchained_macros_patternsleuth::PatternsleuthResolver> =
                    ::unchained_macros_core::Resolver {
                        name: stringify!(#name),
                        implementation: ::unchained_macros_patternsleuth::PatternsleuthResolver::offset(stringify!(#name)),
                        filter: None,
                    };
                &DESCRIPTOR
            };
        }
    }
}

/// Compatibility adapter for provider-offset methods; core only sees typed resolvers.
#[proc_macro_attribute]
pub fn runtime_bindings(args: TokenStream, item: TokenStream) -> TokenStream {
    wrap_runtime(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}
fn wrap_runtime(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let mut declaration_item: syn::Item = syn::parse2(item)?;
    let mut resolvers = Vec::new();
    if let syn::Item::Impl(implementation) = &mut declaration_item {
        for item in &mut implementation.items {
            let syn::ImplItem::Fn(method) = item else { continue };
            let Some(index) = method.attrs.iter().position(|attribute|
                attribute.path().segments.last().is_some_and(|segment| segment.ident == "bind_resolved_request_function")) else { continue };
            let attribute = method.attrs.remove(index);
            let name = key(attribute.meta.require_list()?.tokens.clone())?;
            let emitted = declaration(&name);
            let cfg: Vec<_> = implementation.attrs.iter().chain(method.attrs.iter())
                .filter(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")).collect();
            // Isolate the type per binding, including repeated offset keys and cfg gates.
            let hidden = quote::format_ident!("__UNCHAINED_REQUEST_{}", method.sig.ident);
            // The enclosing private module keeps the original key/type spelling.
            resolvers.push(quote! {
                #(#cfg)*
                #[allow(non_snake_case)]
                mod #hidden { #emitted }
            });
            method.attrs.push(syn::parse_quote!(#[bind_resolved_func(#hidden::#name)]));
        }
    }
    Ok(quote! {
        #(#resolvers)*
        #[::unchained_macros_core::runtime_bindings(#args)]
        #declaration_item
    })
}

#[proc_macro_attribute]
pub fn bind_resolved_request_function(args: TokenStream, item: TokenStream) -> TokenStream {
    standalone(args.into(), item.into()).unwrap_or_else(|error| error.to_compile_error()).into()
}
fn standalone(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let name = key(args)?;
    let mut function: syn::ItemFn = syn::parse2(item)?;
    for input in &mut function.sig.inputs {
        let FnArg::Typed(argument) = input else { continue };
        let annotations: Vec<_> = argument.attrs.iter().filter(|attribute| attribute.path().is_ident("native_by_value")).collect();
        if !annotations.is_empty() {
            if annotations.len() != 1 || !matches!(annotations[0].meta, syn::Meta::Path(_))
                || !matches!(argument.ty.as_ref(), syn::Type::Ptr(pointer) if pointer.mutability.is_some())
            {
                return Err(syn::Error::new_spanned(annotations[0], "#[native_by_value] requires verified *mut T ABI storage"));
            }
            argument.attrs.retain(|attribute| !attribute.path().is_ident("native_by_value"));
        }
    }
    let signature = &function.sig;
    if signature.unsafety.is_none() || signature.asyncness.is_some() || signature.constness.is_some()
        || signature.abi.is_some() || signature.variadic.is_some() || signature.generics.where_clause.is_some()
        || signature.generics.params.iter().any(|parameter| !matches!(parameter, GenericParam::Lifetime(_)))
        || !function.block.stmts.is_empty()
    {
        return Err(syn::Error::new_spanned(signature, "request bindings require empty unsafe native declarations"));
    }
    let mut arguments = Vec::new();
    let mut types = Vec::new();
    for input in &signature.inputs {
        match input {
            FnArg::Receiver(receiver) => {
                let element = match receiver.ty.as_ref() {
                    syn::Type::Reference(reference) => reference.elem.as_ref(),
                    syn::Type::Ptr(pointer) => pointer.elem.as_ref(),
                    _ => return Err(syn::Error::new_spanned(receiver, "use a borrowed or raw-pointer Self receiver")),
                };
                if !matches!(element, syn::Type::Path(path) if path.path.is_ident("Self")) {
                    return Err(syn::Error::new_spanned(receiver, "native receivers must point to Self"));
                }
                arguments.push(quote!(self));
                let ty = &receiver.ty;
                types.push(quote!(#ty));
            }
            FnArg::Typed(argument) => {
                let Pat::Ident(identifier) = argument.pat.as_ref() else {
                    return Err(syn::Error::new_spanned(argument, "native parameters must use plain identifiers"));
                };
                if identifier.by_ref.is_some() || identifier.subpat.is_some() {
                    return Err(syn::Error::new_spanned(identifier, "native parameters must use plain identifiers"));
                }
                let identifier = &identifier.ident;
                let ty = &argument.ty;
                arguments.push(quote!(#identifier));
                types.push(quote!(#ty));
            }
        }
    }
    let output = &signature.output;
    let native_name = signature.ident.to_string();
    let resolver = declaration(&name);
    function.block = Box::new(syn::parse2(quote!({
        #resolver
        static BINDING: ::unchained_macros_core::FunctionBinding<::unchained_macros_patternsleuth::PatternsleuthResolver> =
            ::unchained_macros_core::FunctionBinding::new(#native_name, <#name as ::unchained_macros_core::ResolverDeclaration>::DESCRIPTOR);
        ::unchained_macros_core::__private::inventory::submit! {
            ::unchained_macros_core::__private::ErasedFunctionBindingRegistration::new(
                &::unchained_macros_core::FunctionBindingRegistration { binding: &BINDING })
        }
        let native: unsafe extern "C" fn(#(#types),*) #output = unsafe {
            ::core::mem::transmute(BINDING.address().expect(concat!("Native binding ", stringify!(#name), " is not initialized")).get())
        };
        unsafe { native(#(#arguments),*) }
    }))?);
    Ok(quote!(#function))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test_case::test_case(quote!(GetPost_); "empty suffix")]
    #[test_case::test_case(quote!(CreateHttpRequest); "wrong prefix")]
    #[test_case::test_case(quote!(module::GetPost_GetMotd); "qualified")]
    #[test_case::test_case(quote!("GetPost_GetMotd"); "literal")]
    #[test_case::test_case(quote!(GetPost_GetMotd<u8>); "generic")]
    fn request_bindings_reject_non_request_keys(value: Tokens) {
        assert!(standalone(value, quote!(pub unsafe fn request() {})).is_err());
    }
}
