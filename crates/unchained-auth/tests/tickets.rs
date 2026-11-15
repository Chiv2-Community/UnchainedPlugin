use std::sync::OnceLock;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rsa::{pkcs8::EncodePrivateKey, traits::PublicKeyParts, RsaPrivateKey};
use serde_json::{json, Value};
use unchained_auth::{AuthError, TicketVerifier};

const ISSUER: &str = "https://auth.test";
const SERVER: &str = "00000000-0000-4000-8000-000000000001";
const KID: &str = "00000000-0000-4000-8000-000000000002";
const JTI: &str = "00000000-0000-4000-8000-000000000003";
const OTHER_JTI: &str = "00000000-0000-4000-8000-000000000004";
const NOW: u64 = 1_800_000_000;

struct SigningMaterial {
    key: EncodingKey,
    jwks: Vec<u8>,
}

fn material() -> &'static SigningMaterial {
    static MATERIAL: OnceLock<SigningMaterial> = OnceLock::new();
    MATERIAL.get_or_init(|| {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let pem = private.to_pkcs8_pem(Default::default()).unwrap();
        let jwks = json!({"keys": [{
            "kty": "RSA", "alg": "RS256", "use": "sig", "kid": KID,
            "n": URL_SAFE_NO_PAD.encode(private.n().to_bytes_be()), "e": "AQAB"
        }]});
        SigningMaterial {
            key: EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            jwks: serde_json::to_vec(&jwks).unwrap(),
        }
    })
}

fn claims() -> Value {
    json!({
        "iss": ISSUER, "sub": "123456789012345678", "aud": format!("urn:unchained:server:{SERVER}"),
        "purpose": "unchained.join", "auth_time": NOW - 100,
        "iat": NOW, "nbf": NOW, "exp": NOW + 60, "jti": JTI
    })
}

fn header() -> Header {
    let mut header = Header::new(Algorithm::RS256);
    header.typ = Some("unchained-join+jwt".into());
    header.kid = Some(KID.into());
    header
}

fn token(claims: &Value) -> String {
    encode(&header(), claims, &material().key).unwrap()
}

fn verifier(capacity: usize) -> TicketVerifier {
    TicketVerifier::new(ISSUER, SERVER, &material().jwks, capacity).unwrap()
}

#[test]
fn verifies_identity_and_consumes_once() {
    let ticket = token(&claims());
    let mut verifier = verifier(1);
    let identity = verifier.verify_and_consume(&ticket, NOW).unwrap();
    assert_eq!(identity.discord_id(), "123456789012345678");
    assert_eq!(identity.auth_time(), NOW - 100);
    assert_eq!(identity.server_id().to_string(), SERVER);
    assert_eq!(
        verifier.verify_and_consume(&ticket, NOW),
        Err(AuthError::Replay)
    );
}

#[test]
fn rejects_signed_claim_drift_and_missing_claims() {
    for (name, value) in [
        ("iss", json!("https://attacker.test")),
        ("aud", json!("urn:unchained:server:other")),
        ("purpose", json!("access")),
        ("sub", json!("0123456789012345678")),
        ("sub", json!("0")),
        ("sub", json!("18446744073709551616")),
        ("sub", json!(123)),
        ("auth_time", json!(0)),
        ("auth_time", json!(NOW + 1)),
        ("iat", json!(NOW + 31)),
        ("nbf", json!(NOW + 1)),
        ("exp", json!(NOW + 61)),
        ("exp", json!(NOW - 1)),
        ("jti", json!("not-a-uuid")),
    ] {
        let mut body = claims();
        body[name] = value;
        assert_eq!(
            verifier(1).verify_and_consume(&token(&body), NOW),
            Err(AuthError::InvalidTicket),
            "{name}: {}",
            body[name]
        );
    }
    for name in claims().as_object().unwrap().keys() {
        let mut body = claims();
        body.as_object_mut().unwrap().remove(name);
        assert_eq!(
            verifier(1).verify_and_consume(&token(&body), NOW),
            Err(AuthError::InvalidTicket),
            "missing {name}"
        );
    }
}

