use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::io;
use std::time::{Duration, SystemTime};

pub struct InventoryDiff {
    pub new_domains: Vec<String>,
    pub missing_domains: Vec<String>,
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

    // Split items into recent and past
    let mut recent_items = Vec::new();
    let mut past_items = Vec::new();

    for item in &items {
        // Skip non-domain items
        if item.item_type != "domain" {
            continue;
        }

        // Convert DateTime<Utc> to SystemTime
        let datetime: DateTime<Utc> = item.discovered_at;
        let system_time = SystemTime::from(datetime);

        if system_time >= threshold {
            recent_items.push(item);
        } else {
            past_items.push(item);
        }
    }

    // Get unique domain names
    let recent_domains: HashSet<&String> = recent_items.iter().map(|item| &item.name).collect();
    let past_domains: HashSet<&String> = past_items.iter().map(|item| &item.name).collect();

    // Find new and missing domains
    let new_domains: Vec<String> = recent_domains
        .difference(&past_domains)
        .map(|&s| s.clone())
        .collect();

    let missing_domains: Vec<String> = past_domains
        .difference(&recent_domains)
        .map(|&s| s.clone())
        .collect();

    Ok(InventoryDiff {
        new_domains,
        missing_domains,
    })
}
