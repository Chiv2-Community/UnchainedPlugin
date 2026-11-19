use super::*;
use quote::format_ident;
use syn::fold::Fold;
use syn::visit_mut::VisitMut;
use syn::{parse::Parse, parse::ParseStream, Expr, Token};

pub(super) struct Arguments {
    pub resolver: Path,
    pub condition: Expr,
}

impl Parse for Arguments {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let resolver = input.parse()?;
        let condition = if input.is_empty() {
            syn::parse_quote!(ACTIVE)
        } else {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                syn::parse_quote!(ACTIVE)
            } else {
                input.parse()?
            }
        };
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        Ok(Self {
            resolver,
            condition,
        })
    }
}

pub(super) fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let function: ItemFn = syn::parse2(item)?;
    let (original, helpers) = expand_function(args, &function, None, false)?;
    Ok(quote!(#function #original #helpers))
}

pub(super) fn expand_request(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let function: ItemFn = syn::parse2(item)?;
    let (original, helpers) = expand_function(args, &function, None, true)?;
    Ok(quote!(#function #original #helpers))
}

pub(super) fn expand_impl(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(
            args,
            "runtime_bindings takes no arguments",
        ));
    }
    let mut implementation: syn::ItemImpl = syn::parse2(item)?;
    if implementation.trait_.is_some()
        || !implementation.generics.params.is_empty()
        || implementation.generics.where_clause.is_some()
    {
        return Err(syn::Error::new_spanned(
            &implementation,
            "runtime_bindings requires a non-generic inherent impl",
        ));
    }
    let self_ty = &implementation.self_ty;
    let mut names: std::collections::HashSet<_> = implementation
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(method) => Some(method.sig.ident.to_string()),
            syn::ImplItem::Const(item) => Some(item.ident.to_string()),
            _ => None,
        })
        .collect();
    let mut originals = Vec::new();
    let mut helpers = Vec::new();
    for item in &mut implementation.items {
        let syn::ImplItem::Fn(method) = item else {
            continue;
        };
        let binding_attributes: Vec<_> = method.attrs.iter().enumerate().filter(|(_, attr)|
            attr.path().segments.last().is_some_and(|s|
                s.ident == "bind_resolved_func" || s.ident == "bind_resolved_request_function")
        ).map(|(index, _)| index).collect();
        if !binding_attributes.is_empty() {
            if binding_attributes.len() != 1 || method.attrs.iter().any(|attr|
                attr.path().segments.last().is_some_and(|s|
                    s.ident == "bind_resolved_func_detour" || s.ident == "bind_resolved_request_function_detour")
            ) {
                return Err(syn::Error::new_spanned(method, "duplicate or conflicting native binding attributes"));
            }
            let attribute = method.attrs.remove(binding_attributes[0]);
            let require_resolver = attribute.path().segments.last().unwrap().ident == "bind_resolved_func";
            let args = attribute.meta.require_list()?.tokens.clone();
            let function: ItemFn = syn::parse2(quote!(#method))?;
            let (expanded, descriptor, helper) = super::function_detour::expand_binding(
                args, &function, self_ty, require_resolver,
            )?;
            if !names.insert(descriptor.ident.to_string()) {
                return Err(syn::Error::new_spanned(descriptor, "generated binding metadata conflicts with an associated item"));
            }
            *method = syn::parse2(quote!(#expanded))?;
            originals.push(syn::ImplItem::Const(descriptor));
            helpers.push(helper);
            continue;
        }
        let attributes: Vec<_> = method
            .attrs
            .iter()
            .enumerate()
            .filter(|(_, attr)| {
                attr.path()
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "bind_resolved_func_detour"
                        || s.ident == "bind_resolved_request_function_detour")
            })
            .map(|(i, _)| i)
            .collect();
        if attributes.is_empty() {
            continue;
        }
        if attributes.len() != 1 {
            return Err(syn::Error::new_spanned(
                method,
                "duplicate detour attribute",
            ));
        }
        let attribute = method.attrs.remove(attributes[0]);
        let request = attribute.path().segments.last().unwrap().ident == "bind_resolved_request_function_detour";
        let args = attribute.meta.require_list()?.tokens.clone();
        let function: ItemFn = syn::parse2(quote!(#method))?;
        let (original, helper) = expand_function(args, &function, Some(self_ty), request)?;
        if !names.insert(original.sig.ident.to_string()) {
            return Err(syn::Error::new_spanned(
                &original.sig.ident,
                "generated original method conflicts with an existing associated item",
            ));
        }
        originals.push(syn::parse2::<syn::ImplItem>(quote!(#original))?);
        helpers.push(helper);
    }
    implementation.items.extend(originals);
    let cfg: Vec<_> = implementation
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        .collect();
    // A const scope isolates helper names across impl blocks without changing
    // module-relative paths or the visibility of the inherent methods.
    Ok(quote! {
        #(#cfg)*
        const _: () = {
            #implementation
            #(#helpers)*
        };
    })
}

struct ConcreteSelf<'a>(&'a Type);

impl Fold for ConcreteSelf<'_> {
    fn fold_expr_path(&mut self, path: syn::ExprPath) -> syn::ExprPath {
        if path.qself.is_none()
            && path.path.segments.len() > 1
            && path
                .path
                .segments
                .first()
                .is_some_and(|s| s.ident == "Self")
        {
            let tail: syn::punctuated::Punctuated<_, Token![::]> =
                path.path.segments.iter().skip(1).cloned().collect();
            let concrete = self.0;
            return syn::parse_quote!(<#concrete>::#tail);
        }
        syn::fold::fold_expr_path(self, path)
    }

    fn fold_type(&mut self, ty: Type) -> Type {
        if matches!(&ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident("Self")) {
            return self.0.clone();
        }
        if let Type::Path(path) = &ty {
            if path.qself.is_none()
                && path
                    .path
                    .segments
                    .first()
                    .is_some_and(|s| s.ident == "Self")
            {
                let tail: syn::punctuated::Punctuated<_, Token![::]> =
                    path.path.segments.iter().skip(1).cloned().collect();
                let concrete = self.0;
                return syn::parse_quote!(<#concrete>::#tail);
            }
        }
        syn::fold::fold_type(self, ty)
    }
}

struct ElidedOutput(syn::Lifetime);

impl VisitMut for ElidedOutput {
    fn visit_type_bare_fn_mut(&mut self, _: &mut syn::TypeBareFn) {}

    fn visit_parenthesized_generic_arguments_mut(
        &mut self,
        _: &mut syn::ParenthesizedGenericArguments,
    ) {
    }

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_" {
            *lifetime = self.0.clone();
        }
    }

    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {
        if reference.lifetime.is_none()
            || reference.lifetime.as_ref().is_some_and(|l| l.ident == "_")
        {
            reference.lifetime = Some(self.0.clone());
        }
        syn::visit_mut::visit_type_reference_mut(self, reference);
    }
}

fn expand_function(
    args: Tokens,
    function: &ItemFn,
    self_ty: Option<&Type>,
    request: bool,
) -> syn::Result<(ItemFn, Tokens)> {
    let Arguments {
        resolver,
        condition,
    } = syn::parse2(args)?;
    let resolver = resolver_path(quote!(#resolver))?;
    if request { super::request_key(&resolver)?; }
    let name = resolver.segments.last().unwrap().ident.to_string();
    // Reuse direct-binding declaration checks without imposing an empty unsafe body.
    let mut stub = function.clone();
    stub.sig.unsafety = Some(syn::parse_quote!(unsafe));
    stub.block = Box::new(syn::parse_quote!({}));
    super::expand_binding(quote!(#resolver), quote!(#stub), !request)?;
    let ident = &function.sig.ident;
    let spelling = ident.to_string();
    let stripped = spelling
        .strip_suffix("_detour")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            syn::Error::new_spanned(
                ident,
                "detour names must end with _detour and have a nonempty base name",
            )
        })?;
    let original_ident = format_ident!("{}", stripped, span = ident.span());
    let native_signature = concrete_signature(function, self_ty)?;
    let mut arguments = Vec::new();
    if matches!(function.sig.inputs.first(), Some(FnArg::Receiver(_))) {
        arguments.push(quote!(self));
    }
    let mut types = Vec::new();
    let mut names = Vec::new();
    for (index, input) in native_signature.inputs.iter().enumerate() {
        let FnArg::Typed(argument) = input else {
            return Err(syn::Error::new_spanned(input, "unsupported detour receiver"));
        };
        let Pat::Ident(ident) = &*argument.pat else { unreachable!() };
        types.push(&argument.ty);
        names.push(&ident.ident);
        let name = &ident.ident;
        if index != 0 || arguments.is_empty() {
            arguments.push(quote!(#name));
        }
    }
    let condition = match &condition {
        Expr::Path(path) if path.path.is_ident("ACTIVE") => quote!(|| true),
        Expr::Path(path)
            if ["INACTIVE", "CALLED", "DISABLED"]
                .iter()
                .any(|s| path.path.is_ident(s)) =>
        {
            quote!(|| false)
        }
        _ => quote!(#condition),
    };
    let hook = format_ident!("__UNCHAINED_HOOK_{}", ident);
    let detour = format_ident!("__UnchainedDetour_{}", ident);
    let replacement = format_ident!("__unchained_replacement_{}", ident);
    let attach = format_ident!("__unchained_attach_{}", ident);
    let condition_fn = format_ident!("__unchained_condition_{}", ident);
    let mut original = function.clone();
    original.sig.ident = original_ident;
    original.sig.unsafety = Some(syn::parse_quote!(unsafe));
    original
        .attrs
        .retain(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"));
    let resolver_type_check = (!request).then(|| quote! {
        let _: ::core::marker::PhantomData<#resolver> = ::core::marker::PhantomData;
    });
    original.block = Box::new(syn::parse_quote!({
        #resolver_type_check
        unsafe { #hook.get()
            .expect(concat!("Detour ", #name, " is not initialized")).call(#(#arguments),*) }
    }));
    let cfg = original.attrs.clone();
    let output = &native_signature.output;
    let generics = &native_signature.generics;
    let lifetimes: Vec<_> = generics.lifetimes().map(|p| &p.lifetime).collect();
    if generics.lifetimes().any(|p| !p.bounds.is_empty()) {
        return Err(syn::Error::new_spanned(
            generics,
            "detour lifetimes cannot have bounds",
        ));
    }
    let binder = if lifetimes.is_empty() {
        quote!(for<'__unchained_call>)
    } else {
        quote!(for<#(#lifetimes),*>)
    };
    let invoke = if let Some(self_ty) = self_ty {
        quote!(<#self_ty>::#ident(#(#names),*))
    } else {
        quote!(#ident(#(#names),*))
    };
    let condition = if let Some(self_ty) = self_ty {
        // Keep Self usable in method-level conditions.
        let condition: Expr = syn::parse2(condition)?;
        let condition = ConcreteSelf(self_ty).fold_expr(condition);
        quote!(#condition)
    } else {
        condition
    };
    Ok((
        original,
        quote! {
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
                unsafe { #invoke }
            }
            #(#cfg)*
            #[allow(non_snake_case)]
            unsafe fn #attach(
                offsets: ::std::collections::HashMap<String, u64>, auto_activate: bool,
            ) -> Result<Option<usize>, Box<dyn ::std::error::Error>> {
                let offset = usize::try_from(*offsets.get(#name)
                    .ok_or_else(|| format!("No address found for {}", #name))?)?;
                let address = (*crate::resolvers::BASE_ADDR).checked_add(offset)
                    .ok_or("Native detour address overflow")?;
                if address == 0 { return Err("Native detour address is null".into()); }
                if #hook.get().is_some() {
                    return Err(::retour::Error::AlreadyInitialized.into());
                }
                let claim = ::unchained_macros::__private::claim_detour_address(address)?;
                let target: #binder unsafe extern "C" fn(#(#types),*) #output =
                    unsafe { ::core::mem::transmute(address) };
                let detour = unsafe { #detour::new(target, #replacement)? };
                #hook.set(detour).map_err(|_| ::retour::Error::AlreadyInitialized)?;
                claim.commit();
                unsafe { ::unchained_macros::__private::publish_function_trampolines(
                    address, #hook.get().expect("Hook was just initialized").trampoline() as *const () as usize,
                )?; }
                if auto_activate && #condition_fn() {
                    unsafe { #hook.get().expect("Hook was just initialized").enable()?; }
                }
                Ok(Some(offset))
            }
            #(#cfg)*
            ::inventory::submit! {
                crate::resolvers::HookRegistration {
                    name: #name, hook_fn: #attach, condition: #condition_fn,
                }
            }
            #(#cfg)*
            ::inventory::submit! {
                ::unchained_macros::DetourBindingRegistration {
                    name: #name,
                    initialized: || #hook.get().is_some(),
                    enabled: || #hook.get().is_some_and(|hook| hook.is_enabled()),
                }
            }
        },
    ))
}

pub(super) fn concrete_signature(function: &ItemFn, self_ty: Option<&Type>) -> syn::Result<syn::Signature> {
    let mut native_signature = function.sig.clone();
    let ident = &function.sig.ident;
    // Native callbacks have an explicit receiver parameter. Preserve Rust's
    // receiver-based output lifetime elision before removing that receiver.
    if let Some(FnArg::Receiver(receiver)) = native_signature.inputs.first_mut() {
        let self_ty = self_ty.ok_or_else(|| {
            syn::Error::new_spanned(
                &*receiver,
                "detour methods require #[unchained_macros::runtime_bindings] on the impl",
            )
        })?;
        if let Type::Reference(reference) = receiver.ty.as_mut() {
            let lifetime = reference
                .lifetime
                .clone()
                .filter(|l| l.ident != "_")
                .unwrap_or_else(|| {
                    let mut name = "__unchained_receiver".to_string();
                    while function
                        .sig
                        .generics
                        .lifetimes()
                        .any(|p| p.lifetime.ident == name)
                    {
                        name.push('_');
                    }
                    let lifetime = syn::Lifetime::new(&format!("'{name}"), ident.span());
                    native_signature
                        .generics
                        .params
                        .push(syn::parse_quote!(#lifetime));
                    lifetime
                });
            reference.lifetime = Some(lifetime.clone());
            ElidedOutput(lifetime).visit_return_type_mut(&mut native_signature.output);
        }
        let ty = ConcreteSelf(self_ty).fold_type(*receiver.ty.clone());
        let mut receiver_name = "__unchained_this".to_string();
        while function.sig.inputs.iter().any(|input| matches!(input,
            FnArg::Typed(arg) if matches!(arg.pat.as_ref(), Pat::Ident(p) if p.ident == receiver_name)))
        {
            receiver_name.push('_');
        }
        let receiver_name = format_ident!("{}", receiver_name);
        native_signature.inputs[0] = syn::parse_quote!(#receiver_name: #ty);
    }
    if let Some(self_ty) = self_ty {
        native_signature = ConcreteSelf(self_ty).fold_signature(native_signature);
    }
    Ok(native_signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_detours_use_registry_keys_without_marker_types() {
        let expanded = expand_impl(quote!(), quote! {
            impl Client {
                #[bind_resolved_request_function_detour(GetPost_GetPlayerInventory, || Self::enabled())]
                pub unsafe fn get_post_get_player_inventory_detour(self: *mut Self, value: i32) -> i32 {
                    unsafe { self.get_post_get_player_inventory(value) }
                }
            }
        }).unwrap();
        let text = expanded.to_string();
        assert!(text.contains("\"GetPost_GetPlayerInventory\""));
        assert!(text.contains("get_post_get_player_inventory"));
        assert!(!text.contains("PhantomData"));
        assert!(!text.contains("bind_resolved_request_function_detour"));
        syn::parse2::<syn::File>(expanded).unwrap();
    }

    #[test]
    fn standalone_request_detours_preserve_conditions_and_suffix_rules() {
        let expanded = expand_request(quote!(GetPost_GetMotd, INACTIVE),
            quote!(unsafe fn get_motd_detour(value: i32) -> i32 { unsafe { get_motd(value) } })).unwrap();
        assert!(!expanded.to_string().contains("PhantomData"));
        syn::parse2::<syn::File>(expanded).unwrap();
        assert!(expand_request(quote!(GetPost_GetMotd), quote!(fn get_motd() {})).is_err());
        assert!(expand_request(quote!(CreateHttpRequest), quote!(fn get_motd_detour() {})).is_err());
    }

    #[test]
    fn request_and_native_detour_attributes_cannot_both_annotate_a_method() {
        assert!(expand_impl(quote!(), quote! {
            impl Client {
                #[bind_resolved_func_detour(Native)]
                #[bind_resolved_request_function_detour(GetPost_GetMotd)]
                unsafe fn request_detour(&self) {}
            }
        }).unwrap_err().to_string().contains("duplicate detour"));
    }

    #[test]
    fn accepts_safe_bodies_and_conditions() {
        for args in [
            quote!(Resolver),
            quote!(Resolver, INACTIVE),
            quote!(Resolver, || true),
        ] {
            let expanded = expand(
                args,
                quote!(
                    fn foo_detour(value: i32) -> i32 {
                        value + 1
                    }
                ),
            )
            .unwrap();
            syn::parse2::<syn::File>(expanded).unwrap();
        }
    }

    #[test]
    fn rejects_unsupported_declarations() {
        for item in [
            quote!(
                fn foo_detour(&self) {}
            ),
            quote!(
                fn foo_detour<T>(x: T) {}
            ),
            quote!(
                async fn foo_detour() {}
            ),
            quote!(
                fn foo_detour((x, y): (u8, u8)) {}
            ),
            quote!(
                fn foo_detour<'a: 'b, 'b>(x: &'a u8) {}
            ),
        ] {
            assert!(expand(quote!(Resolver), item).is_err());
        }
        assert!(expand(
            quote!(Resolver, ACTIVE, false),
            quote!(
                fn foo_detour() {}
            )
        )
        .is_err());
        assert!(expand(
            quote!(Resolver),
            quote!(
                fn foo_detour(&mut self) {}
            )
        )
        .unwrap_err()
        .to_string()
        .contains("runtime_bindings"));
    }

    #[test]
    fn enforces_suffix_for_standalone_functions() {
        for name in [
            quote!(
                fn foo() {}
            ),
            quote!(
                fn _detour() {}
            ),
        ] {
            assert!(expand(quote!(Resolver), name)
                .unwrap_err()
                .to_string()
                .contains("must end"));
        }
    }

    #[test]
    fn expands_inherent_detours_and_rejects_collisions() {
        let expanded = expand_impl(
            quote!(),
            quote! {
                impl Native {
                    #[bind_resolved_func_detour(Resolver, || Self::enabled())]
                    fn foo_detour(&mut self, x: i32) -> i32 { unsafe { self.foo(x) } }
                }
            },
        )
        .unwrap();
        syn::parse2::<syn::File>(expanded).unwrap();
        for implementation in [
            quote!(impl Native {
                fn foo(&self) {}
                #[bind_resolved_func_detour(Resolver)]
                fn foo_detour(&self) {}
            }),
            quote!(impl Native {
                #[bind_resolved_func_detour(Resolver)]
                fn foo(&self) {}
            }),
            quote!(
                impl<T> Native<T> {}
            ),
            quote!(impl Trait for Native {}),
            quote!(impl Native {
                #[bind_resolved_func_detour(Resolver)]
                #[bind_resolved_func_detour(Resolver)]
                fn foo_detour(&self) {}
            }),
        ] {
            assert!(expand_impl(quote!(), implementation).is_err());
        }
        assert!(expand_impl(quote!(unexpected), quote!(impl Native {})).is_err());
    }
}
