use std::net::Ipv6Addr;

pub fn is_ipv6_host(host: &str) -> bool {
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host.parse::<Ipv6Addr>().is_ok()
}

/// Bare endpoints retain the PR's legacy convention: the final colon is the port separator.
pub fn split_ipv6_endpoint(endpoint: &str) -> Option<(&str, &str)> {
    let (host, port) = if let Some(bracketed) = endpoint.strip_prefix('[') {
        bracketed.split_once("]:")?
    } else {
        endpoint.rsplit_once(':')?
    };
    if host.parse::<Ipv6Addr>().is_err()
        || port.is_empty()
        || !port.bytes().all(|byte| byte.is_ascii_digit())
        || port.parse::<u16>().is_err()
    {
        return None;
    }
    Some((host, port))
}

pub fn ipv6_port_separator(endpoint: &str) -> Option<i32> {
    let (_, port) = split_ipv6_endpoint(endpoint)?;
    // Valid IPv6 endpoints are ASCII, so byte and UTF-16 indices agree.
    i32::try_from(endpoint.len() - port.len() - 1).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_complete_ipv6_hosts() {
        for host in ["::1", "2001:db8::1", "[2001:db8::1]", "::ffff:192.0.2.1"] {
            assert!(is_ipv6_host(host), "{host}");
        }
        for host in [
            "",
            "127.0.0.1",
            "example.org",
            "[invalid]",
            "[::1]:7071",
            "[::1",
            "::1]",
        ] {
            assert!(!is_ipv6_host(host), "{host}");
        }
    }

    #[test]
    fn splits_bracketed_and_legacy_bare_endpoints() {
        for endpoint in ["[2001:db8::1]:7071", "2001:db8::1:7071"] {
            assert_eq!(split_ipv6_endpoint(endpoint), Some(("2001:db8::1", "7071")));
            assert_eq!(
                ipv6_port_separator(endpoint),
                Some(endpoint.rfind(':').unwrap() as i32)
            );
        }
        assert_eq!(split_ipv6_endpoint("[::1]:0"), Some(("::1", "0")));
        assert_eq!(
            split_ipv6_endpoint("[::ffff:192.0.2.1]:65535"),
            Some(("::ffff:192.0.2.1", "65535"))
        );
    }

    #[test]
    fn leaves_ipv4_dns_and_malformed_input_to_the_original() {
        for endpoint in [
            "127.0.0.1:7071",
            "example.org:7071",
            "[invalid]:7071",
            "[::1]",
            "[::1]:",
            "[::1]:65536",
            "[::1]:-1",
            "[::1]:+1",
            "[::1]: 1",
            "[::1]:1:2",
            "[::1:7071",
            "",
            "::1",
        ] {
            assert_eq!(split_ipv6_endpoint(endpoint), None, "{endpoint}");
        }
    }

    #[test]
    fn preserves_full_host_keys_and_independent_ports() {
        let a = "[2001:db8::1]:7071";
        let b = "[2001:db8::2]:7071";
        let a_key = &a[..ipv6_port_separator(a).unwrap() as usize];
        let b_key = &b[..ipv6_port_separator(b).unwrap() as usize];
        assert_eq!(a_key, "[2001:db8::1]");
        assert_ne!(a_key, b_key);
        assert_eq!(split_ipv6_endpoint("[2001:db8::1]:7777").unwrap().1, "7777");
    }
}
