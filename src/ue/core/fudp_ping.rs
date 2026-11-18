use crate::{
    tools::hook_globals::cli_args,
    ue::{FString, TFunctionFIcmpEchoResult},
};

define_pattern_resolver!(
    UDPEcho,
    ["48 89 5C 24 20 55 56 57 48 81 EC 90 00 00 00 49 8B D8 0F 29 B4 24 80 00 00 00 48 8B E9"]
);

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

#[unchained_macros::bind_resolved_func_detour(UDPEcho, || !cli_args().is_server())]
unsafe fn udp_echo_detour(
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
            return unsafe { udp_echo(&mut normalized, timeout, callback) };
        }
    }
    unsafe {
        udp_echo(endpoint, timeout, callback);
    }
}
