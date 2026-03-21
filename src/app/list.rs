use crate::storage::{read_records, JsonlStorage};
use std::io;

pub fn list_domains(storage: &JsonlStorage, unique: bool) -> Result<Vec<String>, io::Error> {
    let mut domains: Vec<String> = read_records(storage)?
        .items
        .into_iter()
        .filter(|item| item.item_type == "domain")
        .map(|item| item.name)
        .collect();

    // If unique flag is set, deduplicate while preserving order
    if unique {
        domains.sort();
        domains.dedup();
    }

    Ok(domains)
}
