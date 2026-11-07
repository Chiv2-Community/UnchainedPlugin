// Also runnable without Windows dependencies:
// rustc --edition=2021 --test tests/steam_auth_policy.rs -o <test-binary>
#[path = "../src/resolvers/steam_auth/policy.rs"]
mod policy;
#[path = "../src/resolvers/steam_auth/signatures.rs"]
mod signatures;

#[test]
#[ignore = "requires a local copy of the supported Steam executable"]
fn supported_executable_has_unique_signatures() {
    let path = std::env::var_os("STEAM_AUTH_TEST_EXE").expect("Set STEAM_AUTH_TEST_EXE");
    let binary = std::fs::read(path).unwrap();
    for signature in [signatures::NODE_SIGNATURE, signatures::TOKEN_SIGNATURE] {
        let prefix: Vec<u8> = signature
            .split_ascii_whitespace()
            .take(10)
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect();
        let hits: Vec<usize> = binary
            .windows(prefix.len())
            .enumerate()
            .filter(|(_, bytes)| *bytes == prefix)
            .map(|(offset, _)| offset)
            .filter(|&offset| policy::signature_matches(&binary[offset..], signature))
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "signature must resolve exactly once: {hits:x?}"
        );
    }
}
