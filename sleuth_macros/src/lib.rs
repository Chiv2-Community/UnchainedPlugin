extern crate proc_macro;
use proc_macro::TokenStream;

/* -- SIGNATURES -- */

#[proc_macro_attribute]
pub fn resolver(args: TokenStream, input: TokenStream) -> TokenStream {
    use quote::quote;
    use syn::{parse_macro_input, ItemFn, Ident, LitStr, LitInt};

    // 1. Parse resolver mode (e.g., Simple, Call)
    let mode_ident = parse_macro_input!(args as Ident);
    let mode = mode_ident.to_string();

    let input_fn = parse_macro_input!(input as ItemFn);
    let name = &input_fn.sig.ident;
    let vis = &input_fn.vis;

    // 2. Containers for attributes
    let mut global_patterns = Vec::new();
    let mut steam_patterns = Vec::new();
    let mut egs_patterns = Vec::new();
    let mut priority_patterns = Vec::new();

    let mut offset_val = quote!(0usize);
    let mut read_rip4 = false;
    let mut validate_byte: Option<u8> = None;
    let mut is_optional = false;

    // 3. Parse Attributes
    for attr in &input_fn.attrs {
        if attr.path().is_ident("pattern") {
            // Handle #[pattern("string")]
            if let Ok(lit) = attr.parse_args::<LitStr>() {
                global_patterns.push(lit.value());
            } else {
                // Handle #[pattern(STEAM = "string", EGS = "string", priority(1, "string"))]
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("priority") {
                        let content;
                        syn::parenthesized!(content in meta.input);
                        let p: LitInt = content.parse()?;
                        content.parse::<syn::Token![,]>()?;
                        let s: LitStr = content.parse()?;
                        priority_patterns.push((p.base10_parse::<usize>()?, s.value()));
                    } else if meta.path.is_ident("STEAM") {
                        let s: LitStr = meta.value()?.parse()?;
                        steam_patterns.push(s.value());
                    } else if meta.path.is_ident("EGS") {
                        let s: LitStr = meta.value()?.parse()?;
                        egs_patterns.push(s.value());
                    }
                    Ok(())
                });
            }
        } else if attr.path().is_ident("offset") {
            if let Ok(lit) = attr.parse_args::<LitInt>() {
                offset_val = quote!(#lit);
            }
        } else if attr.path().is_ident("read_rip4") {
            read_rip4 = true;
        } else if attr.path().is_ident("validate") {
            if let Ok(lit) = attr.parse_args::<LitInt>() {
                validate_byte = Some(lit.base10_parse().unwrap());
            }
        } else if attr.path().is_ident("optional") {
            is_optional = true;
        }
    }

    priority_patterns.sort_by_key(|p| p.0);
    let priority_strings: Vec<String> = priority_patterns.into_iter().map(|p| p.1).collect();

    // 4. Build Inner Logic
    let inner_logic = if mode == "Simple" || mode == "Call" {
        let is_call = mode == "Call";
        
        // Logic to validate the byte if requested
        let validation_logic = if let Some(expected) = validate_byte {
            quote! {
                let actual = ctx.image().memory.u8(addr)?;
                if actual != #expected {
                    return Err(::patternsleuth::resolvers::ResolveError::Msg(
                        format!("Validation failed: expected 0x{:02X}, found 0x{:02X}", #expected, actual).into()
                    ));
                }
            }
        } else {
            quote! {}
        };

        // Logic for RIP relative addressing
        let rip_logic = if read_rip4 {
            quote! { addr = ctx.image().memory.rip4(addr)?; }
        } else {
            quote! {}
        };

        quote! {
            use patternsleuth::MemoryTrait;
            let mut sigs: Vec<&str> = vec![#(#global_patterns),*];
            sigs.extend_from_slice(&[#(#priority_strings),*]);
            
            match crate::resolvers::current_platform() {
                crate::resolvers::PlatformType::STEAM => sigs.extend_from_slice(&[#(#steam_patterns),*]),
                crate::resolvers::PlatformType::EGS => sigs.extend_from_slice(&[#(#egs_patterns),*]),
                _ => {}
            }

            let scans = ::patternsleuth::resolvers::futures::future::join_all(
                sigs.iter().map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;

            let mut addr: usize = if #is_call {
                ::patternsleuth::resolvers::try_ensure_one::<usize>(
                    scans.iter().flatten().map(|a| Ok(ctx.image().memory.rip4(*a)?))
                )?
            } else {
                ::patternsleuth::resolvers::ensure_one::<usize>(
                    scans.into_iter().flatten()
                )?
            };

            addr += #offset_val;
            #rip_logic
            #validation_logic
            
            Ok(addr) // The missing piece in your expansion!
        }
    } else {
        let block = &input_fn.block;
        quote! { (async move #block).await }
    };

    let log_level = if is_optional { quote!(sdebug) } else { quote!(serror) };

    // 5. Final Expansion
    let expanded = quote! {
        #[derive(Debug, Clone, Copy, ::serde::Serialize, ::serde::Deserialize, PartialEq, Eq)]
        #vis struct #name(pub usize);

        ::patternsleuth::resolvers::impl_resolver_singleton!(
            all,
            #name,
            |ctx| async {
                let res: ::patternsleuth::resolvers::Result<usize> = {
                    #inner_logic
                };

                let wrapped = res.map(#name);
                if let Err(e) = &wrapped {
                    crate::#log_level!(
                        "Sleuth: {} failed to resolve: {:?}",
                        stringify!(#name),
                        e
                    );
                }
                wrapped
            }
        );
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn custom_resolver(_args: TokenStream, input: TokenStream) -> TokenStream {
    use quote::quote;
    use syn::{parse_macro_input, ItemFn, LitStr, LitInt};

    let input_fn = parse_macro_input!(input as ItemFn);
    let name = &input_fn.sig.ident;
    let vis = &input_fn.vis;
    let block = &input_fn.block;

    let mut global_patterns = Vec::new();
    let mut steam_patterns = Vec::new();
    let mut egs_patterns = Vec::new();
    let mut offset_val = quote!(0usize);
    let mut read_rip4 = false;
    let mut validate_byte = quote!(None::<u8>);

    for attr in &input_fn.attrs {
        if attr.path().is_ident("pattern") {
            if let Ok(lit) = attr.parse_args::<LitStr>() {
                global_patterns.push(lit.value());
            } else {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("STEAM") {
                        let s: LitStr = meta.value()?.parse()?;
                        steam_patterns.push(s.value());
                    } else if meta.path.is_ident("EGS") {
                        let s: LitStr = meta.value()?.parse()?;
                        egs_patterns.push(s.value());
                    }
                    Ok(())
                });
            }
        } else if attr.path().is_ident("offset") {
            if let Ok(lit) = attr.parse_args::<LitInt>() {
                offset_val = quote!(#lit);
            }
        } else if attr.path().is_ident("read_rip4") {
            read_rip4 = true;
        } else if attr.path().is_ident("validate") {
            if let Ok(lit) = attr.parse_args::<LitInt>() {
                validate_byte = quote!(Some(#lit));
            }
        }
    }

    TokenStream::from(quote! {
        #[derive(Debug, Clone, Copy, ::serde::Serialize, ::serde::Deserialize, PartialEq, Eq)]
        #vis struct #name(pub usize);

        impl ::patternsleuth::resolvers::Resolution for #name {
            fn typetag_name(&self) -> &'static str { stringify!(#name) }
            fn typetag_deserialize(&self) {}
        }
        impl ::patternsleuth::resolvers::Singleton for #name {
            fn get(&self) -> Option<usize> { Some(self.0) }
        }

        impl #name {
            pub fn resolver() -> &'static ::patternsleuth::resolvers::ResolverFactory<#name> {
                static GLOBAL: ::std::sync::OnceLock<::patternsleuth::resolvers::ResolverFactory<#name>> = ::std::sync::OnceLock::new();
                GLOBAL.get_or_init(|| {
                    ::patternsleuth::resolvers::ResolverFactory {
                        factory: |ctx| {
                            Box::pin(async move {
                                let mut active_sigs: Vec<&str> = vec![#(#global_patterns),*];
                                match crate::resolvers::current_platform() {
                                    crate::resolvers::PlatformType::STEAM => active_sigs.extend_from_slice(&[#(#steam_patterns),*]),
                                    crate::resolvers::PlatformType::EGS => active_sigs.extend_from_slice(&[#(#egs_patterns),*]),
                                    _ => {}
                                }

                                let offset: usize = #offset_val;
                                let should_read_rip4: bool = #read_rip4;
                                let validate_expected: Option<u8> = #validate_byte;

                                // FIX: Convert anyhow::Error to ResolveError::Msg
                                let res: ::patternsleuth::resolvers::Result<usize> = (async move #block)
                                    .await
                                    .map_err(|e| ::patternsleuth::resolvers::ResolveError::Msg(e.to_string().into()));
                                
                                let wrapped = res.map(#name);
                                if let Err(e) = &wrapped {
                                    crate::serror!("Sleuth: {} failed: {:?}", stringify!(#name), e);
                                }
                                wrapped
                            })
                        },
                    }
                })
            }

            pub fn dyn_resolver() -> &'static ::patternsleuth::resolvers::DynResolverFactory {
                static GLOBAL: ::std::sync::OnceLock<::patternsleuth::resolvers::DynResolverFactory> = ::std::sync::OnceLock::new();
                GLOBAL.get_or_init(|| {
                    ::patternsleuth::resolvers::DynResolverFactory {
                        factory: |ctx| Box::pin(async move {
                            ctx.resolve(Self::resolver()).await.map(|ok| ok as ::std::sync::Arc<dyn ::patternsleuth::resolvers::Resolution>)
                        }),
                    }
                })
            }
        }

        ::inventory::submit! {
            ::patternsleuth::resolvers::NamedResolver {
                name: stringify!(#name),
                getter: #name::dyn_resolver,
            }
        }
    })
}

/* -- DETOURS (ergonomic attribute wrappers over the plugin's CREATE_HOOK! / CALL_ORIGINAL!) -- */
//
// These transform an annotated function into a `crate::CREATE_HOOK!` invocation (NONE mode), so
// they reuse the existing retour/define_pattern_resolver!/HookRegistration pipeline. They are for
// ORDINARY functions resolved by a `define_pattern_resolver!` whose name is `Resolver`; they are
// NOT for Blueprint-VM exec thunks (use kismet_exec_observe!/kismet_exec_forward! for those).
//
//   #[detour_add_listener(Resolver)]            // observe AFTER the call; returns the original value
//   fn foo(a: A, b: B) -> R { /* `result: &R` and the args are in scope */ }
//
//   #[detour_around(Resolver)]                  // you drive; `foo_original(args)` is the un-hooked path
//   fn foo(a: A, b: B) -> R { /* ...; */ foo_original(a, b) }
//
// Caller contract: calling the hooked function runs the MODIFIED behavior for everyone; the
// generated `*_original` (retour trampoline) is the UNMODIFIED path. `detour_replace` is an alias
// of `detour_around`.

enum DetourMode {
    Around,
    AddListener,
}

fn detour_impl(args: TokenStream, input: TokenStream, mode: DetourMode) -> TokenStream {
    use quote::{format_ident, quote};
    use syn::{FnArg, Ident, ItemFn, Pat, ReturnType, Type};

    let resolver = match syn::parse::<Ident>(args) {
        Ok(id) => id,
        Err(e) => return e.to_compile_error().into(),
    };
    let func = match syn::parse::<ItemFn>(input) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error().into(),
    };

    let fn_name = &func.sig.ident;
    let body = &func.block;
    let ret: Type = match &func.sig.output {
        ReturnType::Type(_, ty) => (**ty).clone(),
        ReturnType::Default => syn::parse_quote!(()),
    };

    let mut decls = Vec::new();
    let mut names = Vec::new();
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Typed(pt) => {
                let ty = &pt.ty;
                let name = match &*pt.pat {
                    Pat::Ident(pi) => pi.ident.clone(),
                    other => {
                        return syn::Error::new_spanned(other, "detour args must be simple identifiers")
                            .to_compile_error()
                            .into()
                    }
                };
                decls.push(quote!(#name: #ty));
                names.push(name);
            }
            FnArg::Receiver(r) => {
                return syn::Error::new_spanned(r, "detour fns cannot take `self`")
                    .to_compile_error()
                    .into()
            }
        }
    }
    if decls.is_empty() {
        return syn::Error::new_spanned(
            &func.sig,
            "detour fns need at least one argument (CREATE_HOOK! requires args)",
        )
        .to_compile_error()
        .into();
    }

    let generated = match mode {
        DetourMode::Around => {
            let original = format_ident!("{}_original", fn_name);
            quote! {
                crate::CREATE_HOOK!(#resolver, ACTIVE, NONE, #ret, ( #(#decls),* ), {
                    // The un-hooked path (retour trampoline), named `<fn>_original`.
                    #[allow(unused)]
                    let #original = | #(#decls),* | -> #ret {
                        crate::CALL_ORIGINAL!(#resolver( #(#names),* ))
                    };
                    #body
                });
            }
        }
        DetourMode::AddListener => {
            quote! {
                crate::CREATE_HOOK!(#resolver, ACTIVE, NONE, #ret, ( #(#decls),* ), {
                    let __detour_ret: #ret = crate::CALL_ORIGINAL!(#resolver( #(#names),* ));
                    {
                        // Observe the original's result without modifying it.
                        #[allow(unused)]
                        let result = &__detour_ret;
                        #body
                    };
                    __detour_ret
                });
            }
        }
    };
    generated.into()
}

#[proc_macro_attribute]
pub fn detour_around(args: TokenStream, input: TokenStream) -> TokenStream {
    detour_impl(args, input, DetourMode::Around)
}

/// Alias of [`detour_around`].
#[proc_macro_attribute]
pub fn detour_replace(args: TokenStream, input: TokenStream) -> TokenStream {
    detour_impl(args, input, DetourMode::Around)
}

#[proc_macro_attribute]
pub fn detour_add_listener(args: TokenStream, input: TokenStream) -> TokenStream {
    detour_impl(args, input, DetourMode::AddListener)
}
