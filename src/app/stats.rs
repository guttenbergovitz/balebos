use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use std::collections::HashSet;
use std::io;

pub struct DomainStats {
    pub total_entries: usize,
    pub unique_domains: usize,
    pub latest_domain: Option<String>,
    pub latest_timestamp: Option<String>,
}

pub fn get_domain_stats(storage: &JsonlStorage) -> Result<DomainStats, io::Error> {
    let items: Vec<InventoryItem> = storage.read_all()?;

    // Filter to get only domain items
    let domains: Vec<&InventoryItem> = items
        .iter()
        .filter(|item| item.item_type == "domain")
        .collect();

    // Total count of domain entries
    let total_entries = domains.len();

    // Count unique domains using a HashSet
    let unique_domains: HashSet<&String> = domains.iter().map(|item| &item.name).collect();

    // Find the latest entry by timestamp
    let latest = domains
        .iter()
        .max_by(|a, b| a.discovered_at.cmp(&b.discovered_at));

    // Extract latest domain and timestamp
    let (latest_domain, latest_timestamp) = match latest {
        Some(item) => (
            Some(item.name.clone()),
            Some(item.discovered_at.to_string()),
        ),
        None => (None, None),
    };

    Ok(DomainStats {
        total_entries,
        unique_domains: unique_domains.len(),
        latest_domain,
        latest_timestamp,
    })
}
