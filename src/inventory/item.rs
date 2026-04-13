use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::Domain;

/// Type discriminator for all records in the JSONL storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    #[serde(rename = "domain")]
    Domain,
    #[serde(rename = "subdomain")]
    Subdomain,
    #[serde(rename = "http_check")]
    HttpCheck,
    #[serde(rename = "dns_record")]
    DnsRecord,
    #[serde(rename = "asn_record")]
    AsnRecord,
    #[serde(rename = "hosting_detection")]
    HostingDetection,
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
    #[serde(rename = "asset_tag")]
    AssetTag,
    #[serde(rename = "finding")]
    Finding,
    #[serde(rename = "finding_event")]
    FindingEvent,
    #[serde(rename = "lifecycle_event")]
    LifecycleEvent,
}

impl ItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemType::Domain => "domain",
            ItemType::Subdomain => "subdomain",
            ItemType::HttpCheck => "http_check",
            ItemType::DnsRecord => "dns_record",
            ItemType::AsnRecord => "asn_record",
            ItemType::HostingDetection => "hosting_detection",
            ItemType::OrgPerson => "org_person",
            ItemType::OrgCompany => "org_company",
            ItemType::OrgBrand => "org_brand",
            ItemType::OrgProject => "org_project",
            ItemType::OrgAssetAssignment => "org_asset_assignment",
            ItemType::OrgRoleAssignment => "org_role_assignment",
            ItemType::OrgContractorAssignment => "org_contractor_assignment",
            ItemType::OrgAssetImportance => "org_asset_importance",
            ItemType::OrgDomainRegistration => "org_domain_registration",
            ItemType::AssetTag => "asset_tag",
            ItemType::Finding => "finding",
            ItemType::FindingEvent => "finding_event",
            ItemType::LifecycleEvent => "lifecycle_event",
        }
    }
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

/// Tag assignment for an asset (domain/subdomain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetTag {
    pub item_type: ItemType,
    pub id: String,
    pub asset_name: String,
    pub tag: String,
    pub tagged_at: DateTime<Utc>,
    pub tagged_by: String,
    pub removed_at: Option<DateTime<Utc>>,
}

impl AssetTag {
    pub fn new(asset_name: String, tag: String) -> Self {
        Self {
            item_type: ItemType::AssetTag,
            id: Uuid::new_v4().to_string(),
            asset_name,
            tag,
            tagged_at: Utc::now(),
            tagged_by: "cli".to_string(),
            removed_at: None,
        }
    }

    pub fn removed(asset_name: String, tag: String) -> Self {
        Self {
            item_type: ItemType::AssetTag,
            id: Uuid::new_v4().to_string(),
            asset_name,
            tag,
            tagged_at: Utc::now(),
            tagged_by: "cli".to_string(),
            removed_at: Some(Utc::now()),
        }
    }
}
