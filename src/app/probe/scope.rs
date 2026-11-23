//! Inspection scope guard (brief §8): refuse to actively probe private,
//! loopback or link-local infrastructure unless the operator explicitly
//! enables internal mode.
//!
//! The guard evaluates literals at the application boundary: IP address
//! strings and localhost-style names discovered by external tools. It does
//! not (yet) resolve hostnames before probing — a public name that resolves
//! to a private address is caught by the scanner's own connection, not by
//! this gate. That follow-up is tracked in `.specs/09`, not claimed here.

use std::net::IpAddr;
use std::str::FromStr;

/// Why a target was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeDenial {
    PrivateAddress(String),
    Loopback(String),
    LinkLocal(String),
    LocalhostName(String),
}

impl std::fmt::Display for ScopeDenial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeDenial::PrivateAddress(ip) => {
                write!(f, "private address {ip} (use --allow-internal)")
            }
            ScopeDenial::Loopback(ip) => write!(f, "loopback address {ip}"),
            ScopeDenial::LinkLocal(ip) => write!(f, "link-local address {ip}"),
            ScopeDenial::LocalhostName(name) => {
                write!(f, "localhost-style name {name} (use --allow-internal)")
            }
        }
    }
}

/// Local suffixes that unambiguously denote internal infrastructure.
const LOCAL_SUFFIXES: [&str; 4] = [".local", ".internal", ".lan", ".localdomain"];

/// Decide whether actively probing `target` is within the authorised
/// scope. `allow_internal` is the operator's explicit opt-in (`--allow-internal`
/// or `BALEBOS_ALLOW_INTERNAL=1`).
pub fn check_scope(target: &str, allow_internal: bool) -> Result<(), ScopeDenial> {
    if allow_internal {
        return Ok(());
    }

    let trimmed = target.trim_end_matches('.');
    let without_scheme = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);
    let without_path = without_scheme.split('/').next().unwrap_or(without_scheme);

    // Host:port only when a single colon appears (IPv4/hostname). IPv6
    // literals use brackets ([::1]) or stand bare (multiple colons).
    let host = if let Some(rest) = without_path.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else if without_path.matches(':').count() == 1 {
        without_path.split(':').next().unwrap_or(without_path)
    } else {
        without_path
    };

    if let Ok(ip) = IpAddr::from_str(host) {
        if ip.is_loopback() {
            return Err(ScopeDenial::Loopback(ip.to_string()));
        }
        if is_link_local(&ip) {
            return Err(ScopeDenial::LinkLocal(ip.to_string()));
        }
        if is_private_ipv4(&ip) || is_private_ipv6(&ip) {
            return Err(ScopeDenial::PrivateAddress(ip.to_string()));
        }
        return Ok(());
    }

    let lower = host.to_lowercase();
    if lower == "localhost" || lower.starts_with("localhost.") {
        return Err(ScopeDenial::LocalhostName(host.to_string()));
    }
    if LOCAL_SUFFIXES.iter().any(|suffix| lower.ends_with(suffix)) {
        return Err(ScopeDenial::LocalhostName(host.to_string()));
    }
    Ok(())
}

fn is_private_ipv4(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_documentation() || v4.octets()[0] == 0,
        _ => false,
    }
}

/// 169.254.0.0/16 (IPv4) and fe80::/10 (IPv6).
fn is_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.octets()[0] == 169 && v4.octets()[1] == 254,
        IpAddr::V6(v6) => v6.segments()[0] & 0xffc0 == 0xfe80,
    }
}

fn is_private_ipv6(ip: &IpAddr) -> bool {
    matches!(
        ip,
        IpAddr::V6(v6) if v6.is_unique_local() || v6.segments()[0] & 0xfe00 == 0xfc00
    )
}

/// True when the operator enabled internal probing via the environment.
pub fn internal_allowed_from_env() -> bool {
    std::env::var("BALEBOS_ALLOW_INTERNAL")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_targets_are_allowed() {
        for target in [
            "example.com",
            "https://example.com/path",
            "api.example.co.uk",
            "8.8.8.8",
            "2606:4700:4700::1111",
            "http://93.184.216.34:8080/x",
        ] {
            assert!(check_scope(target, false).is_ok(), "denied {target}");
        }
    }

    #[test]
    fn internal_targets_are_denied_by_default() {
        for target in [
            "10.0.0.5",
            "192.168.1.1",
            "172.16.0.1",
            "127.0.0.1",
            "::1",
            "fe80::1",
            "169.254.1.1",
            "fd00::1",
            "localhost",
            "db.internal",
            "printer.lan",
            "box.localdomain.",
        ] {
            assert!(
                check_scope(target, false).is_err(),
                "allowed internal target {target}"
            );
        }
    }

    #[test]
    fn allow_internal_overrides() {
        assert!(check_scope("10.0.0.5", true).is_ok());
        assert!(check_scope("localhost", true).is_ok());
    }

    #[test]
    fn urls_and_ports_are_parsed_before_checking() {
        assert!(check_scope("http://192.168.0.1:8080/admin", false).is_err());
        assert!(check_scope("https://example.com:443/", false).is_ok());
    }
}
