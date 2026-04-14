use crate::app::finding::{Finding, FindingEvent};
use crate::inventory::LifecycleEvent;
use crate::app::probe::{AsnObservation, DnsObservation, HostingObservation, HttpObservation};
use crate::error::Result;
use crate::inventory::{AssetTag, InventoryItem, ItemType};
use crate::org::{
    AssetAssignmentRecord, AssetImportanceRecord, BrandRecord, CompanyRecord,
    DomainRegistrationRecord, ExternalContractorAssignment, PersonRecord, ProjectRecord,
    RoleAssignmentRecord,
};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Deserialize)]
pub struct RecordHeader {
    pub item_type: ItemType,
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
}

/// Parse typed records from an iterator of JSON string lines.
pub fn parse_records<I: Iterator<Item = String>>(lines: I) -> Result<Records> {
    let mut records = Records::default();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let header = match serde_json::from_str::<RecordHeader>(&line) {
            Ok(h) => h,
            Err(_) => continue,
        };

        match header.item_type {
            ItemType::Domain | ItemType::Subdomain => {
                if let Ok(item) = serde_json::from_str::<InventoryItem>(&line) {
                    records.items.push(item);
                }
            }
            ItemType::HttpCheck => {
                if let Ok(obs) = serde_json::from_str::<HttpObservation>(&line) {
                    records.http_observations.push(obs);
                }
            }
            ItemType::DnsRecord => {
                if let Ok(obs) = serde_json::from_str::<DnsObservation>(&line) {
                    records.dns_observations.push(obs);
                }
            }
            ItemType::AsnRecord => {
                if let Ok(obs) = serde_json::from_str::<AsnObservation>(&line) {
                    records.asn_observations.push(obs);
                }
            }
            ItemType::HostingDetection => {
                if let Ok(obs) = serde_json::from_str::<HostingObservation>(&line) {
                    records.hosting_observations.push(obs);
                }
            }
            ItemType::OrgPerson => {
                if let Ok(rec) = serde_json::from_str::<PersonRecord>(&line) {
                    records.persons.push(rec);
                }
            }
            ItemType::OrgCompany => {
                if let Ok(rec) = serde_json::from_str::<CompanyRecord>(&line) {
                    records.companies.push(rec);
                }
            }
            ItemType::OrgBrand => {
                if let Ok(rec) = serde_json::from_str::<BrandRecord>(&line) {
                    records.brands.push(rec);
                }
            }
            ItemType::OrgProject => {
                if let Ok(rec) = serde_json::from_str::<ProjectRecord>(&line) {
                    records.projects.push(rec);
                }
            }
            ItemType::OrgAssetAssignment => {
                if let Ok(rec) = serde_json::from_str::<AssetAssignmentRecord>(&line) {
                    records.asset_assignments.push(rec);
                }
            }
            ItemType::OrgRoleAssignment => {
                if let Ok(rec) = serde_json::from_str::<RoleAssignmentRecord>(&line) {
                    records.role_assignments.push(rec);
                }
            }
            ItemType::OrgContractorAssignment => {
                if let Ok(rec) = serde_json::from_str::<ExternalContractorAssignment>(&line) {
                    records.contractor_assignments.push(rec);
                }
            }
            ItemType::OrgAssetImportance => {
                if let Ok(rec) = serde_json::from_str::<AssetImportanceRecord>(&line) {
                    records.asset_importances.push(rec);
                }
            }
            ItemType::OrgDomainRegistration => {
                if let Ok(rec) = serde_json::from_str::<DomainRegistrationRecord>(&line) {
                    records.domain_registrations.push(rec);
                }
            }
            ItemType::AssetTag => {
                if let Ok(tag) = serde_json::from_str::<AssetTag>(&line) {
                    records.asset_tags.push(tag);
                }
            }
            ItemType::Finding => {
                if let Ok(f) = serde_json::from_str::<Finding>(&line) {
                    records.findings.push(f);
                }
            }
            ItemType::FindingEvent => {
                if let Ok(e) = serde_json::from_str::<FindingEvent>(&line) {
                    records.finding_events.push(e);
                }
            }
            ItemType::LifecycleEvent => {
                if let Ok(e) = serde_json::from_str::<LifecycleEvent>(&line) {
                    records.lifecycle_events.push(e);
                }
            }
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


