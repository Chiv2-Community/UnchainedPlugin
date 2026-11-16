//! Credential-free parsing shared by native adapters and host tests.
use std::net::SocketAddr;

/// Reserved client-writable PlayFab key; never supplied by a backend response.
pub const PROOF_KEY: &str = "UnchainedJoin_v1";

/// Legacy opaque-grant envelope; never a fallback after JWT rejection.
pub const PREFIX: &str = "ucj1.";

pub fn encode_ticket(server: &str, ticket: &str) -> Option<String> {
    (valid_server(server) && valid_ticket(ticket)).then(|| format!("{PREFIX}{server}.{ticket}"))
}

pub fn decode_ticket(value: &str) -> Option<(&str, &str)> {
    let (server, ticket) = value.strip_prefix(PREFIX)?.split_once('.')?;
    (valid_server(server) && valid_ticket(ticket)).then_some((server, ticket))
}

pub fn proof_value(
    audience: &str,
    server: &str,
    playfab: &str,
    challenge: &str,
    commitment: &str,
) -> Option<String> {
    use sha2::{Digest, Sha256};
    if trusted_backend(audience).as_deref() != Some(audience)
        || !valid_server(server)
        || !valid_ticket(challenge)
        || playfab.is_empty()
        || playfab.len() > MAX_PLAYFAB_ID_LENGTH
        || !playfab.bytes().all(|b| b.is_ascii_hexdigit())
        || commitment.len() != 64
        || !commitment
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let playfab = playfab.to_ascii_uppercase();
    let material = [
        "unchained-playfab-proof-v1",
        audience,
        server,
        &playfab,
        challenge,
        commitment,
    ]
    .join("\0");
    Some(format!(
        "uc-proof-v1:{:x}",
        Sha256::digest(material.as_bytes())
    ))
}

pub fn claimed_account(url: &str) -> Option<&str> {
    target(url)?;
    url.split('?')
        .skip(1)
        .find_map(|part| part.strip_prefix("PlayFabId="))
}

// Local parsing budget, not a discovered Unreal URL limit.
const MAX_JOIN_OPTIONS_BYTES: usize = 16 * 1024;
// Matches the backend GameJoinRedemptionRequest contract's hexadecimal ID bound.
const MAX_PLAYFAB_ID_LENGTH: usize = 64;

pub fn trusted_backend(browser: &str) -> Option<String> {
    let identity = reqwest::Url::parse(browser).ok()?;
    if !identity.username().is_empty()
        || identity.password().is_some()
        || identity.query().is_some()
        || identity.fragment().is_some()
    {
        return None;
    }
    let secure = identity.scheme() == "https";
    let loopback = identity
        .host_str()
        .and_then(|host| {
            host.trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .ok()
        })
        .is_some_and(|ip| ip.is_loopback());
    (secure || (identity.scheme() == "http" && loopback))
        .then(|| identity.as_str().trim_end_matches('/').to_owned())
}

pub fn target(url: &str) -> Option<SocketAddr> {
    if url.len() > MAX_JOIN_OPTIONS_BYTES {
        return None;
    }
    let (address, options) = url.split_once('?')?;
    let mut ids = options
        .split('?')
        .filter_map(|option| option.strip_prefix("PlayFabId="));
    let id = ids.next()?;
    if id.is_empty()
        || id.len() > MAX_PLAYFAB_ID_LENGTH
        || !id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || ids.next().is_some()
    {
        return None;
    }
    let address: SocketAddr = address.parse().ok()?;
    (address.port() != 0).then_some(address)
}

/// Structural URL-budget check only. Authentication belongs to the host's JWT
/// verifier; neither a valid shape nor decoded claims establish identity.
pub fn admission_ticket(value: &str) -> bool {
    if value.is_empty() || value.len() > 16384 {
        return false;
    }
    let mut parts = value.split('.');
    (0..3).all(|_| {
        parts.next().is_some_and(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        })
    }) && parts.next().is_none()
}

fn valid_server(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

fn valid_ticket(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

pub fn replace_ticket(url: &str, ticket: &str) -> String {
    let mut parts = url.split('?');
    let mut result = parts.next().unwrap_or_default().to_owned();
    for option in parts.filter(|option| {
        !option
            .split('=')
            .next()
            .unwrap_or_default()
            .eq_ignore_ascii_case("UnofficialTicket")
    }) {
        result.push('?');
        result.push_str(option);
    }
    result.push_str("?UnofficialTicket=");
    result.push_str(ticket);
    result
}

pub fn admission_options(options: &str) -> bool {
    if options.len() > MAX_JOIN_OPTIONS_BYTES {
        return false;
    }
    let single = |name: &str| {
        let mut values = options.split('?').filter_map(|option| {
            let (key, value) = option.split_once('=')?;
            key.eq_ignore_ascii_case(name).then_some(value)
        });
        let value = values.next()?;
        values.next().is_none().then_some(value)
    };
    single("PlayFabId").is_some_and(|id| {
        !id.is_empty()
            && id.len() <= MAX_PLAYFAB_ID_LENGTH
            && id.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) && single("UnofficialTicket")
        .is_some_and(|ticket| admission_ticket(ticket) || decode_ticket(ticket).is_some())
}
