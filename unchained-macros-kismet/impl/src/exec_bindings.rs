use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{FnArg, ItemImpl, ReturnType, Type};

fn cfg(attributes: &[syn::Attribute]) -> Vec<syn::Attribute> {
    attributes.iter().filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr")).cloned().collect()
}

fn value_type(ty: &Type) -> syn::Result<Type> {
    if let Type::Reference(reference) = ty {
        if reference.mutability.is_some() || reference.lifetime.is_some() {
            return Err(syn::Error::new_spanned(ty, "exec observations require immutable call-scoped borrows"));
        }
        Ok((*reference.elem).clone())
    } else { Ok(ty.clone()) }
}

fn parameter(ty: &Type, result: bool) -> syn::Result<Tokens> {
    let ty = value_type(ty)?;
    let kind = match &ty {
        Type::Tuple(tuple) if tuple.elems.is_empty() && result => return Ok(quote!(None)),
        Type::Ptr(pointer) if matches!(&*pointer.elem, Type::Path(path) if path.path.segments.last().unwrap().ident == "UObject") => "ObjectProperty",
        Type::Path(path) => match path.path.segments.last().unwrap().ident.to_string().as_str() {
            "bool" => "BoolProperty",
            "u8" => "ByteProperty",
            "i32" => "IntProperty",
            "f32" => "FloatProperty",
            "FString" => "StrProperty",
            "FText" => "TextProperty",
            "FLinearColor" => "StructProperty:LinearColor",
            _ => return Err(syn::Error::new_spanned(&ty, "unsupported exec observation type")),
        },
        _ => return Err(syn::Error::new_spanned(&ty, "unsupported exec observation type")),
    };
    Ok(quote!(Some(::unchained_macros_kismet::schema::Parameter {
        kind: #kind, size: ::core::mem::size_of::<#ty>(),
    })))
}

