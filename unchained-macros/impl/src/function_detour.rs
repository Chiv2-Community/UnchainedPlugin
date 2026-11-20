use super::*;
use quote::{format_ident, ToTokens};

fn binding_documentation(
    function: &ItemFn,
    self_ty: &Type,
    resolver: &Path,
    require_resolver: bool,
) -> syn::Result<syn::LitStr> {
    let name = &function.sig.ident;
    let native_name = format!("{}::{name}", self_ty.to_token_stream().to_string().replace(' ', ""));
    let resolution = resolution_documentation(resolver, require_resolver);
    let documentation = format!(
        "Native binding to [`{native_name}`](Self::{name}).\n\n\
         Resolved by {resolution}.\n\n\
         Direct calls invoke the original implementation, bypassing detours."
    );
    Ok(syn::LitStr::new(&documentation, name.span()))
}

fn resolution_documentation(resolver: &Path, require_resolver: bool) -> String {
    let name = &resolver.segments.last().unwrap().ident;
    let path = resolver.to_token_stream().to_string().replace(' ', "");
    if require_resolver {
        format!("[`{name}`]({path})")
    } else {
        format!("`{name}` (offset-registry key; not a Rust resolver type)")
    }
}

fn native_type(signature: &syn::Signature) -> syn::Result<Tokens> {
    if signature.generics.lifetimes().any(|p| !p.bounds.is_empty()) {
        return Err(syn::Error::new_spanned(
            &signature.generics,
            "native binding lifetimes cannot have bounds",
        ));
    }
    let lifetimes: Vec<_> = signature
        .generics
        .lifetimes()
        .map(|p| &p.lifetime)
        .collect();
    let binder = (!lifetimes.is_empty()).then(|| quote!(for<#(#lifetimes),*>));
    let types = signature.inputs.iter().map(|input| match input {
        FnArg::Typed(arg) => &arg.ty,
        _ => unreachable!(),
    });
    let output = &signature.output;
    Ok(quote!(#binder unsafe extern "C" fn(#(#types),*) #output))
}

pub(super) fn expand_binding(
    args: Tokens,
    function: &ItemFn,
    self_ty: &Type,
    require_resolver: bool,
) -> syn::Result<(ItemFn, syn::ImplItemConst, Tokens)> {
    // Keep declaration validation shared with standalone/legacy bindings.
    super::expand_binding(args.clone(), quote!(#function), require_resolver)?;
    let resolver = resolver_path(args)?;
    let documentation = binding_documentation(function, self_ty, &resolver, require_resolver)?;
    if !require_resolver {
        request_key(&resolver)?;
    }
    let resolver_name = resolver.segments.last().unwrap().ident.to_string();
    let signature = super::detour::concrete_signature(function, Some(self_ty))?;
    let native = native_type(&signature)?;
    let name = &function.sig.ident;
    let descriptor = format_ident!("__UNCHAINED_BINDING_{}", name, span = Span::mixed_site());
    let storage = format_ident!("__UNCHAINED_NATIVE_BINDING_{}", name, span = Span::mixed_site());
    let vis = &function.vis;
    let cfg: Vec<_> = function
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        .collect();
    let arguments = function.sig.inputs.iter().map(|input| match input {
        FnArg::Receiver(_) => quote!(self),
        FnArg::Typed(arg) => {
            let Pat::Ident(pat) = &*arg.pat else {
                unreachable!()
            };
            let ident = &pat.ident;
            quote!(#ident)
        }
    });
    let call_types = function.sig.inputs.iter().map(|input| match input {
        FnArg::Receiver(receiver) => &receiver.ty,
        FnArg::Typed(argument) => &argument.ty,
    });
    let call_output = &function.sig.output;
    let resolver_check = require_resolver.then(|| {
        quote! {
            let _: ::core::marker::PhantomData<#resolver> = ::core::marker::PhantomData;
        }
    });
    let mut expanded = function.clone();
    attach_native_documentation(&mut expanded.attrs, &format!("binding-{resolver_name}"), &documentation);
    expanded.block = Box::new(syn::parse2(quote!({
        #resolver_check
        let native: unsafe extern "C" fn(#(#call_types),*) #call_output = unsafe {
            ::core::mem::transmute(#storage.address()
                .expect(concat!("Native binding ", #resolver_name, " is not initialized")).get())
        };
        unsafe { native(#(#arguments),*) }
    }))?);
    if !expanded.attrs.iter().any(|a| a.path().is_ident("inline")) {
        expanded.attrs.push(syn::parse_quote!(#[inline(always)]));
    }
    let descriptor = syn::parse2(quote! {
        #(#cfg)*
        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        #vis const #descriptor: ::unchained_macros::NativeFunction<#native> =
            unsafe { ::unchained_macros::NativeFunction::new(&#storage) };
    })?;
    let helper = quote! {
        #(#cfg)*
        #[allow(non_upper_case_globals)]
        static #storage: ::unchained_macros::FunctionBinding =
            ::unchained_macros::FunctionBinding::new(#resolver_name);
        #(#cfg)*
        ::inventory::submit! {
            ::unchained_macros::FunctionBindingRegistration { binding: &#storage }
        }
    };
    Ok((expanded, descriptor, helper))
}

pub(super) fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let mut function: ItemFn = syn::parse2(item)?;
    let super::detour::Arguments {
        resolver: mut target,
        condition,
    } = syn::parse2(args)?;
    if target.segments.len() < 2 || target.segments.iter().any(|s| !s.arguments.is_empty()) {
        return Err(syn::Error::new_spanned(
            target,
            "detour target must be a non-generic Type::method binding path",
        ));
    }
    let native_target = target.clone();
    // Keep the user's target span on the native method, not its internal descriptor.
    for segment in &mut target.segments {
        segment.ident.set_span(Span::mixed_site());
    }
    let method = target.segments.last_mut().unwrap();
    method.ident = format_ident!("__UNCHAINED_BINDING_{}", method.ident, span = Span::mixed_site());
    let mut stub = function.clone();
    stub.block = Box::new(syn::parse_quote!({}));
    super::expand_binding(quote!(TypeCheck), quote!(#stub), false)?;
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
             Invoking `{native_link}` in this project directly skips this detour.\n\n\
             See the [native binding and resolver]({native_link})."
        ),
        function.sig.ident.span(),
    );
    let key = format!("detour-{}", function.sig.ident);
    attach_native_documentation(&mut function.attrs, &key, &documentation);
    let signature = super::detour::concrete_signature(&function, None)?;
    let native = native_type(&signature)?;
    let ident = &function.sig.ident;
    let hook = format_ident!("__UNCHAINED_HOOK_{}", ident, span = Span::mixed_site());
    let detour = format_ident!("__UnchainedDetour_{}", ident, span = Span::mixed_site());
    let replacement = format_ident!("__unchained_replacement_{}", ident, span = Span::mixed_site());
    let attach = format_ident!("__unchained_attach_{}", ident, span = Span::mixed_site());
    let condition_fn = format_ident!("__unchained_condition_{}", ident, span = Span::mixed_site());
    let cfg: Vec<_> = function
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        .collect();
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
    let lifetimes: Vec<_> = generics.lifetimes().map(|p| &p.lifetime).collect();
    let binder = if lifetimes.is_empty() {
        quote!(for<'__unchained_call>)
    } else {
        quote!(for<#(#lifetimes),*>)
    };
    let condition = match &condition {
        syn::Expr::Path(path) if path.path.is_ident("ACTIVE") => quote!(|| true),
        syn::Expr::Path(path)
            if ["INACTIVE", "CALLED", "DISABLED"]
                .iter()
                .any(|s| path.path.is_ident(s)) =>
        {
            quote!(|| false)
        }
        _ => quote!(#condition),
    };
    Ok(quote! {
        #function
        #(#cfg)*
        const _: () = {
            let _ = #native_target;
            // Invariant signature metadata makes mismatches compiler errors.
            let _: ::unchained_macros::NativeFunction<#native> = #target;
        };
        #(#cfg)*
        #[allow(non_snake_case)]
        fn #condition_fn() -> bool { (#condition)() }
        #(#cfg)*
        ::retour::lifetime_detour! {
            #[allow(non_camel_case_types)]
            #(#cfg)*
            struct #detour: #binder unsafe extern "C" fn(#(#names: #types),*) #output;
        }
        #(#cfg)*
        #[allow(non_upper_case_globals)]
        static #hook: ::std::sync::OnceLock<#detour> = ::std::sync::OnceLock::new();
        #(#cfg)*
        #[allow(non_snake_case)]
        unsafe extern "C" fn #replacement #generics (#(#names: #types),*) #output {
            unsafe { #ident(#(#names),*) }
        }
        #(#cfg)*
        #[allow(non_snake_case)]
        unsafe fn #attach(
            base_address: usize,
            _offsets: ::std::collections::HashMap<String, u64>, auto_activate: bool,
        ) -> Result<Option<usize>, Box<dyn ::std::error::Error>> {
            let binding = #target.binding();
            let address = binding.entry_address()
                .ok_or_else(|| format!("Native binding {} is not initialized", binding.name()))?.get();
            let offset = address.checked_sub(base_address)
                .ok_or("Native detour address precedes the module")?;
            if #hook.get().is_some() { return Err(::retour::Error::AlreadyInitialized.into()); }
            let claim = ::unchained_macros::__private::claim_detour_address(address)?;
            let target: #native = unsafe { ::core::mem::transmute(address) };
            let detour = unsafe { #detour::new(target, #replacement)? };
            #hook.set(detour).map_err(|_| ::retour::Error::AlreadyInitialized)?;
            let hook = #hook.get().expect("Hook was just initialized");
            // The detour and trampoline are process-lifetime storage, even disabled.
            claim.commit();
            unsafe { ::unchained_macros::__private::publish_function_trampolines(
                address, hook.trampoline() as *const () as usize,
            )?; }
            if auto_activate && #condition_fn() { unsafe { hook.enable()?; } }
            Ok(Some(offset))
        }
        #(#cfg)*
        ::inventory::submit! {
            ::unchained_macros::HookRegistration {
                name: #target.name(), hook_fn: #attach, condition: #condition_fn,
            }
        }
        #(#cfg)*
        ::inventory::submit! {
            ::unchained_macros::DetourBindingRegistration {
                name: #target.name(),
                initialized: || #hook.get().is_some(),
                enabled: || #hook.get().is_some_and(|hook| hook.is_enabled()),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn documentation(attributes: &[syn::Attribute]) -> String {
        attributes.iter().filter_map(|attribute| {
            if !attribute.path().is_ident("doc") { return None; }
            let syn::Meta::NameValue(value) = &attribute.meta else { return None };
            let syn::Expr::Lit(value) = &value.value else { return None };
            let syn::Lit::Str(value) = &value.lit else { return None };
            Some(value.value())
        }).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn binding_hover_identifies_native_method_resolver_and_bypass() {
        let function = syn::parse_quote! {
            /// Existing native safety note.
            pub(crate) unsafe fn Call<'a>(
                self: *const Self,
                Result: &'a mut FString,
            ) -> &'a mut FString {}
        };
        let (binding, descriptor, _) = expand_binding(
            quote!(self::Native__Call), &function, &syn::parse_quote!(Native), true,
        ).unwrap();
        let docs = documentation(&binding.attrs);
        assert!(documentation(&descriptor.attrs).is_empty());
        assert!(docs.starts_with("Native binding to [`Native::Call`](Self::Call)."));
        assert!(docs.contains("Resolved by [`Native__Call`](self::Native__Call)."));
        assert!(docs.contains("Direct calls invoke the original implementation, bypassing detours."));
        assert!(docs.contains("Existing native safety note."));
        assert!(!docs.contains("impl Native"));
        assert!(!docs.contains("Overrides in-game calls"));
    }

    #[test]
    fn request_hover_identifies_registry_keys_without_fabricating_resolver_links() {
        let function = syn::parse_quote! {
            pub unsafe fn GetPost_Example(value: i32) -> i32 {}
        };
        let (binding, _, _) = expand_binding(
            quote!(GetPost_Example), &function, &syn::parse_quote!(Client), false,
        ).unwrap();
        let docs = documentation(&binding.attrs);
        assert!(docs.contains("Resolved by `GetPost_Example` (offset-registry key;"));
        assert!(!docs.contains("](GetPost_Example)"));
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
        assert!(docs.contains(
            "[native binding and resolver](model::Native::Call)"
        ));
        assert!(docs.contains("Overrides in-game calls to"));
        assert!(docs.contains("Invoking `model::Native::Call` in this project directly skips this detour."));
    }

    #[test]
    fn targets_typed_binding_metadata_without_generating_an_original_caller() {
        let expanded = expand(
            quote!(model::Native::call, || enabled()),
            quote!(
                unsafe fn feature_override(value: i32) -> i32 {
                    unsafe { model::Native::call(value) }
                }
            ),
        )
        .unwrap();
        let text = expanded.to_string();
        assert!(text.contains("Native :: __UNCHAINED_BINDING_call"));
        assert!(text.contains("NativeFunction"));
        assert!(text.contains("publish_function_trampolines"));
        assert!(text.contains("unchained_macros :: HookRegistration"));
        assert!(text.contains("base_address : usize"));
        assert!(!text.contains("crate :: resolvers"));
        assert!(!text.contains("PhantomData"));
        assert!(!text.contains("unsafe fn feature ("));
        syn::parse2::<syn::File>(expanded).unwrap();
    }

    #[test]
    fn rejects_non_method_targets_and_invalid_replacement_declarations() {
        for target in [
            quote!(Resolver),
            quote!(Type::<u8>::call),
            quote!(Type::call::<u8>),
        ] {
            assert!(expand(
                target,
                quote!(
                    unsafe fn replacement() {}
                )
            )
            .is_err());
        }
        for declaration in [
            quote!(
                fn replacement() {}
            ),
            quote!(
                unsafe fn replacement<T>(value: T) {}
            ),
            quote!(
                async unsafe fn replacement() {}
            ),
            quote!(
                unsafe extern "C" fn replacement() {}
            ),
            quote!(
                unsafe fn replacement(&self) {}
            ),
            quote!(
                unsafe fn replacement<'a: 'static>(value: &'a i32) {}
            ),
        ] {
            assert!(expand(quote!(Type::call), declaration).is_err());
        }
    }

    #[test]
    fn runtime_binding_descriptors_carry_visibility_and_cfg_restrictions() {
        let expanded = super::super::detour::expand_impl(
            quote!(),
            quote! {
                impl Native {
                    #[cfg(feature = "example")]
                    #[bind_resolved_func(Resolver)]
                    pub(crate) unsafe fn call(&self) -> &Self {}
                }
            },
        )
        .unwrap();
        let text = expanded.to_string();
        assert!(text.contains("pub (crate) const __UNCHAINED_BINDING_call"));
        assert!(text.contains("for < '__unchained_receiver >"));
        assert!(text.matches("cfg (feature = \"example\")").count() >= 3);
        syn::parse2::<syn::File>(expanded).unwrap();
    }
}
