use crate::{
    common::{cfg_attributes, fresh_local, resolver_path},
    docs::attach_native_documentation,
    signature::{
        concrete_signature, consume_native_parameter_metadata, native_type, normalized_mutability, parameter_type,
        validate_binding,
    },
};

use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote, ToTokens};
use syn::{FnArg, ItemFn, Pat, Path, Type};

struct BindingResolver {
    backend: Tokens,
    descriptor: Tokens,
    declaration: Option<Path>,
}

impl BindingResolver {
    fn documentation_key(&self, fallback: &str) -> String {
        let name = self
            .declaration
            .as_ref()
            .map(|path| path.segments.last().unwrap().ident.to_string())
            .unwrap_or_else(|| fallback.to_owned());

        format!("binding-{name}")
    }
}

impl syn::parse::Parse for BindingResolver {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let path: Path = input.parse()?;

        if input.is_empty() {
            let path = resolver_path(quote!(#path))?;

            Ok(Self {
                backend: quote!(<#path as ::unchained_macros_core::ResolverDeclaration>::Backend),
                descriptor: quote!(<#path as ::unchained_macros_core::ResolverDeclaration>::DESCRIPTOR),
                declaration: Some(path),
            })
        } else {
            input.parse::<syn::Token![,]>()?;

            let descriptor: syn::Expr = input.parse()?;

            Ok(Self {
                backend: quote!(#path),
                descriptor: quote!(#descriptor),
                declaration: None,
            })
        }
    }
}

pub(crate) fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let resolver: BindingResolver = syn::parse2(args)?;

    let function: ItemFn = syn::parse2(item)?;

    validate_binding(&function)?;

    let binding = format_ident!("__UNCHAINED_BINDING", span = Span::mixed_site());

    let storage = binding_storage(&binding, &resolver, &function.sig.ident.to_string(), &[]);

    let documentation = syn::LitStr::new(
        &format!(
            "Native binding `{}`. Direct calls invoke the original implementation, bypassing detours.",
            function.sig.ident
        ),
        function.sig.ident.span(),
    );

    let key = resolver.documentation_key(&function.sig.ident.to_string());

    let expanded = call_wrapper(&function, &resolver, &binding, storage, &documentation, &key)?;

    Ok(quote!(#expanded))
}

fn binding_storage(
    name: &syn::Ident,
    resolver: &BindingResolver,
    target_name: &str,
    attributes: &[syn::Attribute],
) -> Tokens {
    let backend = &resolver.backend;

    let descriptor = &resolver.descriptor;

    let cfg: Vec<_> = cfg_attributes(attributes).collect();

    quote! {
        #(#cfg)*
        #[allow(non_upper_case_globals)]
        static #name: ::unchained_macros_core::FunctionBinding<#backend> = {
            ::unchained_macros_core::__private::inventory::submit! {
                &#name as &dyn ::unchained_macros_core::__private::FunctionInfo
            }
            ::unchained_macros_core::FunctionBinding::new(
                #target_name, #descriptor,
            )
        };
    }
}

fn call_wrapper(
    function: &ItemFn,
    resolver: &BindingResolver,
    binding: &syn::Ident,
    setup: Tokens,
    documentation: &syn::LitStr,
    documentation_key: &str,
) -> syn::Result<ItemFn> {
    let resolver_name = resolver
        .declaration
        .as_ref()
        .map(|path| path.segments.last().unwrap().ident.to_string())
        .unwrap_or_else(|| function.sig.ident.to_string());

    let mut expanded = function.clone();

    consume_native_parameter_metadata(&mut expanded.sig)?;

    let signature = &expanded.sig;

    let (arguments, types): (Vec<_>, Vec<_>) = signature
        .inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Receiver(receiver) => (quote!(self), &receiver.ty),
            FnArg::Typed(argument) => {
                let Pat::Ident(name) = argument.pat.as_ref() else {
                    unreachable!()
                };

                let name = &name.ident;

                (quote!(#name), &argument.ty)
            }
        })
        .unzip();

    let output = &signature.output;

    let native = fresh_local(signature, "native_call");

    expanded.block = Box::new(syn::parse2(quote!({
        #setup
        let #native: unsafe extern "C" fn(#(#types),*) #output = unsafe {
            ::core::mem::transmute(#binding.address()
                .expect(concat!("Native binding ", #resolver_name, " is not initialized")).get())
        };

        unsafe { #native(#(#arguments),*) }
    }))?);

    if !expanded
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("inline"))
    {
        expanded.attrs.push(syn::parse_quote!(#[inline(always)]));
    }

    attach_native_documentation(&mut expanded.attrs, documentation_key, documentation);

    Ok(expanded)
}

fn binding_documentation(function: &ItemFn, self_ty: &Type, resolver: &BindingResolver) -> syn::LitStr {
    let name = &function.sig.ident;

    let native_name = format!("{}::{name}", self_ty.to_token_stream().to_string().replace(' ', ""));

    let mut documentation = format!("Native binding to [`{native_name}`](Self::{name}).");

    if let Some(path) = &resolver.declaration {
        let key = &path.segments.last().unwrap().ident;

        let path = path.to_token_stream().to_string().replace(' ', "");

        documentation.push_str(&format!("\nResolved by [`{key}`]({path})."));
    }

    syn::LitStr::new(&documentation, name.span())
}

pub(crate) fn expand_method(
    args: Tokens,
    function: &ItemFn,
    self_ty: &Type,
) -> syn::Result<(ItemFn, Vec<syn::ImplItemConst>, Tokens)> {
    validate_binding(function)?;

    let resolver: BindingResolver = syn::parse2(args)?;

    let documentation = binding_documentation(function, self_ty, &resolver);

    let signature = concrete_signature(function, self_ty);

    let normalized = native_type(&normalized_mutability(&signature))?;

    let parameters = (0..signature.inputs.len())
        .map(|index| parameter_type(&signature, index))
        .collect::<syn::Result<Vec<_>>>()?;

    let parameter_values = parameters.iter().map(|_| quote!(::core::marker::PhantomData));

    let name = &function.sig.ident;

    let normalized_descriptor = format_ident!("__UNCHAINED_MUTABILITY_{}", name, span = Span::mixed_site());

    let parameter_descriptor = format_ident!("__UNCHAINED_PARAMETERS_{}", name, span = Span::mixed_site());

    let storage = format_ident!("__UNCHAINED_NATIVE_BINDING_{}", name, span = Span::mixed_site());

    let target_descriptor = format_ident!("__UNCHAINED_TARGET_{}", name, span = Span::mixed_site());

    let vis = &function.vis;

    let cfg: Vec<_> = cfg_attributes(&function.attrs).collect();

    let target_name = format!(
        "{}::{}",
        self_ty.to_token_stream().to_string().replace(' ', ""),
        function.sig.ident
    );

    // Match the native-doc generator's owner/member key, independent of the backend.
    let owner = match self_ty {
        Type::Path(path) => path.path.segments.last().unwrap().ident.to_string(),
        _ => self_ty.to_token_stream().to_string().replace(' ', ""),
    };

    let key = resolver.documentation_key(&format!("{owner}__{name}"));

    let expanded = call_wrapper(function, &resolver, &storage, quote!(), &documentation, &key)?;

    let registration = binding_storage(&storage, &resolver, &target_name, &function.attrs);

    let (target_type, target_storage, target_setup) =
        native_dispatch(&signature, self_ty, &storage, &resolver, &function.attrs)?;

    let registration = quote!(#registration #target_setup);

    let descriptors = [
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
        quote! {
            #(#cfg)*
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #vis const #target_descriptor: &'static #target_type = &#target_storage;
        },
    ]
    .into_iter()
    .map(syn::parse2)
    .collect::<syn::Result<Vec<_>>>()?;

    Ok((expanded, descriptors, registration))
}

fn native_dispatch(
    signature: &syn::Signature,
    owner: &Type,
    binding: &syn::Ident,
    resolver: &BindingResolver,
    attrs: &[syn::Attribute],
) -> syn::Result<(Tokens, syn::Ident, Tokens)> {
    let name = &signature.ident;

    let target = format_ident!("__UNCHAINED_NATIVE_TARGET_{}", name);

    let dispatch = format_ident!("__unchained_dispatch_{}", name);

    let backend = &resolver.backend;

    let cfg: Vec<_> = cfg_attributes(attrs).collect();

    let (names, types): (Vec<_>, Vec<_>) = signature
        .inputs
        .iter()
        .map(|input| {
            let FnArg::Typed(arg) = input else { unreachable!() };

            let Pat::Ident(pat) = &*arg.pat else { unreachable!() };

            (&pat.ident, &arg.ty)
        })
        .unzip();

    let lifetimes: Vec<_> = signature.generics.lifetimes().map(|p| &p.lifetime).collect();

    let binder = (!lifetimes.is_empty()).then(|| quote!(for<#(#lifetimes),*>));

    let result: Type = match &signature.output {
        syn::ReturnType::Default => syn::parse_quote!(()),
        syn::ReturnType::Type(_, ty) => *ty.clone(),
    };

    let entry_type = quote!(#binder unsafe fn(#(&#types),*));

    let exit_type = quote!(#binder unsafe fn(#(&#types,)* &#result));

    let prepare_type = quote!(#binder unsafe fn(#(&#types),*) -> ::unchained_macros_core::ExitObservation);

    let full_native = native_type(signature)?;

    let descriptor_type = quote!(::unchained_macros_core::NativeTarget<
        #full_native, #entry_type, #exit_type, #prepare_type, #backend
    >);

    let output = &signature.output;

    let generics = &signature.generics;

    let callback = fresh_local(signature, "callback");

    let prepare = fresh_local(signature, "prepare");

    let observation = fresh_local(signature, "observation");

    let call = fresh_local(signature, "call");

    let returned = fresh_local(signature, "result");

    let setup = quote! {
        #(#cfg)*
        #[allow(non_upper_case_globals)]
        static #target: #descriptor_type = unsafe {
            ::unchained_macros_core::NativeTarget::new(
                &#binding, #dispatch, |dispatch| dispatch as *const (),
            )
        };

        #(#cfg)*
        #[allow(non_snake_case)]
        unsafe extern "C" fn #dispatch #generics (#(#names: #types),*) #output {
            use ::unchained_macros_core::{SupportsEntry as _, SupportsExit as _, SupportsDetour as _};
            #target.observe_entry(|#callback| unsafe { #callback(#(&#names),*) });
            let #observation = #target.prepare_exit(|#prepare| unsafe { #prepare(#(&#names),*) });
            let #returned = match #target.replacement_slot().get() {
                Some(#call) => unsafe { #call(#(#names),*) },
                None => unsafe { <#owner>::#name(#(#names),*) },
            };
            if let Some(#observation) = #observation {
                unsafe { #target.finish_exit(#observation, (&#returned as *const #result).cast()); }
            }

            #returned
        }
    };

    Ok((descriptor_type, target, setup))
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
        assert!(expand(
            quote!(Release<u8>),
            quote!(
                unsafe fn release() {}
            )
        )
        .is_err());

        assert!(crate::value::expand(
            quote!(NativeValue<u8>, Value),
            quote!(
                unsafe fn value() -> u32 {}
            )
        )
        .is_err());
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

        let (binding, descriptors, _) =
            expand_method(quote!(self::Native__Call), &function, &syn::parse_quote!(Native)).unwrap();

        let docs = documentation(&binding.attrs);

        assert!(descriptors
            .iter()
            .all(|descriptor| documentation(&descriptor.attrs).is_empty()));

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

        let (binding, _, _) = expand_method(quote!(OtherBackend__Call), &function, &syn::parse_quote!(Client)).unwrap();

        let docs = documentation(&binding.attrs);

        assert!(docs.contains("Resolved by [`OtherBackend__Call`](OtherBackend__Call)."));
    }

    #[test]

    fn descriptor_binding_shares_typed_generation_without_resolver_hover_links() {
        let function = syn::parse_quote! {
            /// Keep the native safety contract.
            pub unsafe fn Call<'a>(value: &'a mut i32) -> &'a mut i32 {}
        };

        let (binding, metadata, storage) = expand_method(
            quote!(backend::Native, &DESCRIPTOR),
            &function,
            &syn::parse_quote!(Client),
        )
        .unwrap();

        let docs = documentation(&binding.attrs);

        assert!(docs.starts_with("Native binding to [`Client::Call`](Self::Call)."));

        assert!(docs.contains("Keep the native safety contract."));

        assert!(!docs.contains("Resolved by"));

        assert!(!docs.contains("DESCRIPTOR"));

        assert_eq!(metadata.len(), 3);

        let expanded = quote!(#binding #(#metadata)* #storage).to_string();

        assert!(expanded.contains("for < 'a > unsafe extern \"C\" fn"));

        assert!(expanded.contains("backend :: Native"));

        assert!(expanded.contains("& DESCRIPTOR"));

        assert!(!expanded.contains("ResolverDeclaration"));

        assert!(!expanded.contains("ResolverSelection"));

        assert!(expand(
            quote!(backend::Native, &DESCRIPTOR),
            quote!(
                unsafe fn call() {}
            )
        )
        .is_ok());
    }
}
