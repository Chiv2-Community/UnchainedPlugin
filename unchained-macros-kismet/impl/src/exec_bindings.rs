use crate::cfg_attributes;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{FnArg, ReturnType, Type};

fn value_type(ty: &Type) -> syn::Result<Type> {
    if let Type::Reference(reference) = ty {
        if reference.mutability.is_some() || reference.lifetime.is_some() {
            return Err(syn::Error::new_spanned(
                ty,
                "exec observations require immutable call-scoped borrows",
            ));
        }

        Ok((*reference.elem).clone())
    } else {
        Ok(ty.clone())
    }
}

fn parameter(ty: &Type) -> syn::Result<Tokens> {
    let kind = match ty {
        Type::Ptr(pointer) if matches!(&*pointer.elem, Type::Path(path) if path.path.segments.last().unwrap().ident == "UObject") => {
            "ObjectProperty"
        }

        Type::Path(path) => match path.path.segments.last().unwrap().ident.to_string().as_str() {
            "bool" => "BoolProperty",
            "u8" => "ByteProperty",
            "i32" => "IntProperty",
            "f32" => "FloatProperty",
            "FString" => "StrProperty",
            "FText" => "TextProperty",
            "FLinearColor" => "StructProperty:LinearColor",
            _ => return Err(syn::Error::new_spanned(ty, "unsupported exec observation type")),
        },

        _ => return Err(syn::Error::new_spanned(ty, "unsupported exec observation type")),
    };

    Ok(quote!(::unchained_macros_kismet::schema::Parameter {
        kind: #kind, size: ::core::mem::size_of::<#ty>(),
    }))
}

