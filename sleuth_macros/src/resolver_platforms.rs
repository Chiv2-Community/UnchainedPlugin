use quote::quote;
use syn::{bracketed, parse::Parse, punctuated::Punctuated, Ident, Token};

pub(crate) struct Platforms {
    name: Ident,
    supplied: Punctuated<Ident, Token![,]>,
    allowed_missing: Punctuated<Ident, Token![,]>,
}

impl Parse for Platforms {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![,]>()?;
        let supplied;
        bracketed!(supplied in input);
        let supplied = supplied.parse_terminated(Ident::parse, Token![,])?;
        input.parse::<Token![,]>()?;
        let allowed_missing;
        bracketed!(allowed_missing in input);
        let allowed_missing = allowed_missing.parse_terminated(Ident::parse, Token![,])?;
        Ok(Self {
            name,
            supplied,
            allowed_missing,
        })
    }
}

pub(crate) fn expand(platforms: Platforms) -> syn::Result<proc_macro2::TokenStream> {
    let Platforms {
        name,
        supplied,
        allowed_missing,
    } = platforms;
    for platform in supplied.iter().chain(&allowed_missing) {
        if !matches!(
            platform.to_string().as_str(),
            "STEAM" | "EGS" | "XBOX" | "OTHER"
        ) {
            return Err(syn::Error::new(
                platform.span(),
                "unknown resolver platform",
            ));
        }
    }
    let shared = supplied.iter().any(|p| p == "OTHER");
    let mut warnings = Vec::new();
    for platform in ["STEAM", "EGS"] {
        if !shared
            && !supplied.iter().any(|p| p == platform)
            && !allowed_missing.iter().any(|p| p == platform)
        {
            let note = format!("{name}: {platform} signatures weren't supplied; add #[allow_missing_platforms({platform})] inside define_pattern_resolver! if intentional");
            // Rust has no stable custom warning API. Use a local deprecated item
            // so the warning respects normal lint levels without deprecating the resolver.
            warnings.push(quote! {
                const _: () = {
                    #[deprecated(note = #note)]
                    const MISSING_PLATFORM_SIGNATURE: () = ();
                    let _ = MISSING_PLATFORM_SIGNATURE;
                };
            });
        }
    }
    let supplied = supplied.iter();
    Ok(quote! {
        inventory::submit! {
            crate::resolvers::ResolverPlatforms {
                name: stringify!(#name),
                platforms: &[#(crate::resolvers::PlatformType::#supplied),*],
            }
        }
        #(#warnings)*
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expansion(input: &str) -> String {
        expand(syn::parse_str(input).unwrap()).unwrap().to_string()
    }

    #[test]
    fn missing_egs_warns() {
        let output = expansion("SteamOnly, [STEAM], []");
        assert!(output.contains("EGS signatures weren't supplied"));
        assert!(!output.contains("STEAM signatures weren't supplied"));
    }

    #[test]
    fn missing_steam_warns() {
        assert!(expansion("EpicOnly, [EGS], []").contains("STEAM signatures weren't supplied"));
    }

    #[test]
    fn explicit_suppression_does_not_add_coverage() {
        let output = expansion("SteamOnly, [STEAM], [EGS]");
        assert!(!output.contains("deprecated"));
        assert!(!output.contains("PlatformType :: EGS"));
    }

    #[test]
    fn shared_and_complete_coverage_do_not_warn() {
        for input in ["Shared, [OTHER], []", "Both, [STEAM, EGS], []"] {
            assert!(!expansion(input).contains("deprecated"));
        }
    }

    #[test]
    fn unknown_platform_is_an_error() {
        assert!(expand(syn::parse_str("Bad, [STEEM], []").unwrap()).is_err());
        assert!(expand(syn::parse_str("Bad, [STEAM], [EPIC]").unwrap()).is_err());
    }
}
