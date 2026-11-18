//! URLs used by the game's native HTTP transport.

pub(crate) fn tbio_base_url(base_url: &str) -> String {
    format!("{}/api/tbio", base_url.trim_end_matches('/'))
}

pub(crate) fn playfab_matchmake_url(base_url: &str) -> String {
    format!("{}/api/playfab/Client/Matchmake", base_url.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    #[test_case::test_case("https://example.test", "https://example.test"; "root")]
    #[test_case::test_case("https://example.test/", "https://example.test"; "trailing_slash")]
    #[test_case::test_case("https://example.test/browser///", "https://example.test/browser"; "deployment_prefix")]
    fn native_compatibility_urls(base: &str, prefix: &str) {
        assert_eq!(super::tbio_base_url(base), format!("{prefix}/api/tbio"));
        assert_eq!(
            super::playfab_matchmake_url(base),
            format!("{prefix}/api/playfab/Client/Matchmake")
        );
    }
}