#[test_case::test_case(NOW, 30, NOW, Ok(()); "short_lifetime")]
#[test_case::test_case(NOW, 60, NOW - 30, Ok(()); "nbf_leeway_edge")]
#[test_case::test_case(NOW, 60, NOW - 31, Err(AuthError::InvalidTicket); "before_leeway")]
#[test_case::test_case(NOW, 60, NOW + 89, Ok(()); "expiry_leeway_edge")]
#[test_case::test_case(NOW, 60, NOW + 90, Err(AuthError::InvalidTicket); "expired")]
#[test_case::test_case(4_000_000_000, 60, 4_000_000_000, Ok(()); "post_2038")]
fn validates_ticket_times(issued: u64, lifetime: u64, now: u64, expected: Result<(), AuthError>) {
    let mut body = claims();
    body["auth_time"] = json!(issued - 100);
    body["iat"] = json!(issued);
    body["nbf"] = json!(issued);
    body["exp"] = json!(issued + lifetime);
    assert_eq!(
        verifier(1)
            .verify_and_consume(&token(&body), now)
            .map(|_| ()),
        expected
    );
}

#[test]
fn rejects_untrusted_headers_and_forged_signatures() {
    for (name, header, expected) in [
        (
            "unknown key",
            Header {
                kid: Some(OTHER_JTI.into()),
                ..header()
            },
            AuthError::UnknownKey,
        ),
        (
            "wrong type",
            Header {
                typ: Some("JWT".into()),
                ..header()
            },
            AuthError::InvalidTicket,
        ),
        (
            "remote keys",
            Header {
                jku: Some("https://attacker.test/keys".into()),
                ..header()
            },
            AuthError::InvalidTicket,
        ),
    ] {
        let ticket = encode(&header, &claims(), &material().key).unwrap();
        assert_eq!(
            verifier(1).verify_and_consume(&ticket, NOW),
            Err(expected),
            "{name}"
        );
    }
    let hmac = Header {
        typ: Some("unchained-join+jwt".into()),
        kid: Some(KID.into()),
        ..Header::new(Algorithm::HS256)
    };
    let mut forged = token(&claims());
    let signature = forged.rfind('.').unwrap() + 1;
    forged.replace_range(
        signature..signature + 1,
        if &forged[signature..signature + 1] == "A" {
            "B"
        } else {
            "A"
        },
    );
    for (name, ticket) in [
        (
            "algorithm substitution",
            encode(&hmac, &claims(), &EncodingKey::from_secret(b"attacker")).unwrap(),
        ),
        ("tampered signature", forged),
    ] {
        assert_eq!(
            verifier(1).verify_and_consume(&ticket, NOW),
            Err(AuthError::InvalidTicket),
            "{name}"
        );
    }
}

#[test]
fn capacity_never_evicts_a_live_replay_entry() {
    let first = token(&claims());
    let mut next = claims();
    next["jti"] = json!(OTHER_JTI);
    let second = token(&next);
    let mut verifier = verifier(1);
    verifier.verify_and_consume(&first, NOW).unwrap();
    assert_eq!(
        verifier.verify_and_consume(&second, NOW),
        Err(AuthError::Capacity)
    );
    assert_eq!(
        verifier.verify_and_consume(&first, NOW),
        Err(AuthError::Replay)
    );
    next["iat"] = json!(NOW + 90);
    next["nbf"] = json!(NOW + 90);
    next["exp"] = json!(NOW + 150);
    assert!(verifier.verify_and_consume(&token(&next), NOW + 90).is_ok());
}

#[test_case::test_case(KID.replace('-', ""); "unhyphenated")]
#[test_case::test_case(format!("{{{KID}}}"); "braced")]
#[test_case::test_case(format!("urn:uuid:{KID}"); "urn")]
#[test_case::test_case("ABCDEF00-0000-4000-8000-000000000002".into(); "uppercase")]
fn rejects_key_id_aliases(alias: String) {
    let mut jwks: Value = serde_json::from_slice(&material().jwks).unwrap();
    jwks["keys"][0]["kid"] = json!(alias);
    let jwks = serde_json::to_vec(&jwks).unwrap();
    assert!(matches!(
        TicketVerifier::new(ISSUER, SERVER, &jwks, 1),
        Err(AuthError::Configuration)
    ));
    let mut verifier = verifier(1);
    assert_eq!(verifier.replace_keys(&jwks), Err(AuthError::Configuration));
    let header = Header {
        kid: Some(alias),
        ..header()
    };
    let ticket = encode(&header, &claims(), &material().key).unwrap();
    assert_eq!(
        verifier.verify_and_consume(&ticket, NOW),
        Err(AuthError::InvalidTicket)
    );
    verifier.verify_and_consume(&token(&claims()), NOW).unwrap();
}

