use crate::app::finding::{Finding, FindingEvent};
use crate::app::probe::{AsnObservation, DnsObservation, HostingObservation, HttpObservation};
use crate::error::Result;
use crate::inventory::LifecycleEvent;
use crate::inventory::{AssetTag, InventoryItem, ItemType};
use crate::org::{
    AssetAssignmentRecord, AssetImportanceRecord, BrandRecord, CompanyRecord,
    DomainRegistrationRecord, ExternalContractorAssignment, PersonRecord, ProjectRecord,
    RoleAssignmentRecord,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Why a stored record could not be turned into a typed value.
///
/// Skipped records are reported, never dropped silently: evidence
/// integrity requires visibility into corruption and unknown data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The line is not valid JSON at all.
    InvalidJson,
    /// The record's `item_type` is not a known variant; carries the raw value.
    UnknownItemType(String),
    /// The JSON is valid and the type is known, but the record does not
    /// match the expected schema; carries the type.
    SchemaMismatch(String),
}

/// A record that was read but could not be deserialised into a typed value.
#[derive(Debug, Clone)]
pub struct SkippedRecord {
    pub reason: SkipReason,
    /// Truncated raw content for operator identification.
    pub preview: String,
}

/// All typed records extracted from a heterogeneous JSONL log.
#[derive(Default)]
pub struct Records {
    pub items: Vec<InventoryItem>,
    pub http_observations: Vec<HttpObservation>,
    pub dns_observations: Vec<DnsObservation>,
    pub asn_observations: Vec<AsnObservation>,
    pub hosting_observations: Vec<HostingObservation>,
    pub asset_tags: Vec<AssetTag>,
    pub findings: Vec<Finding>,
    pub finding_events: Vec<FindingEvent>,
    // Organizational records
    pub persons: Vec<PersonRecord>,
    pub companies: Vec<CompanyRecord>,
    pub brands: Vec<BrandRecord>,
    pub projects: Vec<ProjectRecord>,
    pub asset_assignments: Vec<AssetAssignmentRecord>,
    pub role_assignments: Vec<RoleAssignmentRecord>,
    pub contractor_assignments: Vec<ExternalContractorAssignment>,
    pub asset_importances: Vec<AssetImportanceRecord>,
    pub domain_registrations: Vec<DomainRegistrationRecord>,
    pub lifecycle_events: Vec<LifecycleEvent>,
    /// Per-scan coverage reports (ADR 0005).
    pub scan_reports: Vec<crate::domain::coverage::ScanReport>,
    /// Records that could not be typed, with reasons. Never silently empty.
    pub skipped: Vec<SkippedRecord>,
}

fn preview(line: &str) -> String {
    const MAX_PREVIEW: usize = 120;
    if line.len() <= MAX_PREVIEW {
        line.to_string()
    } else {
        format!("{}…", &line[..MAX_PREVIEW])
    }
}

/// Parse typed records from an iterator of JSON string lines.
///
/// Malformed lines, unknown record types and schema mismatches are collected
/// into `Records::skipped` rather than dropped.
pub fn parse_records<I: Iterator<Item = String>>(lines: I) -> Result<Records> {
    let mut records = Records::default();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                records.skipped.push(SkippedRecord {
                    reason: SkipReason::InvalidJson,
                    preview: preview(trimmed),
                });
                continue;
            }
        };

        let type_str = value
            .get("item_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let item_type = match ItemType::from_type_str(&type_str) {
            Some(t) => t,
            None => {
                records.skipped.push(SkippedRecord {
                    reason: SkipReason::UnknownItemType(type_str),
                    preview: preview(trimmed),
                });
                continue;
            }
        };

        macro_rules! try_parse {
            ($target:ty, $field:ident) => {
                match serde_json::from_str::<$target>(trimmed) {
                    Ok(v) => records.$field.push(v),
                    Err(_) => records.skipped.push(SkippedRecord {
                        reason: SkipReason::SchemaMismatch(type_str.clone()),
                        preview: preview(trimmed),
                    }),
                }
            };
        }

        match item_type {
            ItemType::Domain | ItemType::Subdomain => try_parse!(InventoryItem, items),
            ItemType::HttpCheck => try_parse!(HttpObservation, http_observations),
            ItemType::DnsRecord => try_parse!(DnsObservation, dns_observations),
            ItemType::AsnRecord => try_parse!(AsnObservation, asn_observations),
            ItemType::HostingDetection => try_parse!(HostingObservation, hosting_observations),
            ItemType::OrgPerson => try_parse!(PersonRecord, persons),
            ItemType::OrgCompany => try_parse!(CompanyRecord, companies),
            ItemType::OrgBrand => try_parse!(BrandRecord, brands),
            ItemType::OrgProject => try_parse!(ProjectRecord, projects),
            ItemType::OrgAssetAssignment => try_parse!(AssetAssignmentRecord, asset_assignments),
            ItemType::OrgRoleAssignment => try_parse!(RoleAssignmentRecord, role_assignments),
            ItemType::OrgContractorAssignment => {
                try_parse!(ExternalContractorAssignment, contractor_assignments)
            }
            ItemType::OrgAssetImportance => try_parse!(AssetImportanceRecord, asset_importances),
            ItemType::OrgDomainRegistration => {
                try_parse!(DomainRegistrationRecord, domain_registrations)
            }
            ItemType::AssetTag => try_parse!(AssetTag, asset_tags),
            ItemType::Finding => try_parse!(Finding, findings),
            ItemType::FindingEvent => try_parse!(FindingEvent, finding_events),
            ItemType::LifecycleEvent => try_parse!(LifecycleEvent, lifecycle_events),
            ItemType::ScanReport => try_parse!(crate::domain::coverage::ScanReport, scan_reports),
        }
    }

    Ok(records)
}

