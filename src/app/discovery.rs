//! Autonomous bounded discovery pipeline (`balebos discover run <domain>`).
//!
//! Closes the loop from a known domain to persisted CT candidates, DNS SRV/TXT
//! references, public JavaScript asset references, and OpenAPI specifications
//! without requiring manual input files or paid API keys.

use std::collections::BTreeSet;
use std::net::{IpAddr, ToSocketAddrs};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use regex::Regex;

use crate::app::probe::process;
use crate::app::probe::scope::check_scope;
use crate::app::references::{
    extract_dns_srv_txt_references, extract_js_references, parse_openapi_references,
};
use crate::app::sources::{parse_ct_json, EnrichmentCandidateRecord};
use crate::domain::normalize::normalize_fqdn;
use crate::domain::service::ApplicationReferenceObservation;
use crate::error::Result;
use crate::inventory::ItemType;
use crate::storage::{StorageEngine, StorageEngineExt};

const MAX_JS_ASSETS_PER_DOMAIN: usize = 5;
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_FETCH_BYTES: usize = 512 * 1024;

/// Summary of an autonomous discovery run.
#[derive(Debug, Clone, Default)]
pub struct AutonomousDiscoverySummary {
    pub domain: String,
    pub ct_candidates: usize,
    pub ct_entities: Vec<String>,
    pub dns_references: usize,
    pub js_assets_fetched: usize,
    pub js_references: usize,
    pub openapi_references: usize,
    pub errors: Vec<String>,
}

/// Extract same-scope `<script src="...">` URLs from public HTML (bounded).
pub fn extract_script_srcs_from_html(base_url: &str, html: &str) -> Vec<String> {
    let script_re =
        Regex::new(r#"(?i)<script[^>]+src=["']([^"']+)["']"#).expect("valid script src regex");
    let base_trimmed = base_url.trim_end_matches('/');
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for cap in script_re.captures_iter(html) {
        if out.len() >= MAX_JS_ASSETS_PER_DOMAIN {
            break;
        }
        let raw = cap[1].trim();
        let resolved = if raw.starts_with("https://") || raw.starts_with("http://") {
            raw.to_string()
        } else if raw.starts_with("//") {
            format!("https:{}", raw)
        } else if raw.starts_with('/') {
            format!("{}{}", base_trimmed, raw)
        } else {
            format!("{}/{}", base_trimmed, raw)
        };
        if seen.insert(resolved.clone()) {
            out.push(resolved);
        }
    }
    out
}

/// Verify that a script or spec URL resolves to an authorised, public-scope host
/// using strict authority/label-boundary matching (never substring `.contains()`).
pub fn is_url_in_scope(raw_url: &str, approved_scope: &[String]) -> bool {
    let lower = raw_url.trim().to_ascii_lowercase();
    let Some(after_scheme) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    else {
        return false;
    };
    let Some(authority) = after_scheme.split(['/', '?', '#']).next() else {
        return false;
    };
    let host_raw = authority
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    let host = normalize_fqdn(host_raw);
    if host.is_empty() || check_scope(&host, false).is_err() {
        return false;
    }
    approved_scope.iter().any(|s| {
        let sc = normalize_fqdn(s);
        !sc.is_empty() && (host == sc || host.ends_with(&format!(".{}", sc)))
    })
}

/// Validate all DNS-resolved IP addresses for a host before connecting,
/// preventing DNS rebinding / SSRF to private, loopback, link-local,
/// unspecified, or multicast addresses.
pub fn validate_resolved_ips(host: &str, ips: &[IpAddr]) -> std::result::Result<IpAddr, String> {
    if ips.is_empty() {
        return Err(format!("DNS resolution returned no addresses for {}", host));
    }
    for ip in ips {
        if ip.is_unspecified() || ip.is_multicast() {
            return Err(format!(
                "refusing connection to {} ({} is unspecified/multicast)",
                host, ip
            ));
        }
        if let Err(denial) = check_scope(&ip.to_string(), false) {
            return Err(format!(
                "refusing connection to {} (resolved IP {} denied: {})",
                host, ip, denial
            ));
        }
    }
    Ok(ips[0])
}

fn extract_host_and_port(url: &str) -> std::result::Result<(String, u16), String> {
    let lower = url.trim().to_ascii_lowercase();
    let (default_port, after_scheme) = if let Some(rest) = lower.strip_prefix("https://") {
        (443u16, rest)
    } else if let Some(rest) = lower.strip_prefix("http://") {
        (80u16, rest)
    } else {
        return Err(format!("unsupported URL scheme: {}", url));
    };
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("");
    if let Some((h, p)) = authority.rsplit_once(':') {
        let port = p
            .parse::<u16>()
            .map_err(|_| format!("invalid port in URL: {}", url))?;
        Ok((normalize_fqdn(h), port))
    } else {
        Ok((normalize_fqdn(authority), default_port))
    }
}

/// Construct a `curl` command with pinned IP resolution, zero redirects,
/// and all proxy environment variables disabled so SSRF checks cannot be
/// bypassed via `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY`.
pub fn build_pinned_curl_command(url: &str, resolve_arg: &str) -> Command {
    let mut cmd = Command::new("curl");
    cmd.args([
        "-sS",
        "--max-time",
        "10",
        "--max-filesize",
        "524288",
        "--max-redirs",
        "0",
        "--noproxy",
        "*",
        "--resolve",
        resolve_arg,
        url,
    ]);
    for var in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        cmd.env_remove(var);
    }
    cmd
}

