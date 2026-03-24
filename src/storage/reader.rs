use crate::app::probe::{DnsObservation, HttpObservation};
use crate::error::Result;
use crate::inventory::{InventoryItem, ItemType};
use crate::org::{
    AssetAssignmentRecord, AssetImportanceRecord, BrandRecord, CompanyRecord,
    DomainRegistrationRecord, ExternalContractorAssignment, PersonRecord, ProjectRecord,
    RoleAssignmentRecord,
};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::JsonlStorage;

#[derive(Deserialize)]
struct RecordHeader {
    item_type: ItemType,
}

/// All typed records extracted from a heterogeneous JSONL log.
#[derive(Default)]
pub struct Records {
    pub items: Vec<InventoryItem>,
    pub http_observations: Vec<HttpObservation>,
    pub dns_observations: Vec<DnsObservation>,
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
}

/// Read a heterogeneous JSONL log and separate into typed records.
///
/// Skips empty lines and malformed JSON silently.
/// Unknown `item_type` values are ignored.
pub fn read_records(storage: &JsonlStorage) -> Result<Records> {
    let file_path = storage.get_filepath();
    let path = Path::new(file_path);

    if !path.exists() {
        return Ok(Records::default());
    }

    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut records = Records::default();

    for line_result in reader.lines() {
        let line = line_result?;

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
        }
    }

    Ok(records)
}