pub fn expand(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let owner: syn::LitStr = syn::parse2(args)?;
    if !owner.value().starts_with("/Script/") || !owner.value().contains('.') || owner.value().contains('\0') {
        return Err(syn::Error::new_spanned(owner, "expected a /Script/Package.Class path without NUL"));
    }
    let implementation: ItemImpl = syn::parse2(item)?;
    if implementation.trait_.is_some() || !implementation.generics.params.is_empty() || implementation.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(&implementation, "exec bindings require a non-generic inherent impl"));
    }
    let Type::Path(path) = &*implementation.self_ty else {
        return Err(syn::Error::new_spanned(&implementation.self_ty, "expected a named native type"));
    };
    let target = &implementation.self_ty;
    let class = &path.path.segments.last().unwrap().ident;
    let outer_cfg = cfg(&implementation.attrs);
    let mut output = Vec::new();
    for item in &implementation.items {
        let syn::ImplItem::Fn(method) = item else {
            return Err(syn::Error::new_spanned(item, "exec binding impls contain only method declarations"));
        };
        let signature = &method.sig;
        if signature.unsafety.is_none() || signature.abi.is_some() || signature.asyncness.is_some()
            || signature.constness.is_some() || signature.variadic.is_some()
            || !signature.generics.params.is_empty() || signature.generics.where_clause.is_some()
            || !method.block.stmts.is_empty()
        {
            return Err(syn::Error::new_spanned(method, "use an empty unsafe non-generic exec declaration without an ABI"));
        }
        let annotations: Vec<_> = method.attrs.iter().filter(|a| a.path().is_ident("exec_observe")).collect();
        if annotations.len() != 1 {
            return Err(syn::Error::new_spanned(method, "exec observation requires #[exec_observe(verified)] and current binary evidence"));
        }
        let options = annotations[0].parse_args_with(
            syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated,
        )?;
        let mut verified = false;
        let mut dummy = false;
        for option in options {
            if option == "verified" && !verified { verified = true; }
            else if option == "dummied" && !dummy { dummy = true; }
            else { return Err(syn::Error::new_spanned(option, "expected unique verified or dummied options")); }
        }
        if !verified { return Err(syn::Error::new_spanned(annotations[0], "exec wrappers require an explicit verified Step/lifetime contract")); }
        let method_name = &signature.ident;
        let member = method_name.to_string();
        let prefix = format!("__UNCHAINED_EXEC_{class}_{method_name}");
        let resolver = format_ident!("{prefix}_RESOLVER");
        let arguments = format_ident!("{prefix}_ARGUMENTS");
        let binding = format_ident!("{prefix}_BINDING");
        let hook = format_ident!("{prefix}_HOOK");
        let descriptor = format_ident!("{prefix}_LISTENERS");
        let attach = format_ident!("{prefix}_attach");
        let dispatch = format_ident!("{prefix}_dispatch");
        let entry = format_ident!("{prefix}_entry");
        let snapshot = format_ident!("{prefix}_snapshot");
        let finish = format_ident!("{prefix}_finish");
        let associated = format_ident!("__UNCHAINED_LISTENERS_{method_name}");
        let rejected = format_ident!("__UNCHAINED_BINDING_{method_name}");
        let visibility = &method.vis;
        let mut attributes = method.attrs.clone();
        attributes.retain(|a| !a.path().is_ident("exec_observe"));
        let mut attributes_cfg = outer_cfg.clone();
        attributes_cfg.extend(cfg(&attributes));
        let mut types = Vec::new();
        let mut parameters = Vec::new();
        let mut decode = Vec::new();
        let mut borrowed = Vec::new();
        for (index, input) in signature.inputs.iter().enumerate() {
            let FnArg::Typed(input) = input else {
                return Err(syn::Error::new_spanned(input, "exec declarations have explicit inputs, not Rust receivers"));
            };
            let ty = value_type(&input.ty)?;
            let spec = parameter(&input.ty, false)?;
            parameters.push(quote! { match #spec { Some(parameter) => parameter, None => unreachable!() } });
            let local = format_ident!("argument_{index}");
            if matches!(&ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "bool")) {
                decode.push(quote! { let #local = unsafe { inputs.get(#index).expect("validated exec argument").boolean() }; });
                borrowed.push(quote!(&#local));
            } else {
                decode.push(quote! { let #local = inputs.get(#index).expect("validated exec argument"); let #local = unsafe { #local.borrow::<#ty>() }; });
                borrowed.push(quote!(#local));
            }
            types.push(ty);
        }
        let result: Type = match &signature.output {
            ReturnType::Default => syn::parse_quote!(()),
            ReturnType::Type(_, ty) => {
                if matches!(&**ty, Type::Reference(_)) {
                    return Err(syn::Error::new_spanned(ty, "exec result references are unsupported"));
                }
                *ty.clone()
            }
        };
        let result_parameter = parameter(&result, true)?;
        let observed_result = if matches!(&result, Type::Tuple(tuple) if tuple.elems.is_empty()) {
            quote! { let void_result = (); let observed_result = (&void_result as *const ()).cast_mut().cast(); }
        } else { quote! { let observed_result = result; } };
        let dummy_type = if dummy { quote!(::unchained_macros_kismet::VerifiedDummiedExecMethodCannotBeDetoured) }
            else { quote!(::unchained_macros_kismet::ExecOnlyMethodHasNoNativeCallAbi) };
        output.push(quote! {
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #arguments: &[::unchained_macros_kismet::schema::Parameter] = &[#(#parameters),*];
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #resolver: ::unchained_macros_core::Resolver<::unchained_macros_kismet::KismetExecResolver> =
                ::unchained_macros_core::Resolver {
                    name: concat!(stringify!(#class), "__", #member, "__exec"),
                    implementation: unsafe { ::unchained_macros_kismet::KismetExecResolver::new(
                        #owner, #member, #arguments, #result_parameter,
                    ) },
                    filter: None,
                };
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #binding: ::unchained_macros_core::FunctionBinding<::unchained_macros_kismet::KismetExecResolver> =
                ::unchained_macros_core::FunctionBinding::new(concat!(stringify!(#class), "::", #member, " [exec]"), &#resolver);
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #hook: ::unchained_macros_core::__private::NativeHook = ::unchained_macros_core::__private::NativeHook::new();
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #descriptor: ::unchained_macros_core::ListenerDescriptor<
                unsafe fn(#(&#types),*),
                unsafe fn(#(&#types,)* &#result),
                unsafe fn(#(&#types),*) -> ::unchained_macros_core::ExitObservation,
                ::unchained_macros_kismet::KismetExecResolver,
            > = unsafe { ::unchained_macros_core::ListenerDescriptor::new(&#resolver, #attach) };
            #(#attributes_cfg)*
            #[allow(non_snake_case, non_upper_case_globals)]
            impl #target {
                #(#attributes)*
                #[doc = "Exec-only observation marker. This is not a native-call binding."]
                #visibility const #method_name: () = ();
                #[doc(hidden)]
                #visibility const #associated: &'static ::unchained_macros_core::ListenerDescriptor<
                    unsafe fn(#(&#types),*),
                    unsafe fn(#(&#types,)* &#result),
                    unsafe fn(#(&#types),*) -> ::unchained_macros_core::ExitObservation,
                    ::unchained_macros_kismet::KismetExecResolver,
                > = &#descriptor;
                #[doc(hidden)]
                #visibility const #rejected: #dummy_type = #dummy_type;
            }
            #(#attributes_cfg)*
            #[allow(non_snake_case)]
            unsafe fn #attach() -> Result<(), Box<dyn ::std::error::Error>> {
                unsafe { #hook.attach(&#binding, #dispatch as *const ()) }
            }
            #(#attributes_cfg)*
            #[allow(non_snake_case)]
            unsafe fn #entry(inputs: ::unchained_macros_kismet::observation::Inputs<'_>) {
                #descriptor.observe_entry(|callback| {
                    #(#decode)*
                    unsafe { callback(#(#borrowed),*); }
                });
            }
            #(#attributes_cfg)*
            #[allow(non_snake_case)]
            unsafe fn #snapshot(inputs: ::unchained_macros_kismet::observation::Inputs<'_>)
                -> Option<::unchained_macros_core::ExitObservation>
            {
                #descriptor.prepare_exit(|prepare| {
                    #(#decode)*
                    unsafe { prepare(#(#borrowed),*) }
                })
            }
            #(#attributes_cfg)*
            #[allow(non_snake_case)]
            unsafe fn #finish(observation: ::unchained_macros_core::ExitObservation, result: *const ::core::ffi::c_void) {
                unsafe { #descriptor.finish_exit(observation, result.cast()); }
            }
            #(#attributes_cfg)*
            #[allow(non_snake_case)]
            unsafe extern "system" fn #dispatch(context: *mut ::core::ffi::c_void, frame: *mut ::core::ffi::c_void, result: *mut ::core::ffi::c_void) {
                let original: unsafe extern "system" fn(*mut ::core::ffi::c_void, *mut ::core::ffi::c_void, *mut ::core::ffi::c_void) =
                    unsafe { ::core::mem::transmute(#binding.address().expect("exec trampoline was published").get()) };
                if !#resolver.implementation.is_observation_thread() {
                    unsafe { original(context, frame, result); }
                    return;
                }
                #observed_result
                unsafe {
                    ::unchained_macros_kismet::observation::observe_exec(
                        frame, observed_result, #resolver.implementation.properties(),
                        ::unchained_macros_kismet::observation::Callbacks {
                            entry: Some(#entry), snapshot: Some(#snapshot), finish: Some(#finish),
                            result_alignment: ::core::mem::align_of::<#result>(),
                        },
                        || original(context, frame, result),
                    );
                }
            }
            #(#attributes_cfg)*
            ::unchained_macros_core::__private::inventory::submit! {
                &#resolver as &dyn ::unchained_macros_core::__private::ResolverInfo
            }
            #(#attributes_cfg)*
            ::unchained_macros_core::__private::inventory::submit! {
                &::unchained_macros_core::BindingRegistration { binding: &#binding }
                    as &dyn ::unchained_macros_core::__private::FunctionInfo
            }
        });
    }
    Ok(quote!(#(#output)*))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_verification_is_required_and_dummies_keep_exec_dispatch() {
        assert!(expand(quote!("/Script/Fixture.Library"), quote! {
            impl Library { pub unsafe fn PrintWarning(message: &FString) {} }
        }).unwrap_err().to_string().contains("verified"));
        let output = expand(quote!("/Script/Fixture.Library"), quote! {
            impl Library {
                #[exec_observe(verified, dummied)]
                pub unsafe fn PrintWarning(message: &FString) {}
            }
        }).unwrap().to_string();
        assert!(output.contains("VerifiedDummiedExecMethodCannotBeDetoured"));
        assert!(output.contains("observe_exec"));
        assert!(output.contains("original (context , frame , result)"));
        assert!(!output.contains("RawDetour"));
        syn::parse2::<syn::File>(output.parse().unwrap()).unwrap();
    }

    #[test]
    fn unsupported_results_and_mutable_borrows_are_rejected() {
        for declaration in [
            quote!(impl Library { #[exec_observe(verified)] unsafe fn Call(value: &mut FString) {} }),
            quote!(impl Library { #[exec_observe(verified)] unsafe fn Call() -> UnsupportedAggregate {} }),
            quote!(impl Library { #[exec_observe(verified)] unsafe fn Call(&self) {} }),
            quote!(impl Library { #[exec_observe(verified)] unsafe fn Call() { todo!() } }),
            quote!(impl Library { #[exec_observe(verified)] unsafe fn Call<T>(value: T) {} }),
        ] {
            assert!(expand(quote!("/Script/Fixture.Library"), declaration).is_err());
        }
    }

    #[test]
    fn cfgs_cover_resolver_descriptor_dispatch_and_inventory() {
        let output = expand(quote!("/Script/Fixture.Library"), quote! {
            #[cfg(feature = "outer")]
            impl Library {
                #[cfg(feature = "inner")]
                #[exec_observe(verified)]
                pub unsafe fn Call() {}
            }
        }).unwrap();
        let file = syn::parse2::<syn::File>(output).unwrap();
        for item in file.items {
            let attributes = match item {
                syn::Item::Static(item) => item.attrs,
                syn::Item::Fn(item) => item.attrs,
                syn::Item::Impl(item) => item.attrs,
                syn::Item::Macro(item) => item.attrs,
                _ => panic!("unexpected generated item"),
            };
            assert_eq!(cfg(&attributes).len(), 2);
        }
    }

    #[test]
    fn qualified_bool_inputs_always_decode_vm_bool32() {
        let output = expand(quote!("/Script/Fixture.Library"), quote! {
            impl Library {
                #[exec_observe(verified)]
                unsafe fn Call(value: std::primitive::bool) {}
            }
        }).unwrap().to_string();
        assert!(output.contains(". boolean ()"));
        assert!(!output.contains("borrow :: < std :: primitive :: bool >"));
    }
}