fn fetch_bounded_url(url: &str) -> std::result::Result<String, String> {
    let (host, port) = extract_host_and_port(url)?;
    let addrs: Vec<IpAddr> = format!("{}:{}", host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for {}: {}", host, e))?
        .map(|sa| sa.ip())
        .collect();
    let pinned_ip = validate_resolved_ips(&host, &addrs)?;
    let resolve_arg = format!("{}:{}:{}", host, port, pinned_ip);

    let out = process::run_bounded(
        &mut build_pinned_curl_command(url, &resolve_arg),
        FETCH_TIMEOUT,
        MAX_FETCH_BYTES,
    )
    .map_err(|e| format!("fetch {} failed: {}", url, e))?;
    if !out.status.success() {
        return Err(format!("fetch {} exited with {}", url, out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run the bounded autonomous discovery pipeline for `domain`.
pub fn run_autonomous_discovery(
    domain: &str,
    scope_domains: &[String],
    save: bool,
    storage: &Arc<dyn StorageEngine>,
) -> Result<AutonomousDiscoverySummary> {
    let norm = normalize_fqdn(domain);
    check_scope(&norm, false).map_err(|denial| {
        crate::error::BalebosError::Validation(format!("Target out of scope: {}", denial))
    })?;

    let mut effective_scope = scope_domains.to_vec();
    if effective_scope.is_empty() {
        effective_scope.push(norm.clone());
    }
    let now = Utc::now();
    let mut summary = AutonomousDiscoverySummary {
        domain: norm.clone(),
        ..Default::default()
    };

    // 1. Certificate Transparency (crt.sh) bounded query
    let scope_set: BTreeSet<String> = effective_scope.iter().cloned().collect();
    let ct_url = format!("https://crt.sh/?q=%25.{}&output=json", norm);
    match fetch_bounded_url(&ct_url) {
        Ok(ct_json) if !ct_json.trim().is_empty() => {
            match parse_ct_json(ct_json.as_bytes(), &scope_set, 200) {
                Ok(cands) => {
                    summary.ct_candidates = cands.len();
                    summary.ct_entities = cands.iter().map(|c| c.entity.id.clone()).collect();
                    if save {
                        for (idx, c) in cands.into_iter().enumerate() {
                            storage.save(&EnrichmentCandidateRecord {
                                item_type: ItemType::EnrichmentCandidate,
                                id: format!("ct-{}-{}-{}", norm, now.timestamp(), idx),
                                candidate: c,
                            })?;
                        }
                    }
                }
                Err(e) => summary.errors.push(format!("crt.sh parse: {}", e)),
            }
        }
        Ok(_) => {}
        Err(e) => summary.errors.push(e),
    }

    // 2. DNS SRV / TXT from persisted DNS observations
    let records = storage.read_all_records()?;
    let mut txts = Vec::new();
    let mut srvs = Vec::new();
    for dns in &records.dns_observations {
        let target_norm = normalize_fqdn(&dns.target);
        if target_norm == norm || target_norm.ends_with(&format!(".{}", norm)) {
            txts.extend(dns.txt.clone());
            srvs.extend(dns.srv.clone());
        }
    }
    let dns_refs = extract_dns_srv_txt_references(&norm, &srvs, &txts, &effective_scope, now);
    summary.dns_references = dns_refs.len();

    let mut all_refs: Vec<ApplicationReferenceObservation> = dns_refs;

    // 3. Public HTML -> bounded same-scope JS bundles + OpenAPI/Swagger specs
    let base_url = format!("https://{}", norm);
    match fetch_bounded_url(&base_url) {
        Ok(html) => {
            for script_url in extract_script_srcs_from_html(&base_url, &html) {
                if !is_url_in_scope(&script_url, &effective_scope) {
                    continue;
                }
                match fetch_bounded_url(&script_url) {
                    Ok(js) => {
                        summary.js_assets_fetched += 1;
                        let js_refs = extract_js_references(
                            &norm,
                            &script_url,
                            &js,
                            &effective_scope,
                            None,
                            now,
                        );
                        summary.js_references += js_refs.len();
                        all_refs.extend(js_refs);
                    }
                    Err(e) => summary.errors.push(e),
                }
            }
        }
        Err(e) => summary.errors.push(e),
    }

    for spec_path in ["/openapi.json", "/swagger.json"] {
        let spec_url = format!("{}{}", base_url, spec_path);
        if let Ok(body) = fetch_bounded_url(&spec_url) {
            if body.trim_start().starts_with('{') {
                if let Ok(oapi_refs) =
                    parse_openapi_references(&norm, &spec_url, &body, &effective_scope, None, now)
                {
                    summary.openapi_references += oapi_refs.len();
                    all_refs.extend(oapi_refs);
                }
            }
        }
    }

    if save {
        for r in &all_refs {
            storage.save(r)?;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_caps_script_srcs_from_html() {
        let html = r#"
          <script src="/assets/app.js"></script>
          <script src="https://www.example.com/vendor.js"></script>
          <script src="//cdn.example.com/chunk1.js"></script>
          <script src="main.js"></script>
          <script src="/assets/extra1.js"></script>
          <script src="/assets/extra2.js"></script>
        "#;
        let srcs = extract_script_srcs_from_html("https://www.example.com", html);
        assert_eq!(srcs.len(), MAX_JS_ASSETS_PER_DOMAIN);
        assert_eq!(srcs[0], "https://www.example.com/assets/app.js");
        assert_eq!(srcs[2], "https://cdn.example.com/chunk1.js");
        assert_eq!(srcs[3], "https://www.example.com/main.js");
    }

    #[test]
    fn scope_check_rejects_path_substring_and_suffix_spoofing() {
        let scope = vec!["example.com".to_string()];
        assert!(is_url_in_scope("https://cdn.example.com/app.js", &scope));
        assert!(is_url_in_scope("https://example.com/app.js", &scope));
        assert!(!is_url_in_scope(
            "https://evil.com/example.com/app.js",
            &scope
        ));
        assert!(!is_url_in_scope("https://notexample.com/app.js", &scope));
        assert!(!is_url_in_scope(
            "http://127.0.0.1/example.com/app.js",
            &scope
        ));
    }

    #[test]
    fn validate_resolved_ips_blocks_private_loopback_and_unspecified() {
        let public_ip: IpAddr = "93.184.216.34".parse().unwrap();
        assert_eq!(
            validate_resolved_ips("example.com", &[public_ip]).unwrap(),
            public_ip
        );
        for bad in [
            "127.0.0.1",
            "10.0.0.5",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "::1",
        ] {
            let ip: IpAddr = bad.parse().unwrap();
            assert!(
                validate_resolved_ips("example.com", &[public_ip, ip]).is_err(),
                "expected {} to be rejected",
                bad
            );
        }
    }

    #[test]
    fn pinned_curl_command_disables_proxies_and_redirects() {
        let cmd = build_pinned_curl_command(
            "https://example.com/app.js",
            "example.com:443:93.184.216.34",
        );
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|w| w == ["--noproxy", "*"]));
        assert!(args.windows(2).any(|w| w == ["--max-redirs", "0"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--resolve", "example.com:443:93.184.216.34"]));
        let envs: Vec<(String, Option<String>)> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|val| val.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert!(envs.iter().any(|(k, v)| k == "HTTPS_PROXY" && v.is_none()));
        assert!(envs.iter().any(|(k, v)| k == "HTTP_PROXY" && v.is_none()));
    }
}
