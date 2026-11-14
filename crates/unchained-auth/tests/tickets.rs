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

fn material() -> &'static (EncodingKey, Vec<u8>) {
    static MATERIAL: OnceLock<(EncodingKey, Vec<u8>)> = OnceLock::new();
    MATERIAL.get_or_init(|| {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let pem = private.to_pkcs8_pem(Default::default()).unwrap();
        let jwks = json!({"keys": [{
            "kty": "RSA", "alg": "RS256", "use": "sig", "kid": KID,
            "n": URL_SAFE_NO_PAD.encode(private.n().to_bytes_be()), "e": "AQAB"
        }]});
        (
            EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            serde_json::to_vec(&jwks).unwrap(),
        )
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
    encode(&header(), claims, &material().0).unwrap()
}

fn verifier(capacity: usize) -> TicketVerifier {
    TicketVerifier::new(ISSUER, SERVER, &material().1, capacity).unwrap()
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
            "{name}"
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

#[test]
fn enforces_time_boundaries() {
    let mut shorter = claims();
    shorter["exp"] = json!(NOW + 30);
    assert!(verifier(1)
        .verify_and_consume(&token(&shorter), NOW)
        .is_ok());
    let ticket = token(&claims());
    assert!(verifier(1).verify_and_consume(&ticket, NOW - 30).is_ok());
    assert_eq!(
        verifier(1).verify_and_consume(&ticket, NOW - 31),
        Err(AuthError::InvalidTicket)
    );
    assert!(verifier(1).verify_and_consume(&ticket, NOW + 89).is_ok());
    assert_eq!(
        verifier(1).verify_and_consume(&ticket, NOW + 90),
        Err(AuthError::InvalidTicket)
    );
}

#[test]
fn generated_claims_support_timestamps_after_2038() {
    let now = 4_000_000_000_u64;
    let mut body = claims();
    body["auth_time"] = json!(now - 100);
    body["iat"] = json!(now);
    body["nbf"] = json!(now);
    body["exp"] = json!(now + 60);
    assert!(verifier(1).verify_and_consume(&token(&body), now).is_ok());
}

#[test]
fn rejects_untrusted_headers_and_forged_signatures() {
    let mut wrong = header();
    wrong.kid = Some(OTHER_JTI.into());
    let unknown = encode(&wrong, &claims(), &material().0).unwrap();
    assert_eq!(
        verifier(1).verify_and_consume(&unknown, NOW),
        Err(AuthError::UnknownKey)
    );
    wrong = header();
    wrong.typ = Some("JWT".into());
    assert_eq!(
        verifier(1).verify_and_consume(&encode(&wrong, &claims(), &material().0).unwrap(), NOW),
        Err(AuthError::InvalidTicket)
    );
    wrong = header();
    wrong.jku = Some("https://attacker.test/keys".into());
    assert_eq!(
        verifier(1).verify_and_consume(&encode(&wrong, &claims(), &material().0).unwrap(), NOW),
        Err(AuthError::InvalidTicket)
    );
    let mut hmac = Header::new(Algorithm::HS256);
    hmac.typ = Some("unchained-join+jwt".into());
    hmac.kid = Some(KID.into());
    let forged = encode(&hmac, &claims(), &EncodingKey::from_secret(b"attacker")).unwrap();
    assert_eq!(
        verifier(1).verify_and_consume(&forged, NOW),
        Err(AuthError::InvalidTicket)
    );
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
    assert_eq!(
        verifier(1).verify_and_consume(&forged, NOW),
        Err(AuthError::InvalidTicket)
    );
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

#[test]
fn key_replacement_preserves_replay_and_failed_replacement_preserves_keys() {
    let ticket = token(&claims());
    let mut verifier = verifier(2);
    verifier.verify_and_consume(&ticket, NOW).unwrap();
    verifier.replace_keys(&material().1).unwrap();
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
fn bounds_untrusted_inputs_and_configuration() {
    for issuer in [
        "http://auth.test",
        "https://user:pass@auth.test",
        "https://auth.test?query",
        "https://auth.test#fragment",
    ] {
        assert!(TicketVerifier::new(issuer, SERVER, &material().1, 1).is_err());
    }
    assert!(TicketVerifier::new(ISSUER, "invalid", &material().1, 1).is_err());
    for capacity in [0, 65537] {
        assert!(TicketVerifier::new(ISSUER, SERVER, &material().1, capacity).is_err());
    }
    assert!(TicketVerifier::new(ISSUER, SERVER, &vec![b' '; 65537], 1).is_err());
    for ticket in ["".to_owned(), "a".repeat(4097), "not.a.ticket".to_owned()] {
        assert_eq!(
            verifier(1).verify_and_consume(&ticket, NOW),
            Err(AuthError::InvalidTicket)
        );
    }
    let mut jwks: Value = serde_json::from_slice(&material().1).unwrap();
    let key = jwks["keys"][0].clone();
    jwks["keys"] = json!([key.clone(), key]);
    assert!(TicketVerifier::new(ISSUER, SERVER, &serde_json::to_vec(&jwks).unwrap(), 1).is_err());
}
