use crate::error::Result;
use crate::inventory::ItemType;
use crate::storage::StorageEngine;
use std::sync::Arc;

pub fn list_domains(storage: &Arc<dyn StorageEngine>, unique: bool) -> Result<Vec<String>> {
    let records = storage.read_all_records()?;
    let junk_assets = crate::app::tags::get_junk_assets(&records.asset_tags);

    let mut domains: Vec<String> = records
        .items
        .into_iter()
        .filter(|item| item.item_type == ItemType::Domain)
        .filter(|item| !crate::ignore::is_junk(&item.name, &junk_assets))
        .map(|item| item.name)
        .collect();

    // If unique flag is set, deduplicate while preserving order
    if unique {
        domains.sort();
        domains.dedup();
    }

    Ok(domains)
}
