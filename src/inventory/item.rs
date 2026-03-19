use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::Domain;

// This struct represents an item in our inventory storage
// It's designed for serialization/deserialization to/from JSONL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    // Type of the inventory item (e.g., "domain")
    pub item_type: String,

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
            item_type: "domain".to_string(),
            id: format!("{:?}", domain.id),
            name: domain.name.clone(),
            discovered_at: domain.discovered_at,
            source: source.to_string(),
            original_input,
        }
    }
}
