//! Public account-control proof. Player sessions go only to the fixed PlayFab host.
//! Upstream: PlayFab/API_Specs@80e104aa, Client/GetAccountInfo/GetUserData/UpdateUserData.
use super::game_join_protocol as protocol;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use server_browser_client::{
    apis::api_v1_api,
    models::{GameAdmissionRequest, GameJoinChallengeRequest, GameJoinTicketRequest},
};
use sha2::{Digest, Sha256};
use std::{net::SocketAddr, time::Duration};

struct PlayFab {
    client: reqwest::Client,
    session: String,
}

impl PlayFab {
    async fn call(&self, operation: &str, body: Value) -> Option<Value> {
        let mut response = self
            .client
            .post(format!("https://EBF8D.playfabapi.com/Client/{operation}"))
            .header("X-Authorization", &self.session)
            .json(&body)
            .send()
            .await
            .ok()?;
        if response.status() != reqwest::StatusCode::OK {
            return None;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len() + chunk.len() > 65536 {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        let body: Value = serde_json::from_slice(&bytes).ok()?;
        (body.get("code").and_then(Value::as_u64) == Some(200)).then_some(body)
    }

    async fn data(&self) -> Option<Value> {
        let response = self
            .call("GetUserData", json!({"Keys":[protocol::PROOF_KEY]}))
            .await?;
        response
            .pointer("/data/Data")
            .filter(|data| data.is_object())
            .cloned()
    }
}

/// All calls are individually bounded. Do not cancel the future after publication:
/// cleanup must run even when verification or ticket issuance fails.
pub async fn issue(
    backend: &str,
    target: SocketAddr,
    claimed: &str,
    session: String,
) -> Option<String> {
    issue_proof(backend, target, claimed, session, None).await
}

/// Signed admissions remain separate from legacy issuance; rejection must never
/// retry through the legacy route.
pub async fn issue_admission(
    backend: &str,
    target: SocketAddr,
    claimed: &str,
    session: String,
    eos_id_token: String,
) -> Option<String> {
    issue_proof(backend, target, claimed, session, Some(eos_id_token)).await
}

async fn issue_proof(
    backend: &str,
    target: SocketAddr,
    claimed: &str,
    session: String,
    eos_id_token: Option<String>,
) -> Option<String> {
    let playfab = PlayFab {
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .ok()?,
        session,
    };
    let account = playfab.call("GetAccountInfo", json!({})).await?;
    let id = account.pointer("/data/AccountInfo/PlayFabId")?.as_str()?;
    if !id.eq_ignore_ascii_case(claimed) {
        return None;
    }
    let id = id.to_ascii_uppercase();
    let existing = playfab.data().await?;
    if let Some(record) = existing.get(protocol::PROOF_KEY) {
        // The opted-in protocol owns this one reserved key. Never replace
        // arbitrary pre-existing user data, even if its key happens to collide.
        let value = record.get("Value")?.as_str()?;
        if !value
            .strip_prefix("uc-proof-v1:")
            .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return None;
        }
    }
    let secret = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    let commitment = format!("{:x}", Sha256::digest(secret.as_bytes()));
    let transport = crate::backend::transport(backend, None).ok()?;
    let challenge = tokio::time::timeout(
        Duration::from_secs(7),
        api_v1_api::begin_game_join_challenge(
            transport.configuration(),
            GameJoinChallengeRequest::new(
                id.clone(),
                commitment.clone(),
                backend.to_owned(),
                target.ip().to_string(),
                i32::from(target.port()),
            ),
        ),
    )
    .await
    .ok()?
    .ok()?;
    let proof = protocol::proof_value(
        backend,
        &challenge.server_id,
        &id,
        &challenge.challenge_id,
        &commitment,
    )?;
    // Publication failure may be an ambiguous network failure. Cleanup still runs.
    let published = playfab
        .call(
            "UpdateUserData",
            json!({"Data":{(protocol::PROOF_KEY):proof},"Permission":"Public"}),
        )
        .await
        .is_some();
    let ticket = if published {
        if let Some(eos_id_token) = eos_id_token {
            tokio::time::timeout(
                Duration::from_secs(7),
                api_v1_api::issue_game_admission(
                    transport.configuration(),
                    GameAdmissionRequest::new(challenge.challenge_id, secret, eos_id_token),
                ),
            )
            .await
            .ok()
            .map(|result| {
                if let Err(server_browser_client::apis::Error::ResponseError(response)) = &result {
                    if response.status == reqwest::StatusCode::CONFLICT
                        && matches!(response.entity.as_ref(),
                            Some(api_v1_api::IssueGameAdmissionError::Status409(status))
                                if status.status == "account_mismatch")
                    {
                        // Never echo an arbitrary backend body or its identifiers.
                        swarn!("Account Mismatch: verified accounts conflict with the backend pin; contact support");
                    }
                }
                result
            })
            .and_then(Result::ok)
            .filter(|ticket| ticket.server_id == challenge.server_id)
            .filter(|ticket| protocol::admission_ticket(&ticket.ticket))
            .map(|ticket| ticket.ticket)
        } else {
            tokio::time::timeout(
                Duration::from_secs(7),
                api_v1_api::issue_game_join_ticket(
                    transport.configuration(),
                    GameJoinTicketRequest::new(challenge.challenge_id, secret),
                ),
            )
            .await
            .ok()
            .and_then(Result::ok)
            .filter(|ticket| ticket.server_id == challenge.server_id)
            .and_then(|ticket| protocol::encode_ticket(&ticket.server_id, &ticket.ticket))
        }
    } else {
        None
    };
    // Avoid removing a different concurrent proof. PlayFab has no CAS here:
    // concurrent joins can fail closed, but cannot authenticate the wrong account.
    let own_value = playfab.data().await.and_then(|data| {
        data.get(protocol::PROOF_KEY)?
            .get("Value")?
            .as_str()
            .map(str::to_owned)
    });
    if own_value.as_deref() == Some(proof.as_str()) {
        if playfab
            .call(
                "UpdateUserData",
                json!({"KeysToRemove":[protocol::PROOF_KEY]}),
            )
            .await
            .is_none()
        {
            swarn!("PlayFab join-proof cleanup unavailable; reserved public commitment may remain");
        }
    }
    let cleaned = playfab
        .data()
        .await
        .is_some_and(|data| data.get(protocol::PROOF_KEY).is_none());
    if !cleaned {
        swarn!("PlayFab join-proof cleanup could not be confirmed; a public commitment may remain");
    }
    sinfo!(
        "PlayFab account-control proof completed: grant={} cleanup_verified={}",
        ticket.is_some(),
        cleaned
    );
    ticket
}
