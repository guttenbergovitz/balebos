//! Discovery source descriptors and external passive enrichment adapters
//! (ADR 0020): Certificate Transparency (`crt.sh` JSON contract) and
//! provider-neutral Passive DNS ingestion.
//!
//! External timestamps (`source_recorded_at`) are kept strictly separate
//! from Balebos `ingested_at` and direct `observed_at` measurements.

use std::collections::BTreeSet;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::normalize::normalize_domain;
use crate::domain::relationship::{EntityKind, EntityRef, SignalCategory};
use crate::error::{BalebosError, Result};
use crate::storage::reader::Records;

/// Whether a source performs passive lookups/correlation or active probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationMode {
    Passive,
    Active,
}

impl std::fmt::Display for OperationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passive => write!(f, "passive"),
            Self::Active => write!(f, "active"),
        }
    }
}

/// Metadata contract describing a discovery or enrichment source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverySourceDescriptor {
    pub id: &'static str,
    pub api_version: &'static str,
    pub mode: OperationMode,
    pub requires_credentials: bool,
    pub scope_required: bool,
    pub rate_limit_hint: &'static str,
    pub limitations: &'static str,
}

/// Catalogue of all external measurement engines and enrichment adapters.
pub fn registered_sources() -> Vec<DiscoverySourceDescriptor> {
    vec![
        DiscoverySourceDescriptor {
            id: "subfinder",
            api_version: "cli-v2",
            mode: OperationMode::Passive,
            requires_credentials: false,
            scope_required: false,
            rate_limit_hint: "120s bounded execution",
            limitations: "OSINT coverage varies by configured provider keys",
        },
        DiscoverySourceDescriptor {
            id: "dnsx",
            api_version: "cli-v1",
            mode: OperationMode::Active,
            requires_credentials: false,
            scope_required: true,
            rate_limit_hint: "100 targets/chunk",
            limitations: "Active DNS cannot enumerate unrelated domains on a shared IP",
        },
        DiscoverySourceDescriptor {
            id: "httpx",
            api_version: "cli-v1",
            mode: OperationMode::Active,
            requires_credentials: false,
            scope_required: true,
            rate_limit_hint: "100 targets/chunk",
            limitations: "Inspects HTTP/TLS endpoints only for targets with DNS or explicit URL",
        },
        DiscoverySourceDescriptor {
            id: "asnmap",
            api_version: "cli-v1",
            mode: OperationMode::Active,
            requires_credentials: false,
            scope_required: false,
            rate_limit_hint: "50 IPs/chunk",
            limitations: "Routing/ASN context only; never implies ownership",
        },
        DiscoverySourceDescriptor {
            id: "crt_sh",
            api_version: "crt.sh-json-v1",
            mode: OperationMode::Passive,
            requires_credentials: false,
            scope_required: false,
            rate_limit_hint: "1 req/query, bounded output",
            limitations:
                "Public CT logs may lag; wildcard SANs are patterns; issuance != live service",
        },
        DiscoverySourceDescriptor {
            id: "passive_dns",
            api_version: "balebos-pdns-v1",
            mode: OperationMode::Passive,
            requires_credentials: true,
            scope_required: false,
            rate_limit_hint: "Provider-dependent; offline fixture/import supported",
            limitations:
                "Provider first_seen/last_seen are historical claims, not uninterrupted validity",
        },
    ]
}

/// Normalised candidate surfaced by Certificate Transparency or Passive DNS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrichmentCandidate {
    pub entity: EntityRef,
    pub source_id: String,
    pub is_wildcard_pattern: bool,
    pub in_scope: bool,
    pub signal: SignalCategory,
    /// When the external provider claims the event occurred (if supplied).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_recorded_at: Option<DateTime<Utc>>,
    /// When Balebos ingested the record (never treated as historical observation time).
    pub ingested_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_ip: Option<String>,
    pub note: String,
}

/// Minimal subset of `crt.sh/?q=%.<domain>&output=json` entry fields.
#[derive(Debug, Deserialize)]
struct CrtShEntry {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    name_value: String,
    #[serde(default)]
    entry_timestamp: Option<String>,
    #[serde(default)]
    not_before: Option<String>,
}

fn parse_ct_timestamp(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let s = raw?.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
    {
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    None
}

/// Parse bounded Certificate Transparency JSON (`crt.sh` format) into
/// normalised, deduplicated `EnrichmentCandidate`s.
pub fn parse_ct_json(
    json_bytes: &[u8],
    approved_scope: &BTreeSet<String>,
    max_candidates: usize,
) -> Result<Vec<EnrichmentCandidate>> {
    let entries: Vec<CrtShEntry> = serde_json::from_slice(json_bytes)
        .map_err(|e| BalebosError::Validation(format!("invalid CT JSON: {e}")))?;

    let ingested_at = Utc::now();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();

    for entry in entries {
        let source_recorded_at = parse_ct_timestamp(
            entry
                .entry_timestamp
                .as_deref()
                .or(entry.not_before.as_deref()),
        );
        let cert_id = entry.id.map(|i| format!("crtsh:{i}"));

        for raw_name in entry.name_value.lines() {
            let trimmed = raw_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            let is_wildcard = trimmed.starts_with("*.");
            let clean_host = normalize_domain(trimmed.trim_start_matches("*."));
            if clean_host.is_empty() {
                continue;
            }
            let dedup_key = if is_wildcard {
                format!("*.{clean_host}")
            } else {
                clean_host.clone()
            };
            if !seen.insert(dedup_key.clone()) {
                continue;
            }

            let in_scope = approved_scope
                .iter()
                .any(|root| clean_host == *root || clean_host.ends_with(&format!(".{root}")));
            let kind = if clean_host.split('.').count() <= 2 {
                EntityKind::Domain
            } else {
                EntityKind::Fqdn
            };
            let note = if is_wildcard {
                "Wildcard CT SAN pattern; does not confirm individual live hosts".to_string()
            } else {
                "CT log entry; issuance does not prove live deployment or common ownership"
                    .to_string()
            };

            out.push(EnrichmentCandidate {
                entity: EntityRef::new(kind, dedup_key),
                source_id: "crt_sh".to_string(),
                is_wildcard_pattern: is_wildcard,
                in_scope,
                signal: SignalCategory::WeakSharedClue,
                source_recorded_at,
                ingested_at,
                certificate_id: cert_id.clone(),
                related_ip: None,
                note,
            });

            if out.len() >= max_candidates {
                return Ok(out);
            }
        }
    }

    Ok(out)
}

/// Provider-neutral Passive DNS input record (`balebos-pdns-v1`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PassiveDnsRecord {
    pub fqdn: String,
    pub ip: String,
    #[serde(default = "default_rrtype")]
    pub record_type: String,
    #[serde(default)]
    pub source_first_seen: Option<DateTime<Utc>>,
    #[serde(default)]
    pub source_last_seen: Option<DateTime<Utc>>,
    #[serde(default = "default_pdns_provider")]
    pub provider: String,
}

