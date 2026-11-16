#[path = "../../src/features/game_join_protocol.rs"]
mod protocol;

#[test_case::test_case("https://example.test", true; "explicit_https")]
#[test_case::test_case("http://example.test", false; "plaintext_public")]
#[test_case::test_case("http://127.0.0.1:18080", true; "loopback_fixture")]
#[test_case::test_case("http://[::1]:18080", true; "ipv6_loopback")]
#[test_case::test_case("https://secret@example.test", false; "userinfo")]
#[test_case::test_case("https://example.test?q=secret", false; "query")]
#[test_case::test_case("https://example.test#fragment", false; "fragment")]
fn trust(browser: &str, allowed: bool) {
    assert_eq!(protocol::trusted_backend(browser).is_some(), allowed);
}

#[test]
fn join_options_and_playfab_id_bounds_match_admission() {
    let ticket = protocol::encode_ticket("server", &"a".repeat(43)).unwrap();
    let url = |id: &str| format!("127.0.0.1:7777?PlayFabId={id}?UnofficialTicket={ticket}");
    let at_id_limit = url(&"A".repeat(64));
    assert!(protocol::target(&at_id_limit).is_some());
    assert!(protocol::admission_options(&at_id_limit));
    let oversized_id = url(&"A".repeat(65));
    assert!(protocol::target(&oversized_id).is_none());
    assert!(!protocol::admission_options(&oversized_id));

    let mut at_options_limit = format!("{}?Padding=", url("ABC"));
    at_options_limit.push_str(&"x".repeat(16 * 1024 - at_options_limit.len()));
    assert!(protocol::target(&at_options_limit).is_some());
    assert!(protocol::admission_options(&at_options_limit));
    at_options_limit.push('x');
    assert!(protocol::target(&at_options_limit).is_none());
    assert!(!protocol::admission_options(&at_options_limit));
}

#[test]
fn versioned_ticket_and_url_round_trip() {
    let ticket = "a".repeat(43);
    let encoded = protocol::encode_ticket("server-id", &ticket).unwrap();
    assert_eq!(
        protocol::decode_ticket(&encoded),
        Some(("server-id", ticket.as_str()))
    );
    let original = "127.0.0.1:7777?PlayFabId=ABC?UnofficialTicket=old?Password=a%2Bb?unofficialticket=duplicate";
    let replaced = protocol::replace_ticket(original, &encoded);
    assert_eq!(
        protocol::target(&replaced).unwrap().to_string(),
        "127.0.0.1:7777"
    );
    assert!(replaced.contains("?Password=a%2Bb"));
    assert!(!replaced.contains("old") && !replaced.contains("duplicate"));
    assert!(protocol::admission_options(&replaced));
    assert!(!protocol::admission_options(&format!(
        "{replaced}?PlayFabId=DEF"
    )));
    assert!(!protocol::admission_options(&format!(
        "{replaced}?UnofficialTicket={encoded}"
    )));
    assert!(!protocol::admission_options(original));
    assert!(protocol::target("[::1]:7777?PlayFabId=ABC").is_some());
    for invalid in [
        "127.0.0.1:7777",
        "Frontend?PlayFabId=ABC",
        "127.0.0.1:7777?PlayFabId=ABC?PlayFabId=DEF",
        "127.0.0.1:0?PlayFabId=ABC",
    ] {
        assert!(protocol::target(invalid).is_none());
    }
    for invalid in [
        "ucj1.server.bad",
        "ucj2.server.bad",
        "ucj1..bad",
        "ucj1.server.a.b",
    ] {
        assert!(protocol::decode_ticket(invalid).is_none());
    }
}
#[test]
fn signed_admission_shape_is_bounded_and_not_authentication() {
    assert!(protocol::admission_ticket("header.claims.signature"));
    for invalid in [
        "",
        "a.b",
        "a.b.c.d",
        ".b.c",
        "a..c",
        "a.b.",
        "a.b.c=",
        "a.b.c?Extra=x",
    ] {
        assert!(!protocol::admission_ticket(invalid));
    }
    assert!(!protocol::admission_ticket(&format!(
        "a.b.{}",
        "c".repeat(16384)
    )));
    let replaced =
        protocol::replace_ticket("127.0.0.1:7777?PlayFabId=ABC", "header.claims.signature");
    assert!(protocol::admission_options(&replaced));
}

#[test]
fn public_proof_matches_backend_vector_and_binds_every_field() {
    let challenge = "c".repeat(43);
    let commitment = "d".repeat(64);
    let proof = protocol::proof_value(
        "https://browser.example",
        "server-1",
        "abc123",
        &challenge,
        &commitment,
    )
    .unwrap();
    assert_eq!(protocol::PROOF_KEY, "UnchainedJoin_v1");
    assert_eq!(
        proof,
        "uc-proof-v1:6205faeb8b646982ebe2fbc7aaa3c6e34d7ed72f03222965824407f5342557d8"
    );
    assert_ne!(
        Some(proof.clone()),
        protocol::proof_value(
            "https://other.example",
            "server-1",
            "ABC123",
            &challenge,
            &commitment
        )
    );
    assert_ne!(
        Some(proof.clone()),
        protocol::proof_value(
            "https://browser.example",
            "server-2",
            "ABC123",
            &challenge,
            &commitment
        )
    );
    assert_ne!(
        Some(proof.clone()),
        protocol::proof_value(
            "https://browser.example",
            "server-1",
            "DEF",
            &challenge,
            &commitment
        )
    );
    assert_ne!(
        Some(proof.clone()),
        protocol::proof_value(
            "https://browser.example",
            "server-1",
            "ABC123",
            &"e".repeat(43),
            &commitment
        )
    );
    assert_ne!(
        Some(proof),
        protocol::proof_value(
            "https://browser.example",
            "server-1",
            "ABC123",
            &challenge,
            &"e".repeat(64)
        )
    );
    assert!(protocol::proof_value(
        "http://untrusted.example",
        "server-1",
        "ABC123",
        &challenge,
        &commitment
    )
    .is_none());
    assert_eq!(
        protocol::claimed_account("127.0.0.1:7777?PlayFabId=ABC"),
        Some("ABC")
    );
}