pub fn expand_method(
    owner: &syn::LitStr,
    class: &syn::Ident,
    method: &syn::ImplItemFn,
) -> syn::Result<(Vec<syn::ImplItem>, Tokens)> {
    let signature = &method.sig;

    if signature.unsafety.is_none()
        || signature.abi.is_some()
        || signature.asyncness.is_some()
        || signature.constness.is_some()
        || signature.variadic.is_some()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || !method.block.stmts.is_empty()
    {
        return Err(syn::Error::new_spanned(
            method,
            "use an empty unsafe non-generic exec declaration without an ABI",
        ));
    }

    let annotations: Vec<_> = method.attrs.iter().filter(|a| a.path().is_ident("dummy")).collect();

    if annotations.len() != 1 {
        return Err(syn::Error::new_spanned(
            method,
            "exec observation requires exactly one #[dummy] declaration and current binary evidence",
        ));
    }

    if !matches!(&annotations[0].meta, syn::Meta::Path(_)) {
        return Err(syn::Error::new_spanned(annotations[0], "#[dummy] takes no parameters"));
    }

    let method_name = &signature.ident;
    let member = method_name.to_string();
    let prefix = format!("__UNCHAINED_EXEC_{class}_{method_name}");
    let resolver = format_ident!("{prefix}_RESOLVER");
    let arguments = format_ident!("{prefix}_ARGUMENTS");
    let binding = format_ident!("{prefix}_BINDING");
    let descriptor = format_ident!("{prefix}_LISTENERS");
    let dispatch = format_ident!("{prefix}_dispatch");
    let entry = format_ident!("{prefix}_entry");
    let associated = format_ident!("__UNCHAINED_TARGET_{method_name}");
    let visibility = &method.vis;

    let mut attributes = method.attrs.clone();
    attributes.retain(|a| !a.path().is_ident("dummy"));
    let attributes_cfg: Vec<_> = cfg_attributes(&attributes).collect();
    let mut types = Vec::new();
    let mut parameters = Vec::new();
    let mut decode = Vec::new();
    let mut borrowed = Vec::new();

    for (index, input) in signature.inputs.iter().enumerate() {
        let FnArg::Typed(input) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "exec declarations have explicit inputs, not Rust receivers",
            ));
        };

        let ty = value_type(&input.ty)?;
        let local = format_ident!("argument_{index}");

        parameters.push(parameter(&ty)?);

        if matches!(&ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "bool")) {
            decode.push(quote! {
                let #local = unsafe { inputs.get(#index).expect("validated exec argument").boolean() };
            });

            borrowed.push(quote!(&#local));
        } else {
            decode.push(quote! {
                let #local = inputs.get(#index).expect("validated exec argument");
                let #local = unsafe { #local.borrow::<#ty>() };
            });
            borrowed.push(quote!(#local));
        }

        types.push(ty);
    }

    let result_parameter = match &signature.output {
        ReturnType::Default => quote!(None),
        ReturnType::Type(_, ty) if matches!(&**ty, Type::Tuple(tuple) if tuple.elems.is_empty()) => quote!(None),
        ReturnType::Type(_, ty) if matches!(&**ty, Type::Reference(_)) => {
            return Err(syn::Error::new_spanned(ty, "exec result references are unsupported"))
        }

        ReturnType::Type(_, ty) => {
            let parameter = parameter(ty)?;

            quote!(Some(#parameter))
        }
    };

    let listener_type = quote!(::unchained_macros_kismet::DummyExecTarget<unsafe fn(#(&#types),*)>);

    let metadata = vec![
        syn::parse2(quote! {
            #(#attributes)*
            #[allow(non_upper_case_globals)]
            #[doc = "Entry-only dummy observation marker. This is not a native-call binding."]
            #visibility const #method_name: () = ();
        })?,
        syn::parse2(quote! {
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            #[doc(hidden)]
            #visibility const #associated: &'static #listener_type = &#descriptor;
        })?,
    ];

    Ok((
        metadata,
        quote! {
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #arguments: &[::unchained_macros_kismet::schema::Parameter] = &[#(#parameters),*];
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #resolver: ::unchained_macros_core::Resolver<::unchained_macros_kismet::KismetExecResolver> = {
                ::unchained_macros_core::__private::inventory::submit! {
                    &#resolver as &dyn ::unchained_macros_core::__private::ResolverInfo
                }
                ::unchained_macros_core::Resolver {
                    name: concat!(stringify!(#class), "__", #member, "__exec"),
                    implementation: unsafe { ::unchained_macros_kismet::KismetExecResolver::new(
                        #owner, #member, #arguments, #result_parameter,
                    ) },
                    filter: None,
                }
            };

            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #binding: ::unchained_macros_core::FunctionBinding<::unchained_macros_kismet::KismetExecResolver> = {
                ::unchained_macros_core::__private::inventory::submit! {
                    &#binding as &dyn ::unchained_macros_core::__private::FunctionInfo
                }
                ::unchained_macros_core::FunctionBinding::new(concat!(stringify!(#class), "::", #member, " [exec]"), &#resolver)
            };
            #(#attributes_cfg)*
            #[allow(non_upper_case_globals)]
            static #descriptor: #listener_type =
                unsafe { ::unchained_macros_kismet::DummyExecTarget::new(&#binding, #dispatch) };
            #(#attributes_cfg)*
            #[allow(non_snake_case)]
            unsafe fn #entry(inputs: ::unchained_macros_kismet::observation::Inputs<'_>) {
                use ::unchained_macros_core::SupportsEntry as _;
                #descriptor.observe_entry(|callback| {
                    #(#decode)*
                    unsafe { callback(#(#borrowed),*); }
                });
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

                unsafe {
                    ::unchained_macros_kismet::observation::observe_exec(
                        frame, #resolver.implementation.properties(),
                        Some(#entry),
                        || original(context, frame, result),
                    );
                }
            }

        },
    ))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::expand;

    #[test]

    fn dummy_marker_keeps_exec_dispatch() {
        let output = expand(
            quote!("/Script/Fixture.Library"),
            quote! {
                impl Library {
                    #[dummy]
                    pub unsafe fn PrintWarning(message: &FString) {}
                }
            },
        )
        .unwrap()
        .to_string();

        assert!(output.contains("DummyExecTarget"));

        assert!(!output.contains("ExitObservation"));

        assert!(!output.contains("prepare_exit"));

        assert!(!output.contains("finish_exit"));

        assert!(output.contains("observe_exec"));

        assert!(output.contains("original (context , frame , result)"));

        assert!(!output.contains("RawDetour"));

        syn::parse2::<syn::File>(output.parse().unwrap()).unwrap();
    }

    #[test]

    fn dummy_rejects_parameters_and_duplicates() {
        for attributes in [
            quote!(#[dummy()]),
            quote!(#[dummy(verified)]),
            quote!(#[dummy(verified, dummied)]),
            quote!(#[dummy = "verified"]),
            quote!(#[dummy] #[dummy]),
        ] {
            assert!(expand(
                quote!("/Script/Fixture.Library"),
                quote! {
                    impl Library { #attributes unsafe fn Call() {} }
                }
            )
            .is_err());
        }
    }

    #[test]

    fn unsupported_results_and_mutable_borrows_are_rejected() {
        for declaration in [
            quote!(impl Library { #[dummy] unsafe fn Call(value: &mut FString) {} }),
            quote!(impl Library { #[dummy] unsafe fn Call() -> UnsupportedAggregate {} }),
            quote!(impl Library { #[dummy] unsafe fn Call(&self) {} }),
            quote!(impl Library { #[dummy] unsafe fn Call() { todo!() } }),
            quote!(impl Library { #[dummy] unsafe fn Call<T>(value: T) {} }),
        ] {
            assert!(expand(quote!("/Script/Fixture.Library"), declaration).is_err());
        }
    }

    #[test]

    fn cfgs_cover_resolver_descriptor_dispatch_and_inventory() {
        let output = expand(
            quote!("/Script/Fixture.Library"),
            quote! {
                #[cfg(feature = "outer")]
                impl Library {
                    #[cfg(feature = "inner")]
                    #[dummy]
                    pub unsafe fn Call() {}
                }
            },
        )
        .unwrap();

        let file = syn::parse2::<syn::File>(output).unwrap();

        let syn::Item::Const(scope) = &file.items[0] else {
            panic!("expected isolated helpers")
        };

        assert_eq!(cfg_attributes(&scope.attrs).count(), 1);

        let syn::Expr::Block(block) = &*scope.expr else {
            panic!("expected block")
        };

        for statement in &block.block.stmts {
            if let syn::Stmt::Macro(item) = statement {
                assert_eq!(cfg_attributes(&item.attrs).count(), 1);

                continue;
            }

            let syn::Stmt::Item(item) = statement else {
                panic!("expected item")
            };

            let attributes = match item {
                syn::Item::Static(item) => &item.attrs,
                syn::Item::Fn(item) => &item.attrs,
                syn::Item::Impl(item) => &item.attrs,
                syn::Item::Macro(item) => &item.attrs,
                _ => panic!("unexpected generated item"),
            };

            if matches!(item, syn::Item::Impl(_))
                && attributes.iter().any(|attribute| {
                    attribute
                        .path()
                        .segments
                        .last()
                        .is_some_and(|segment| segment.ident == "runtime_bindings")
                })
            {
                let syn::Item::Impl(implementation) = item else {
                    unreachable!()
                };

                assert_eq!(cfg_attributes(&implementation.attrs).count(), 1);

                for associated in &implementation.items {
                    let syn::ImplItem::Const(item) = associated else {
                        panic!("expected dummy metadata")
                    };

                    assert_eq!(cfg_attributes(&item.attrs).count(), 1);
                }

                continue;
            }

            assert_eq!(cfg_attributes(attributes).count(), 1);
        }
    }

    #[test]

    fn qualified_bool_inputs_always_decode_vm_bool32() {
        let output = expand(
            quote!("/Script/Fixture.Library"),
            quote! {
                impl Library {
                    #[dummy]
                    unsafe fn Call(value: std::primitive::bool) {}
                }
            },
        )
        .unwrap()
        .to_string();

        assert!(output.contains(". boolean ()"));

        assert!(!output.contains("borrow :: < std :: primitive :: bool >"));
    }
}
