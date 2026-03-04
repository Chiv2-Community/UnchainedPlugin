extern crate proc_macro;
use darling::{FromMeta, ast::NestedMeta};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, parse_macro_input};

#[derive(FromMeta)]
struct CommandArgs {
    name: String,
    #[darling(default, multiple, rename = "alias")]
    aliases: Vec<String>,
    #[darling(default)]
    sub: Option<String>,
    #[darling(default)]
    desc: String,
    #[darling(default)]
    game_thread: bool,
}

#[proc_macro_attribute]
pub fn command(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = match NestedMeta::parse_meta_list(args.into()) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(darling::Error::from(e).write_errors()),
    };
    
    let input_fn = parse_macro_input!(input as ItemFn);
    let cmd_args = match CommandArgs::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(e.write_errors()),
    };

    let fn_name = &input_fn.sig.ident;
    let visibility = &input_fn.vis;
    let name = cmd_args.name;
    let sub = match cmd_args.sub {
        Some(s) => quote! { Some(#s) },
        None => quote! { None },
    };
    let desc = cmd_args.desc;
    let game_thread = cmd_args.game_thread;    
    let aliases = cmd_args.aliases;

    let mut arg_parsers = Vec::new();
    let mut call_args = Vec::new();
    let mut param_descriptions = Vec::new();

    // Iterate through function arguments to build parsers and help strings
    // Inside the for loop in sleuth_macros/src/lib.rs
    for (i, arg) in input_fn.sig.inputs.iter().enumerate() {
        if let FnArg::Typed(pat_type) = arg {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                let arg_ident = &pat_ident.ident;
                let arg_type = &pat_type.ty;
                let type_str = quote!(#arg_type).to_string().replace(" ", "");

                param_descriptions.push(format!("<{}: {}>", arg_ident, type_str));

                // Logic: Check if it's an Option or Vec to handle FromStr issues
                let type_as_string = quote!(#arg_type).to_string();
                
                if type_as_string.contains("Option") {
                    arg_parsers.push(quote! {
                        let #arg_ident: #arg_type = args.get(#i)
                            .map(|s| s.parse().map_err(|_| ::anyhow::anyhow!("Invalid value for '{}'", stringify!(#arg_ident))))
                            .transpose()?; 
                    });
                } else if type_as_string.contains("Vec") {
                    // Greedy: take all remaining args
                    arg_parsers.push(quote! {
                        let #arg_ident: #arg_type = args.iter().skip(#i)
                            .map(|s| s.parse().map_err(|_| ::anyhow::anyhow!("Invalid value in list '{}'", stringify!(#arg_ident))))
                            .collect::<::std::result::Result<#arg_type, _>>()?;
                    });
                } else {
                    // Standard required argument
                    arg_parsers.push(quote! {
                        let #arg_ident: #arg_type = args.get(#i)
                            .ok_or_else(|| ::anyhow::anyhow!("Missing argument '{}'", stringify!(#arg_ident)))?
                            .parse()
                            .map_err(|_| ::anyhow::anyhow!("Invalid value for '{}' (expected {})", stringify!(#arg_ident), #type_str))?;
                    });
                }
                call_args.push(quote! { #arg_ident });
            }
        }
    }

    let params_str = param_descriptions.join(" ");
    let wrapper_name = format_ident!("__wrap_{}", fn_name);

    let expanded = quote! {
        // Original function remains for IDE/LSP hinting
        #visibility #input_fn

        // Generated wrapper with Fully Qualified Paths
        fn #wrapper_name(args: ::std::vec::Vec<::std::string::String>) -> ::anyhow::Result<()> {
            #(#arg_parsers)*
            #fn_name(#(#call_args),*)
        }

        ::inventory::submit! {
            crate::commands::ConsoleCommand {
                name: #name,
                aliases: &[#(#aliases),*],
                subcommand: #sub,
                description: #desc,
                params: #params_str,
                game_thread_required: #game_thread,
                handler: #wrapper_name,
            }
        }
    };

    TokenStream::from(expanded)
}

use syn::{ImplItemFn};
use syn::{LitStr, Token, parse::{Parse, ParseStream}};

// parse the macro attributes: #[handler_command(name = "...", desc = "...")]
struct HandlerArgs {
    name: String,
    desc: String,
    source: Option<String>,
    elevated: bool,
}

impl Parse for HandlerArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = String::new();
        let mut desc = String::new();
        let mut source = None;
        let mut elevated = false;

        // check if the first thing is just a string literal: #[handler_command("yes", ...)]
        if input.peek(LitStr) {
            let val: LitStr = input.parse()?;
            name = val.value();
            // If there's more, skip the comma and move to named arguments
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }

        // parse remaining named arguments: desc = "...", etc.
        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            let _: Token![=] = input.parse()?;
            
            match ident.to_string().as_str() {
                "name" => {
                    let val: LitStr = input.parse()?;
                    name = val.value();
                }
                "desc" => {
                    let val: LitStr = input.parse()?;
                    desc = val.value();
                }
                "source" => {
                    let val: LitStr = input.parse()?;
                    source = Some(val.value());
                }
                "elevated" => {
                    let val: syn::LitBool = input.parse()?;
                    elevated = val.value();
                }
                _ => return Err(syn::Error::new(ident.span(), "Unknown attribute key.")),
            }

            // 4. Handle the optional comma between attributes
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }

        if name.is_empty() {
            return Err(input.error("Command name is required."));
        }
        Ok(HandlerArgs { name, desc, source, elevated })
    }
}

#[proc_macro_attribute]
pub fn handler_command(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(args as HandlerArgs);
    let input_method = parse_macro_input!(input as ImplItemFn);

    let fn_name = &input_method.sig.ident;
    let wrapper_name = format_ident!("__wrap_{}", fn_name);
    let info_name = format_ident!("__info_{}", fn_name);
    
    let cmd_name = attr_args.name;
    let cmd_desc = attr_args.desc;

    let mut arg_parsers = Vec::new();
    let mut call_args = Vec::new();
    let mut usage_parts = vec![format!("!{}", cmd_name)];
    let mut arg_idx: usize = 0;

    for arg in &input_method.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                let arg_ident = &pat_ident.ident;
                let arg_type = &pat_type.ty;
                let type_str = quote!(#arg_type).to_string();

                if type_str.contains("GameCommand") {
                    call_args.push(quote! { cmd });
                } else {
                    // build Usage String: e.g. "<count: i32>"
                    usage_parts.push(format!("<{}: {}>", arg_ident, type_str));

                    arg_parsers.push(quote! {
                        let #arg_ident: #arg_type = cmd.args.get(#arg_idx)
                            .ok_or_else(|| ::anyhow::anyhow!("Missing argument '{}'", stringify!(#arg_ident)))?
                            .parse()
                            .map_err(|_| ::anyhow::anyhow!("Invalid value for '{}' (expected {})", stringify!(#arg_ident), #type_str))?;
                    });
                    call_args.push(quote! { #arg_ident });
                    arg_idx += 1;
                }
            }
        }
    }

    let source_check = if let Some(src) = &attr_args.source {
        let source_variant = format_ident!("{}", src);
        quote! {
            if cmd.source != crate::discord::events::CommandSource::#source_variant {
                return crate::discord::responses::msg(
                    format!("⚠️ This command can only be used from {}!", #src)
                ).into_responses();
            }
        }
    } else {
        quote! {}
    };

    let required_elevation = attr_args.elevated;

    let permission_check = if required_elevation {
        quote! {
            if !cmd.actor.is_elevated() {
                return crate::discord::responses::msg("🚫 You do not have permission to use this command.")
                    .into_responses();
            }
        }
    } else {
        quote! {}
    };


    let source_enum_val = if let Some(src) = &attr_args.source {
        let variant = format_ident!("{}", src);
        quote! { Some(crate::discord::events::CommandSource::#variant) }
    } else {
        quote! { None }
    };

    let usage_str = usage_parts.join(" ");
    
    let expanded = quote! {
        #input_method

        // metadata function for the help system
        pub fn #info_name(&self) -> crate::discord::responses::CommandInfo {
            crate::discord::responses::CommandInfo {
                name: #cmd_name.to_string(),
                description: #cmd_desc.to_string(),
                usage: #usage_str.to_string(),
                source: #source_enum_val,
                elevated: #required_elevation,
            }
        }

        pub fn #wrapper_name(&mut self, cmd: &crate::discord::events::GameCommand) -> Vec<crate::discord::responses::BotResponse> {
            #permission_check
            #source_check
            
            let result: ::anyhow::Result<Vec<crate::discord::responses::BotResponse>> = (|| {
                #(#arg_parsers)*
                Ok(self.#fn_name(#(#call_args),*))
            })();

            match result {
                Ok(resp) => resp,
                Err(e) => crate::discord::responses::msg(format!("⚠️ {}", e)).into_responses()
            }
        }
    };

    TokenStream::from(expanded)
}
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

    let log_level = if is_optional { quote!(debug) } else { quote!(error) };

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
                    ::log::#log_level!(
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
                                    ::log::error!("Sleuth: {} failed: {:?}", stringify!(#name), e);
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