use crate::error::Result;
use crate::inventory::ItemType;
use crate::storage::StorageEngine;
use std::sync::Arc;

pub fn list_domains(storage: &Arc<dyn StorageEngine>, unique: bool) -> Result<Vec<String>> {
    let mut domains: Vec<String> = storage
        .read_all_records()?
        .items
        .into_iter()
        .filter(|item| item.item_type == ItemType::Domain)
        .map(|item| item.name)
        .collect();

    // If unique flag is set, deduplicate while preserving order
    if unique {
        domains.sort();
        domains.dedup();
    }

    Ok(domains)
}