fn default_rrtype() -> String {
    "A".to_string()
}

fn default_pdns_provider() -> String {
    "passive_dns".to_string()
}

/// Parse provider-neutral Passive DNS JSON array into normalised candidates.
pub fn parse_passive_dns_json(
    json_bytes: &[u8],
    approved_scope: &BTreeSet<String>,
    max_candidates: usize,
) -> Result<Vec<EnrichmentCandidate>> {
    let rows: Vec<PassiveDnsRecord> = serde_json::from_slice(json_bytes)
        .map_err(|e| BalebosError::Validation(format!("invalid Passive DNS JSON: {e}")))?;

    let ingested_at = Utc::now();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut out = Vec::new();

    for row in rows {
        let host = normalize_domain(row.fqdn.trim().trim_end_matches('.'));
        let ip = row.ip.trim().to_string();
        if host.is_empty() || ip.is_empty() || !seen.insert((host.clone(), ip.clone())) {
            continue;
        }
        let in_scope = approved_scope
            .iter()
            .any(|root| host == *root || host.ends_with(&format!(".{root}")));
        let kind = if host.split('.').count() <= 2 {
            EntityKind::Domain
        } else {
            EntityKind::Fqdn
        };
        out.push(EnrichmentCandidate {
            entity: EntityRef::new(kind, host),
            source_id: format!("passive_dns:{}", row.provider),
            is_wildcard_pattern: false,
            in_scope,
            signal: SignalCategory::WeakSharedClue,
            source_recorded_at: row.source_last_seen.or(row.source_first_seen),
            ingested_at,
            certificate_id: None,
            related_ip: Some(ip),
            note: "Historical Passive DNS claim; provider timestamps do not guarantee continuous existence".to_string(),
        });

        if out.len() >= max_candidates {
            break;
        }
    }

    Ok(out)
}

/// Return candidates from `enrichment` that are not already present in `records.items`.
pub fn novel_candidates<'a>(
    records: &Records,
    enrichment: &'a [EnrichmentCandidate],
) -> Vec<&'a EnrichmentCandidate> {
    let known: BTreeSet<String> = records
        .items
        .iter()
        .map(|i| normalize_domain(&i.name))
        .collect();
    enrichment
        .iter()
        .filter(|c| !known.contains(c.entity.id.trim_start_matches("*.")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryItem, ItemType};

    #[test]
    fn ct_parser_separates_timestamps_flags_wildcards_and_finds_novel_hosts() {
        let fixture = br#"[
            {
                "id": 1001,
                "name_value": "*.internal.example.com\nnew-portal.example.com\nknown.example.com",
                "entry_timestamp": "2026-01-10T12:00:00Z"
            }
        ]"#;

        let mut scope = BTreeSet::new();
        scope.insert("example.com".to_string());

        let parsed = parse_ct_json(fixture, &scope, 50).unwrap();
        assert_eq!(parsed.len(), 3);
        assert!(parsed[0].is_wildcard_pattern);
        assert_eq!(
            parsed[0]
                .source_recorded_at
                .unwrap()
                .format("%Y-%m-%d")
                .to_string(),
            "2026-01-10"
        );
        assert!(parsed[1].in_scope);

        let mut records = Records::default();
        records.items.push(InventoryItem {
            item_type: ItemType::Domain,
            id: "1".to_string(),
            name: "known.example.com".to_string(),
            discovered_at: Utc::now(),
            source: "subfinder".to_string(),
            original_input: None,
        });

        let novel = novel_candidates(&records, &parsed);
        assert_eq!(novel.len(), 2);
        assert!(novel
            .iter()
            .any(|c| c.entity.id == "new-portal.example.com"));
    }

    #[test]
    fn passive_dns_parser_preserves_provider_timestamps_and_scope() {
        let fixture = br#"[
            {
                "fqdn": "legacy.example.net",
                "ip": "203.0.113.55",
                "record_type": "A",
                "source_first_seen": "2024-05-01T00:00:00Z",
                "source_last_seen": "2024-06-01T00:00:00Z",
                "provider": "fixture-pdns"
            }
        ]"#;

        let parsed = parse_passive_dns_json(fixture, &BTreeSet::new(), 50).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(!parsed[0].in_scope);
        assert_eq!(parsed[0].related_ip.as_deref(), Some("203.0.113.55"));
        assert_eq!(
            parsed[0]
                .source_recorded_at
                .unwrap()
                .format("%Y-%m-%d")
                .to_string(),
            "2024-06-01"
        );
    }
}