#[test_case::test_case(JTI.replace('-', ""); "unhyphenated")]
#[test_case::test_case(format!("{{{JTI}}}"); "braced")]
#[test_case::test_case(format!("urn:uuid:{JTI}"); "urn")]
#[test_case::test_case("ABCDEF00-0000-4000-8000-000000000003".into(); "uppercase")]
fn rejects_ticket_id_aliases_without_consuming(alias: String) {
    let mut verifier = verifier(1);
    let mut body = claims();
    body["jti"] = json!(alias);
    assert_eq!(
        verifier.verify_and_consume(&token(&body), NOW),
        Err(AuthError::InvalidTicket)
    );
    body["jti"] = json!(uuid::Uuid::parse_str(&alias).unwrap().to_string());
    let ticket = token(&body);
    verifier.verify_and_consume(&ticket, NOW).unwrap();
    assert_eq!(
        verifier.verify_and_consume(&ticket, NOW),
        Err(AuthError::Replay)
    );
}

#[test]
fn key_replacement_preserves_replay_and_failed_replacement_preserves_keys() {
    let ticket = token(&claims());
    let mut verifier = verifier(2);
    verifier.verify_and_consume(&ticket, NOW).unwrap();
    verifier.replace_keys(&material().jwks).unwrap();
    assert_eq!(
        verifier.verify_and_consume(&ticket, NOW),
        Err(AuthError::Replay)
    );
    assert_eq!(verifier.replace_keys(b"{}"), Err(AuthError::Configuration));
    let mut next = claims();
    next["jti"] = json!(OTHER_JTI);
    assert!(verifier.verify_and_consume(&token(&next), NOW).is_ok());
}

#[test]
fn key_replacement_preserves_verifier_policy() {
    let issuer = "https://other-auth.test";
    let mut verifier = TicketVerifier::new(issuer, OTHER_JTI, &material().jwks, 1).unwrap();
    verifier.replace_keys(&material().jwks).unwrap();
    let mut body = claims();
    for (claim, value) in [
        ("iss", json!(issuer)),
        ("aud", json!(format!("urn:unchained:server:{OTHER_JTI}"))),
    ] {
        assert_eq!(
            verifier.verify_and_consume(&token(&body), NOW),
            Err(AuthError::InvalidTicket)
        );
        body[claim] = value;
    }
    let identity = verifier.verify_and_consume(&token(&body), NOW).unwrap();
    assert_eq!(identity.server_id().to_string(), OTHER_JTI);
}

#[test]
fn rejects_invalid_configuration() {
    let keys = material().jwks.as_slice();
    let oversized = vec![b' '; 65537];
    let mut jwks: Value = serde_json::from_slice(keys).unwrap();
    let key = jwks["keys"][0].clone();
    jwks["keys"] = json!([key.clone(), key]);
    let duplicates = serde_json::to_vec(&jwks).unwrap();

    for (name, issuer, server, capacity, jwks) in [
        ("HTTP issuer", "http://auth.test", SERVER, 1, keys),
        (
            "issuer credentials",
            "https://user:pass@auth.test",
            SERVER,
            1,
            keys,
        ),
        ("issuer query", "https://auth.test?query", SERVER, 1, keys),
        (
            "issuer fragment",
            "https://auth.test#fragment",
            SERVER,
            1,
            keys,
        ),
        ("server ID", ISSUER, "invalid", 1, keys),
        ("zero capacity", ISSUER, SERVER, 0, keys),
        ("oversized capacity", ISSUER, SERVER, 65537, keys),
        ("oversized JWKS", ISSUER, SERVER, 1, oversized.as_slice()),
        ("duplicate key", ISSUER, SERVER, 1, duplicates.as_slice()),
    ] {
        assert!(
            matches!(
                TicketVerifier::new(issuer, server, jwks, capacity),
                Err(AuthError::Configuration)
            ),
            "{name}"
        );
    }
}

#[test_case::test_case(String::new(); "empty")]
#[test_case::test_case("a".repeat(4097); "oversized")]
#[test_case::test_case("not.a.ticket".to_owned(); "invalid_encoding")]
fn rejects_malformed_tickets(ticket: String) {
    assert_eq!(
        verifier(1).verify_and_consume(&ticket, NOW),
        Err(AuthError::InvalidTicket)
    );
}
