extern crate proc_macro;
use darling::{FromMeta, ast::NestedMeta};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, LitInt, LitStr, Pat, parse_macro_input};

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
