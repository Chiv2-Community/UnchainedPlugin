//! UE 4.25-plus: [FUDPPing](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Online/ICMP/Public/Icmp.h).

use crate::tools::hook_globals::cli_args;
use crate::chivalry2::engine::FString;
use std::ffi::c_void;

define_pattern_resolver!(
    UDPEcho,
    ["48 89 5C 24 20 55 56 57 48 81 EC 90 00 00 00 49 8B D8 0F 29 B4 24 80 00 00 00 48 8B E9"]
);

/// Static native ping helpers; no instance layout is assumed.
pub struct FUDPPing {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl FUDPPing {
    #[bind_resolved_func_detour(UDPEcho, || !cli_args().is_server())]
    unsafe fn udp_echo_detour(
        endpoint: *mut FString,
        timeout: f32,
        callback: *mut TFunctionFIcmpEchoResult,
    ) {
        if !endpoint.is_null() {
            let text = unsafe { (*endpoint).to_string() };
            strace!("UDPEcho: received endpoint {text}");
            if let Some(normalized) = Self::bracket_ipv6_udp_endpoint(&text) {
                strace!("UDPEcho: normalizing IPv6 endpoint to {normalized}");
                let mut normalized = FString::from(normalized.as_str());
                return unsafe { Self::udp_echo(&mut normalized, timeout, callback) };
            }
        }
        unsafe {
            Self::udp_echo(endpoint, timeout, callback);
        }
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
}

/// Opaque `TFunction<void(FIcmpEchoResult)>`; passed only through native pointers.
pub type TFunctionFIcmpEchoResult = c_void;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detour_and_original_remain_static_native_calls() {
        type UdpEcho = unsafe fn(*mut FString, f32, *mut TFunctionFIcmpEchoResult);
        let _: UdpEcho = FUDPPing::udp_echo_detour;
        let _: UdpEcho = FUDPPing::udp_echo;
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
        assert_eq!(FUDPPing::bracket_ipv6_udp_endpoint(endpoint).as_deref(), expected);
    }
}
