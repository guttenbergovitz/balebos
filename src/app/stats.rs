use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

// Lightweight header to identify record type
#[derive(Deserialize)]
struct RecordHeader {
    item_type: String,
}

pub struct DomainStats {
    pub total_entries: usize,
    pub unique_domains: usize,
    pub latest_domain: Option<String>,
    pub latest_timestamp: Option<String>,
    pub top_domains: Vec<(String, usize)>,
}

pub fn get_domain_stats(storage: &JsonlStorage) -> Result<DomainStats, io::Error> {
    let file_path = storage.get_filepath();
    let path = Path::new(file_path);

    let mut items = Vec::new();

    if path.exists() {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        for line_result in reader.lines() {
            let line = line_result?;

            if line.trim().is_empty() {
                continue;
            }

            // Check record type without full deserialization
            let header = match serde_json::from_str::<RecordHeader>(&line) {
                Ok(h) => h,
                Err(_) => continue,
            };

            // Only process domain and subdomain records
            if header.item_type == "domain" || header.item_type == "subdomain" {
                if let Ok(item) = serde_json::from_str::<InventoryItem>(&line) {
                    items.push(item);
                }
            }
        }
    }

    // Total count of domain entries (items already filtered to domain/subdomain)
    let total_entries = items.len();

    // Count unique domains using a HashSet
    let unique_domains: HashSet<&String> = items.iter().map(|item| &item.name).collect();

    // Find the latest entry by timestamp
    let latest = items
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

    // Count occurrences of each domain
    let mut domain_counts = HashMap::new();
    for item in items.iter() {
        *domain_counts.entry(item.name.clone()).or_insert(0) += 1;
    }

    // Find top domains (by count)
    let mut top_domains: Vec<(String, usize)> = domain_counts
        .iter()
        .map(|(name, count)| (name.clone(), *count))
        .collect();

    // Sort by count (descending)
    top_domains.sort_by(|a, b| b.1.cmp(&a.1));

    // Limit to top 5 for display
    if top_domains.len() > 5 {
        top_domains.truncate(5);
    }

    Ok(DomainStats {
        total_entries,
        unique_domains: unique_domains.len(),
        latest_domain,
        latest_timestamp,
        top_domains,
    })
}
