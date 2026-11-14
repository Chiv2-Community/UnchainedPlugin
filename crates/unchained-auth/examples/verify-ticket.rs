//! Local interop harness: credentials arrive on stdin, never in process arguments.
use std::io::{self, Read};

use serde::Deserialize;
use unchained_auth::{AuthError, TicketVerifier};

#[derive(Deserialize)]
struct Input {
    issuer: String,
    server_id: String,
    jwks: serde_json::Value,
    ticket: String,
    now: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    io::stdin().take(100_001).read_to_end(&mut bytes)?;
    if bytes.len() > 100_000 {
        return Err("input limit exceeded".into());
    }
    let input: Input = serde_json::from_slice(&bytes)?;
    let mut verifier = TicketVerifier::new(
        &input.issuer,
        &input.server_id,
        &serde_json::to_vec(&input.jwks)?,
        32,
    )?;
    let identity = verifier.verify_and_consume(&input.ticket, input.now)?;
    assert_eq!(
        verifier.verify_and_consume(&input.ticket, input.now),
        Err(AuthError::Replay)
    );
    println!(
        "verified Discord identity {}; replay rejected",
        identity.discord_id()
    );
    Ok(())
}
