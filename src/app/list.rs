use crate::error::Result;
use crate::inventory::ItemType;
use crate::storage::{read_records, JsonlStorage};

pub fn list_domains(storage: &JsonlStorage, unique: bool) -> Result<Vec<String>> {
    let mut domains: Vec<String> = read_records(storage)?
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
