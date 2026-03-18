use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::io;
use std::time::{Duration, SystemTime};

pub struct InventoryDiff {
    // Domains
    pub new_domains: Vec<String>,
    pub missing_domains: Vec<String>,

    // Subdomains
    pub new_subdomains: Vec<String>,
    pub missing_subdomains: Vec<String>,
}

// Parse a string as a duration in days
fn parse_days(days_str: &str) -> Option<Duration> {
    days_str
        .parse::<u64>()
        .ok()
        .map(|days| Duration::from_secs(days * 24 * 60 * 60))
}

// Get time threshold for comparisons
fn get_time_threshold(days_ago: Option<&str>) -> SystemTime {
    let duration = days_ago
        .and_then(parse_days)
        .unwrap_or_else(|| Duration::from_secs(7 * 24 * 60 * 60)); // Default 7 days

    SystemTime::now()
        .checked_sub(duration)
        .unwrap_or_else(SystemTime::now)
}

pub fn get_inventory_diff(
    storage: &JsonlStorage,
    days_ago: Option<&str>,
) -> Result<InventoryDiff, io::Error> {
    let items: Vec<InventoryItem> = storage.read_all()?;

    // Threshold time for recent vs past
    let threshold = get_time_threshold(days_ago);

    // Process domains =======================================
    // Split domain items into recent and past
    let mut recent_domains = HashSet::new();
    let mut past_domains = HashSet::new();

    for item in &items {
        // Skip non-domain items
        if item.item_type != "domain" {
            continue;
        }

        // Convert DateTime<Utc> to SystemTime
        let datetime: DateTime<Utc> = item.discovered_at;
        let system_time = SystemTime::from(datetime);

        if system_time >= threshold {
            recent_domains.insert(&item.name);
        } else {
            past_domains.insert(&item.name);
        }
    }

    // Find new and missing domains
    let new_domains: Vec<String> = recent_domains
        .difference(&past_domains)
        .map(|&s| s.clone())
        .collect();

    let missing_domains: Vec<String> = past_domains
        .difference(&recent_domains)
        .map(|&s| s.clone())
        .collect();

    // Process subdomains ===================================
    // Now handle subdomains
    let mut recent_subdomains = HashSet::new();
    let mut past_subdomains = HashSet::new();

    for item in &items {
        if item.item_type != "subdomain" {
            continue;
        }

        // Convert DateTime<Utc> to SystemTime
        let datetime: DateTime<Utc> = item.discovered_at;
        let system_time = SystemTime::from(datetime);

        if system_time >= threshold {
            recent_subdomains.insert(&item.name);
        } else {
            past_subdomains.insert(&item.name);
        }
    }

    // Find new and missing subdomains
    let new_subdomains: Vec<String> = recent_subdomains
        .difference(&past_subdomains)
        .map(|&s| s.clone())
        .collect();

    let missing_subdomains: Vec<String> = past_subdomains
        .difference(&recent_subdomains)
        .map(|&s| s.clone())
        .collect();

    Ok(InventoryDiff {
        new_domains,
        missing_domains,
        new_subdomains,
        missing_subdomains,
    })
}
