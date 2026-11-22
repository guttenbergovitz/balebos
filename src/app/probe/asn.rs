use super::types::AsnObservation;
use crate::error::{ProbeError, Result};
use crate::inventory::ItemType;
use crate::storage::{StorageEngine, StorageEngineExt};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
struct AsnmapOutput {
    #[serde(rename = "as-number")]
    as_number: Option<String>,
    #[serde(rename = "as-name")]
    as_name: Option<String>,
    #[serde(rename = "as-country")]
    as_country: Option<String>,
    #[serde(rename = "input")]
    input: Option<String>,
}

/// ASN lookup result from fallback methods
#[derive(Debug, Clone)]
struct AsnLookupResult {
    asn: u32,
    org: String,
    country: String,
    source: String,
}

/// Fallback ASN lookup using Team Cymru DNS service.
/// Query: <reversed-ip>.origin.asn.cymru.com TXT
/// Returns: "ASN | Prefix | Country | Registry | Date"
fn lookup_asn_cymru(ip: &str) -> Option<AsnLookupResult> {
    // Only handle IPv4 for now
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return None;
    }

    // Reverse the IP octets
    let reversed = format!("{}.{}.{}.{}", parts[3], parts[2], parts[1], parts[0]);
    let query = format!("{}.origin.asn.cymru.com", reversed);

    // Use dig for DNS TXT lookup with timeout
    // Try gtimeout (macOS) first, then timeout (Linux)
    let output = match Command::new("gtimeout")
        .args(["5", "dig", "+short", "TXT", &query, "+time=3", "+tries=2"])
        .output()
    {
        Ok(o) => Some(o),
        Err(_) => Command::new("timeout")
            .args(["5", "dig", "+short", "TXT", &query, "+time=3", "+tries=2"])
            .output()
            .ok(),
    };

    let output = output?;

    if !output.status.success() {
        return None;
    }

    let result = String::from_utf8_lossy(&output.stdout);
    let result = result.trim().trim_matches('"');

    // Parse: "47447 | 194.6.208.0/22 | DE | ripencc | 2007-12-14"
    let parts: Vec<&str> = result.split('|').map(|s| s.trim()).collect();
    if parts.len() >= 3 {
        let asn = parts[0].parse::<u32>().ok()?;
        let country = parts[2].to_string();

        // Get org name from ASN (second query)
        let org = lookup_asn_name_cymru(asn).unwrap_or_else(|| "UNKNOWN".to_string());

        return Some(AsnLookupResult {
            asn,
            org,
            country,
            source: "cymru".to_string(),
        });
    }

    None
}

/// Get ASN organization name from Team Cymru
fn lookup_asn_name_cymru(asn: u32) -> Option<String> {
    let query = format!("AS{}.asn.cymru.com", asn);

    let output = Command::new("dig")
        .args(["+short", "TXT", &query])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let result = String::from_utf8_lossy(&output.stdout);
    let result = result.trim().trim_matches('"');

    // Parse: "47447 | DE | ripencc | 2007-12-14 | TTM Telekommunikation Mittleres Muensterland GmbH"
    let parts: Vec<&str> = result.split('|').map(|s| s.trim()).collect();
    if parts.len() >= 5 {
        return Some(parts[4].to_string());
    }

    None
}

/// Fallback ASN lookup using whois command.
fn lookup_asn_whois(ip: &str) -> Option<AsnLookupResult> {
    let output = Command::new("whois").arg(ip).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let result = String::from_utf8_lossy(&output.stdout);

    let mut asn: Option<u32> = None;
    let mut org: Option<String> = None;
    let mut country: Option<String> = None;

    for line in result.lines() {
        let line_lower = line.to_lowercase();

        // Look for origin/ASN
        if (line_lower.starts_with("origin:") || line_lower.starts_with("originas:"))
            && asn.is_none()
        {
            if let Some(value) = line.split(':').nth(1) {
                let cleaned = value.trim().trim_start_matches("AS");
                asn = cleaned.parse().ok();
            }
        }

        // Look for org name
        if (line_lower.starts_with("org-name:") || line_lower.starts_with("orgname:"))
            && org.is_none()
        {
            if let Some(value) = line.split(':').nth(1) {
                org = Some(value.trim().to_string());
            }
        }

        // Look for country
        if line_lower.starts_with("country:") && country.is_none() {
            if let Some(value) = line.split(':').nth(1) {
                country = Some(value.trim().to_uppercase());
            }
        }
    }

    if let Some(asn_val) = asn {
        return Some(AsnLookupResult {
            asn: asn_val,
            org: org.unwrap_or_else(|| "UNKNOWN".to_string()),
            country: country.unwrap_or_else(|| "XX".to_string()),
            source: "whois".to_string(),
        });
    }

    None
}

/// Try all ASN lookup methods in order: asnmap -> cymru -> whois
fn lookup_asn_with_fallback(ip: &str) -> Option<AsnLookupResult> {
    // Try Team Cymru first (fast, reliable, no auth)
    if let Some(result) = lookup_asn_cymru(ip) {
        return Some(result);
    }

    // Fallback to whois (slower but works offline for cached data)
    if let Some(result) = lookup_asn_whois(ip) {
        return Some(result);
    }

    None
}

/// Helper function to check if asnmap is correctly installed.
fn check_asnmap_installation() -> Result<()> {
    Command::new("asnmap")
        .arg("-version")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ProbeError::ToolNotFound {
                    tool: "asnmap".to_string(),
                }
            } else {
                ProbeError::Execution(e)
            }
        })?;
    Ok(())
}

