use crate::app::probe_dns::DnsObservation;
use crate::app::probe_http::HttpObservation;
use crate::inventory::InventoryItem;
use crate::org::{
    AssetAssignmentRecord, BrandRecord, CompanyRecord, PersonRecord, ProjectRecord,
    RoleAssignmentRecord,
};
use serde::Deserialize;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use super::JsonlStorage;

#[derive(Deserialize)]
struct RecordHeader {
    item_type: String,
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
}

/// Read a heterogeneous JSONL log and separate into typed records.
///
/// Skips empty lines and malformed JSON silently.
/// Unknown `item_type` values are ignored.
pub fn read_records(storage: &JsonlStorage) -> Result<Records, io::Error> {
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

        match header.item_type.as_str() {
            "domain" | "subdomain" => {
                if let Ok(item) = serde_json::from_str::<InventoryItem>(&line) {
                    records.items.push(item);
                }
            }
            "http_check" => {
                if let Ok(obs) = serde_json::from_str::<HttpObservation>(&line) {
                    records.http_observations.push(obs);
                }
            }
            "dns_record" => {
                if let Ok(obs) = serde_json::from_str::<DnsObservation>(&line) {
                    records.dns_observations.push(obs);
                }
            }
            "org_person" => {
                if let Ok(rec) = serde_json::from_str::<PersonRecord>(&line) {
                    records.persons.push(rec);
                }
            }
            "org_company" => {
                if let Ok(rec) = serde_json::from_str::<CompanyRecord>(&line) {
                    records.companies.push(rec);
                }
            }
            "org_brand" => {
                if let Ok(rec) = serde_json::from_str::<BrandRecord>(&line) {
                    records.brands.push(rec);
                }
            }
            "org_project" => {
                if let Ok(rec) = serde_json::from_str::<ProjectRecord>(&line) {
                    records.projects.push(rec);
                }
            }
            "org_asset_assignment" => {
                if let Ok(rec) = serde_json::from_str::<AssetAssignmentRecord>(&line) {
                    records.asset_assignments.push(rec);
                }
            }
            "org_role_assignment" => {
                if let Ok(rec) = serde_json::from_str::<RoleAssignmentRecord>(&line) {
                    records.role_assignments.push(rec);
                }
            }
            _ => {}
        }
    }

    Ok(records)
}
