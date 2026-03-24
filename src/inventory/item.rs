use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::Domain;

/// Type discriminator for all records in the JSONL storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemType {
    #[serde(rename = "domain")]
    Domain,
    #[serde(rename = "subdomain")]
    Subdomain,
    #[serde(rename = "http_check")]
    HttpCheck,
    #[serde(rename = "dns_record")]
    DnsRecord,
    #[serde(rename = "org_person")]
    OrgPerson,
    #[serde(rename = "org_company")]
    OrgCompany,
    #[serde(rename = "org_brand")]
    OrgBrand,
    #[serde(rename = "org_project")]
    OrgProject,
    #[serde(rename = "org_asset_assignment")]
    OrgAssetAssignment,
    #[serde(rename = "org_role_assignment")]
    OrgRoleAssignment,
    #[serde(rename = "org_contractor_assignment")]
    OrgContractorAssignment,
    #[serde(rename = "org_asset_importance")]
    OrgAssetImportance,
    #[serde(rename = "org_domain_registration")]
    OrgDomainRegistration,
}

// This struct represents an item in our inventory storage
// It's designed for serialization/deserialization to/from JSONL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    // Type of the inventory item
    pub item_type: ItemType,

    // Unique identifier for the item
    pub id: String,

    // Name of the item (e.g., normalized domain name)
    pub name: String,

    // When this item was discovered/added to inventory
    pub discovered_at: DateTime<Utc>,

    // Where this item came from (e.g., "cli", could be "api", "scan", etc. in the future)
    pub source: String,

    // Original input (before normalization) - optional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_input: Option<String>,
}

impl InventoryItem {
    // Create a new inventory item from a domain
    pub fn from_domain(domain: &Domain, source: &str, original_input: Option<String>) -> Self {
        Self {
            item_type: ItemType::Domain,
            id: format!("{:?}", domain.id),
            name: domain.name.clone(),
            discovered_at: domain.discovered_at,
            source: source.to_string(),
            original_input,
        }
    }
}