/// Run a batch ASN probe for multiple IP addresses.
///
/// Uses asnmap as primary source, with Team Cymru DNS and whois as fallbacks.
/// Takes a map of IP -> hostname for correlation, since we need to track which
/// host each IP came from.
pub fn probe_asn_batch(
    ip_to_host: &HashMap<String, String>,
    storage: &Arc<dyn StorageEngine>,
) -> Result<Vec<AsnObservation>> {
    if ip_to_host.is_empty() {
        return Ok(Vec::new());
    }

    let ips: Vec<&String> = ip_to_host.keys().collect();
    let mut all_observations = Vec::new();
    let mut resolved_ips: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Try asnmap first (if available)
    if check_asnmap_installation().is_ok() {
        let chunk_size = 50;

        for chunk in ips.chunks(chunk_size) {
            // Use a temporary file for IPs
            let temp_dir = std::env::temp_dir();
            let temp_file = temp_dir.join(format!("balebos_asn_{}.txt", Uuid::new_v4()));
            let ip_list: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
            if fs::write(&temp_file, ip_list.join("\n")).is_err() {
                continue;
            }

            let output = Command::new("asnmap")
                .args(["-l", &temp_file.to_string_lossy(), "-json", "-silent"])
                .output();

            let _ = fs::remove_file(&temp_file);

            let output = match output {
                Ok(o) => o,
                Err(_) => continue,
            };

            if !output.status.success() {
                continue;
            }

            let reader = BufReader::new(&output.stdout[..]);

            for line in reader.lines().map_while(|l| l.ok()) {
                if line.trim().is_empty() {
                    continue;
                }

                if let Ok(parsed) = serde_json::from_str::<AsnmapOutput>(&line) {
                    let ip = parsed.input.unwrap_or_default();
                    let host = ip_to_host.get(&ip).cloned().unwrap_or_else(|| ip.clone());

                    // Parse ASN number (format: "ASXXXXX" or just number)
                    let asn = parsed
                        .as_number
                        .as_ref()
                        .and_then(|s| {
                            let cleaned = s.strip_prefix("AS").unwrap_or(s);
                            cleaned.parse::<u32>().ok()
                        })
                        .unwrap_or(0);

                    if asn == 0 {
                        continue;
                    }

                    resolved_ips.insert(ip.clone());

                    let obs = AsnObservation {
                        item_type: ItemType::AsnRecord,
                        id: Uuid::new_v4().to_string(),
                        target: ip,
                        host,
                        asn,
                        org: parsed.as_name.unwrap_or_else(|| "UNKNOWN".to_string()),
                        country: parsed.as_country.unwrap_or_else(|| "XX".to_string()),
                        checked_at: Utc::now(),
                        source: "asnmap".to_string(),
                    };
                    all_observations.push(obs);
                }
            }
        }
    }

    // Use fallbacks for IPs not resolved by asnmap
    let unresolved: Vec<&String> = ips
        .iter()
        .filter(|ip| !resolved_ips.contains(**ip))
        .copied()
        .collect();

    if !unresolved.is_empty() {
        for ip in unresolved {
            if let Some(result) = lookup_asn_with_fallback(ip) {
                let host = ip_to_host.get(ip).cloned().unwrap_or_else(|| ip.clone());

                let obs = AsnObservation {
                    item_type: ItemType::AsnRecord,
                    id: Uuid::new_v4().to_string(),
                    target: ip.clone(),
                    host,
                    asn: result.asn,
                    org: result.org,
                    country: result.country,
                    checked_at: Utc::now(),
                    source: result.source,
                };
                all_observations.push(obs);
            }
        }
    }

    // Save all observations
    if !all_observations.is_empty() {
        storage.save_batch(&all_observations)?;
    }

    Ok(all_observations)
}

/// Run ASN probe for a single IP address.
#[allow(dead_code)]
pub fn probe_asn(ip: &str, host: &str, storage: &Arc<dyn StorageEngine>) -> Result<AsnObservation> {
    let mut ip_to_host = HashMap::new();
    ip_to_host.insert(ip.to_string(), host.to_string());

    let mut results = probe_asn_batch(&ip_to_host, storage)?;

    if let Some(obs) = results.pop() {
        Ok(obs)
    } else {
        // Return empty observation if no result
        let obs = AsnObservation {
            item_type: ItemType::AsnRecord,
            id: Uuid::new_v4().to_string(),
            target: ip.to_string(),
            host: host.to_string(),
            asn: 0,
            org: "UNKNOWN".to_string(),
            country: "XX".to_string(),
            checked_at: Utc::now(),
            source: "asnmap".to_string(),
        };
        storage.save(&obs)?;
        Ok(obs)
    }
}

/// Build IP to host mapping from DNS observations.
/// This extracts all unique IPs from A and AAAA records and maps them back to hosts.
pub fn build_ip_host_map(
    dns_observations: &[super::types::DnsObservation],
) -> HashMap<String, String> {
    let mut ip_to_host = HashMap::new();

    for obs in dns_observations {
        for ip in &obs.a {
            ip_to_host
                .entry(ip.clone())
                .or_insert_with(|| obs.target.clone());
        }
        for ip in &obs.aaaa {
            ip_to_host
                .entry(ip.clone())
                .or_insert_with(|| obs.target.clone());
        }
    }

    ip_to_host
}
