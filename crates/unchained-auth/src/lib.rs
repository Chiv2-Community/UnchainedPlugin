//! Engine-independent join-ticket verification. No network, OAuth, or game hooks.

use std::collections::HashMap;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use server_browser_models::models::{join_ticket_claims::Purpose, JoinTicketClaims, JwksResponse};
use uuid::Uuid;

const MAX_TICKET_BYTES: usize = 4096;
const MAX_JWKS_BYTES: usize = 64 * 1024;
const MAX_KEYS: usize = 8;
const TICKET_SECONDS: u64 = 60;
const CLOCK_SKEW: u64 = 30;

#[derive(Debug, PartialEq, Eq)]
pub enum AuthError {
    Configuration,
    InvalidTicket,
    UnknownKey,
    Replay,
    Capacity,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AuthError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TicketHeader {
    alg: String,
    typ: String,
    kid: String,
}

/// Produced only after signature, claims, and replay checks succeed.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedJoin {
    discord_id: String,
    auth_time: u64,
    server_id: Uuid,
}

impl VerifiedJoin {
    pub fn discord_id(&self) -> &str {
        &self.discord_id
    }
    pub fn auth_time(&self) -> u64 {
        self.auth_time
    }
    pub fn server_id(&self) -> Uuid {
        self.server_id
    }
}

/// Own one instance per live registration; serialize admission through it.
/// Key refresh is external and must preserve this instance's replay history.
pub struct TicketVerifier {
    issuer: String,
    server_id: Uuid,
    keys: HashMap<String, DecodingKey>,
    consumed: HashMap<String, u64>,
    replay_capacity: usize,
}

fn canonical_uuid(value: &str) -> Option<Uuid> {
    let parsed = Uuid::parse_str(value).ok()?;
    (parsed.to_string() == value).then_some(parsed)
}

fn parse_keys(bytes: &[u8]) -> Result<HashMap<String, DecodingKey>, AuthError> {
    if bytes.len() > MAX_JWKS_BYTES {
        return Err(AuthError::Configuration);
    }
    let document: JwksResponse =
        serde_json::from_slice(bytes).map_err(|_| AuthError::Configuration)?;
    if document.keys.is_empty() || document.keys.len() > MAX_KEYS {
        return Err(AuthError::Configuration);
    }
    let mut keys = HashMap::new();
    for key in document.keys {
        let modulus = URL_SAFE_NO_PAD
            .decode(&key.n)
            .map_err(|_| AuthError::Configuration)?;
        if key.kty != "RSA"
            || key.alg != "RS256"
            || key.r#use != "sig"
            || canonical_uuid(&key.kid).is_none()
            || !(256..=512).contains(&modulus.len())
            || modulus[0] & 0x80 == 0
            || key.e != "AQAB"
        {
            return Err(AuthError::Configuration);
        }
        let decoded = DecodingKey::from_rsa_components(&key.n, &key.e)
            .map_err(|_| AuthError::Configuration)?;
        if keys.insert(key.kid, decoded).is_some() {
            return Err(AuthError::Configuration);
        }
    }
    Ok(keys)
}

impl TicketVerifier {
    pub fn new(
        issuer: &str,
        server_id: &str,
        jwks: &[u8],
        replay_capacity: usize,
    ) -> Result<Self, AuthError> {
        let url = url::Url::parse(issuer).map_err(|_| AuthError::Configuration)?;
        if issuer.len() > 2048
            || issuer.chars().any(char::is_whitespace)
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !(1..=65536).contains(&replay_capacity)
        {
            return Err(AuthError::Configuration);
        }
        Ok(Self {
            issuer: issuer.to_owned(),
            server_id: canonical_uuid(server_id).ok_or(AuthError::Configuration)?,
            keys: parse_keys(jwks)?,
            consumed: HashMap::new(),
            replay_capacity,
        })
    }

    /// Install a bounded, externally trusted JWKS snapshot without resetting replay state.
    pub fn replace_keys(&mut self, jwks: &[u8]) -> Result<(), AuthError> {
        self.keys = parse_keys(jwks)?;
        Ok(())
    }

    /// `now` is trusted Unix time from the host, never a client-supplied value.
    /// Consumption must be coordinated with connection admission by the caller.
    pub fn verify_and_consume(
        &mut self,
        ticket: &str,
        now: u64,
    ) -> Result<VerifiedJoin, AuthError> {
        if ticket.len() > MAX_TICKET_BYTES {
            return Err(AuthError::InvalidTicket);
        }
        let encoded_header = ticket.split('.').next().ok_or(AuthError::InvalidTicket)?;
        let header_bytes = URL_SAFE_NO_PAD
            .decode(encoded_header)
            .map_err(|_| AuthError::InvalidTicket)?;
        let header: TicketHeader =
            serde_json::from_slice(&header_bytes).map_err(|_| AuthError::InvalidTicket)?;
        if header.alg != "RS256"
            || header.typ != "unchained-join+jwt"
            || canonical_uuid(&header.kid).is_none()
        {
            return Err(AuthError::InvalidTicket);
        }
        let key = self.keys.get(&header.kid).ok_or(AuthError::UnknownKey)?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_required_spec_claims(&["iss", "sub", "aud", "exp", "nbf", "iat"]);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[format!("urn:unchained:server:{}", self.server_id)]);
        // Use the caller's sampled time for all time checks, including tests.
        validation.validate_exp = false;
        validation.validate_nbf = false;
        let claims = decode::<JoinTicketClaims>(ticket, key, &validation)
            .map_err(|_| AuthError::InvalidTicket)?
            .claims;
        let iat = u64::try_from(claims.iat).map_err(|_| AuthError::InvalidTicket)?;
        let exp = u64::try_from(claims.exp).map_err(|_| AuthError::InvalidTicket)?;
        let auth_time = u64::try_from(claims.auth_time).map_err(|_| AuthError::InvalidTicket)?;
        let discord_id = claims
            .sub
            .parse::<u64>()
            .map_err(|_| AuthError::InvalidTicket)?;
        if claims.iss != self.issuer
            || claims.aud != format!("urn:unchained:server:{}", self.server_id)
            || discord_id == 0
            || discord_id.to_string() != claims.sub
            || claims.purpose != Purpose::UnchainedJoin
            || canonical_uuid(&claims.jti).is_none()
            || auth_time == 0
            || auth_time > iat
            || claims.nbf != claims.iat
            || !exp
                .checked_sub(iat)
                .is_some_and(|lifetime| (1..=TICKET_SECONDS).contains(&lifetime))
            || iat > now.saturating_add(CLOCK_SKEW)
            || now >= exp.saturating_add(CLOCK_SKEW)
        {
            return Err(AuthError::InvalidTicket);
        }
        self.consumed.retain(|_, until| *until > now);
        if self.consumed.contains_key(&claims.jti) {
            return Err(AuthError::Replay);
        }
        if self.consumed.len() >= self.replay_capacity {
            return Err(AuthError::Capacity);
        }
        self.consumed
            .insert(claims.jti, exp.saturating_add(CLOCK_SKEW));
        Ok(VerifiedJoin {
            discord_id: claims.sub,
            auth_time,
            server_id: self.server_id,
        })
    }
}
