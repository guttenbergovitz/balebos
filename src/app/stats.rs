use crate::error::Result;
use crate::storage::{read_records, JsonlStorage};
use std::collections::{HashMap, HashSet};

pub struct DomainStats {
    pub total_entries: usize,
    pub unique_domains: usize,
    pub latest_domain: Option<String>,
    pub latest_timestamp: Option<String>,
    pub top_domains: Vec<(String, usize)>,
}

pub fn get_domain_stats(storage: &JsonlStorage) -> Result<DomainStats> {
    let items = read_records(storage)?.items;

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
