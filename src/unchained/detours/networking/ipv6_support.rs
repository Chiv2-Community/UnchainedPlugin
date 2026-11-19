//! IPv6 compatibility for native UDP pings.

use crate::chivalry2::engine::online::icmp::{FUDPPing, TFunctionFIcmpEchoResult};
use crate::chivalry2::engine::FString;
use crate::tools::hook_globals::cli_args;

#[unchained_macros::bind_func_detour(FUDPPing::udp_echo, || !cli_args().is_server())]
unsafe fn ipv6_udp_echo(
    endpoint: *mut FString,
    timeout: f32,
    callback: *mut TFunctionFIcmpEchoResult,
) {
    if !endpoint.is_null() {
        let text = unsafe { (*endpoint).to_string() };
        strace!("UDPEcho: received endpoint {text}");
        if let Some(normalized) = bracket_ipv6_udp_endpoint(&text) {
            strace!("UDPEcho: normalizing IPv6 endpoint to {normalized}");
            let mut normalized = FString::from(normalized.as_str());
            return unsafe { FUDPPing::udp_echo(&mut normalized, timeout, callback) };
        }
    }
    unsafe { FUDPPing::udp_echo(endpoint, timeout, callback) };
}

fn bracket_ipv6_udp_endpoint(endpoint: &str) -> Option<String> {
    if endpoint.starts_with('[') || endpoint.matches(':').count() < 2 {
        return None;
    }
    let separator = endpoint.rfind(':')?;
    let host = &endpoint[..separator];
    let port = &endpoint[separator + 1..];
    if host.is_empty() || port.is_empty() || port.parse::<u16>().is_err() {
        return None;
    }
    Some(format!("[{host}]:{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_preserves_the_static_native_signature() {
        let _: unsafe fn(*mut FString, f32, *mut TFunctionFIcmpEchoResult) = ipv6_udp_echo;
    }

    #[test_case::test_case("2001:db8::1:7777", Some("[2001:db8::1]:7777"); "ipv6")]
    #[test_case::test_case("::1:7777", Some("[::1]:7777"); "loopback")]
    #[test_case::test_case("fe80::1%3:7777", Some("[fe80::1%3]:7777"); "scope id")]
    #[test_case::test_case("[::1]:7777", None; "already bracketed")]
    #[test_case::test_case("127.0.0.1:7777", None; "ipv4")]
    #[test_case::test_case("localhost:7777", None; "hostname")]
    #[test_case::test_case("::1:", None; "empty port")]
    #[test_case::test_case("::1:invalid", None; "invalid port")]
    #[test_case::test_case("::1:65536", None; "out of range port")]
    fn ipv6_endpoint_normalization_is_preserved(endpoint: &str, expected: Option<&str>) {
        assert_eq!(bracket_ipv6_udp_endpoint(endpoint).as_deref(), expected);
    }
}
