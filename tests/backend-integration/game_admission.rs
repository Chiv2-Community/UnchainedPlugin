#[path = "../../src/features/game_admission.rs"]
mod admission;

use admission::HostAdmissions;
use server_browser_client_sdk::admission::AdmissionError;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const ISSUER: &str = "https://browser.example";
const SERVER: &str = "18e532b1-6733-4b5a-9ffb-1cd7838bc7e4";

#[test]
fn pause_expiry_and_renewal_preserve_the_same_replay_cache() {
    let admissions = HostAdmissions::default();
    assert!(admissions.verifier_for_refresh().is_none());
    assert_eq!(
        admissions.consume("bad"),
        Err(AdmissionError::KeysUnavailable)
    );
    admissions
        .publish(
            ISSUER,
            Some(SERVER),
            Some(Instant::now() + Duration::from_secs(60)),
        )
        .unwrap();
    let original = admissions.verifier_for_refresh().unwrap();
    assert_eq!(admissions.consume("bad"), Err(AdmissionError::Rejected));
    admissions.pause();
    assert_eq!(
        admissions.consume("bad"),
        Err(AdmissionError::KeysUnavailable)
    );
    admissions.publish(ISSUER, Some(SERVER), None).unwrap();
    assert!(Arc::ptr_eq(
        &original,
        &admissions.verifier_for_refresh().unwrap()
    ));
    admissions
        .publish(ISSUER, Some(SERVER), Some(Instant::now()))
        .unwrap();
    assert_eq!(
        admissions.consume("bad"),
        Err(AdmissionError::KeysUnavailable)
    );
    admissions
        .publish(
            ISSUER,
            Some(SERVER),
            Some(Instant::now() + Duration::from_secs(60)),
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &original,
        &admissions.verifier_for_refresh().unwrap()
    ));
    assert_eq!(admissions.consume("bad"), Err(AdmissionError::Rejected));
}

#[test]
fn a_new_registration_gets_a_separate_verifier_and_old_refreshes_cannot_replace_it() {
    let admissions = HostAdmissions::default();
    admissions.publish(ISSUER, Some(SERVER), None).unwrap();
    let old = admissions.verifier_for_refresh().unwrap();
    admissions
        .publish(ISSUER, Some("18e532b1-6733-4b5a-9ffb-1cd7838bc7e5"), None)
        .unwrap();
    let new = admissions.verifier_for_refresh().unwrap();
    assert!(!Arc::ptr_eq(&old, &new));
    old.replace_keys(server_browser_client::models::JwksResponse::new(vec![]))
        .unwrap();
    assert!(Arc::ptr_eq(
        &new,
        &admissions.verifier_for_refresh().unwrap()
    ));
    admissions.publish(ISSUER, None, None).unwrap();
    assert!(admissions.verifier_for_refresh().is_none());
    assert_eq!(
        admissions.consume("bad"),
        Err(AdmissionError::KeysUnavailable)
    );
}