/// Read a heterogeneous JSONL log and separate into typed records.
#[allow(dead_code)]
pub fn read_records(file_path: &str) -> Result<Records> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Ok(Records::default());
    }

    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    parse_records(reader.lines().map_while(|r| r.ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_records_typed_scan_report() {
        let report = crate::domain::coverage::ScanReport::start(chrono::Utc::now()).finish(
            vec!["a.example.com".to_string()],
            vec![crate::domain::coverage::TargetCoverageRecord {
                target: "a.example.com".to_string(),
                coverage: crate::domain::coverage::TargetCoverage::Observed,
            }],
            chrono::Utc::now(),
        );
        let mut value = serde_json::to_value(&report).unwrap();
        value["item_type"] = serde_json::json!("scan_report");

        let records = parse_records(vec![value.to_string()].into_iter()).unwrap();
        assert_eq!(records.scan_reports.len(), 1);
        assert_eq!(records.scan_reports[0], report);
    }

    #[test]
    fn parse_records_reports_invalid_json() {
        let lines = vec!["not json at all".to_string()];
        let records = parse_records(lines.into_iter()).unwrap();

        assert!(records.items.is_empty());
        assert_eq!(records.skipped.len(), 1);
        assert_eq!(records.skipped[0].reason, SkipReason::InvalidJson);
        assert!(records.skipped[0].preview.contains("not json"));
    }

    #[test]
    fn parse_records_reports_unknown_item_type() {
        let lines = vec![r#"{"item_type":"mystery_type","name":"x.example.com"}"#.to_string()];
        let records = parse_records(lines.into_iter()).unwrap();

        assert!(records.items.is_empty());
        assert_eq!(records.skipped.len(), 1);
        match &records.skipped[0].reason {
            SkipReason::UnknownItemType(t) => assert_eq!(t, "mystery_type"),
            other => panic!("expected UnknownItemType, got {:?}", other),
        }
    }

    #[test]
    fn parse_records_reports_schema_mismatch_without_dropping_valid_records() {
        let valid = serde_json::json!({
            "item_type": "domain",
            "id": "1",
            "name": "example.com",
            "discovered_at": "2023-01-01T00:00:00Z",
            "source": "test"
        });
        // Known type, but missing every required field.
        let broken = r#"{"item_type":"domain"}"#.to_string();

        let lines = vec![valid.to_string(), broken];
        let records = parse_records(lines.into_iter()).unwrap();

        assert_eq!(records.items.len(), 1);
        assert_eq!(records.items[0].name, "example.com");
        assert_eq!(records.skipped.len(), 1);
        match &records.skipped[0].reason {
            SkipReason::SchemaMismatch(t) => assert_eq!(t, "domain"),
            other => panic!("expected SchemaMismatch, got {:?}", other),
        }
    }

    #[test]
    fn parse_records_missing_item_type_is_unknown_not_defaulted() {
        let lines = vec![r#"{"name":"no-type.example.com"}"#.to_string()];
        let records = parse_records(lines.into_iter()).unwrap();

        assert!(records.items.is_empty());
        assert_eq!(records.skipped.len(), 1);
        match &records.skipped[0].reason {
            SkipReason::UnknownItemType(t) => assert!(t.is_empty()),
            other => panic!("expected UnknownItemType, got {:?}", other),
        }
    }
}
