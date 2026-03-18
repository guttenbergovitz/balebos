use crate::domain::normalize_domain;
use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::io;

// Severity of an issue
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Critical,
}

// Represents a detected issue
#[derive(Debug, Clone)]
pub struct Issue {
    pub severity: Severity,
    pub message: String,
}

impl Issue {
    fn new(severity: Severity, message: &str) -> Self {
        Self {
            severity,
            message: message.to_string(),
        }
    }

    fn warning(message: &str) -> Self {
        Self::new(Severity::Warning, message)
    }

    fn critical(message: &str) -> Self {
        Self::new(Severity::Critical, message)
    }
}

// Check for domains that haven't been seen in X days (defaults to 14)
fn check_staleness(items: &[InventoryItem], days_threshold: u64) -> Vec<Issue> {
    // Group items by domain name
    let mut domain_latest: HashMap<&String, DateTime<Utc>> = HashMap::new();

    // Find the latest timestamp for each domain
    for item in items.iter().filter(|item| item.item_type == "domain") {
        let entry = domain_latest
            .entry(&item.name)
            .or_insert(item.discovered_at);
        if item.discovered_at > *entry {
            *entry = item.discovered_at;
        }
    }

    // Current time for comparison
    let now = Utc::now();

    // Check which domains are stale
    let mut issues = Vec::new();
    for (domain, latest) in domain_latest {
        let age = now.signed_duration_since(latest);
        let age_days = age.num_seconds() as u64 / (24 * 60 * 60);

        if age_days >= days_threshold {
            let message = format!("Domain {} last seen {} days ago", domain, age_days);
            issues.push(Issue::warning(&message));
        }
    }

    issues
}

// Check for domains that have been seen only once
fn check_low_confidence(items: &[InventoryItem]) -> Vec<Issue> {
    // Count occurrences of each domain
    let mut domain_counts: HashMap<&String, u32> = HashMap::new();

    for item in items.iter().filter(|item| item.item_type == "domain") {
        *domain_counts.entry(&item.name).or_insert(0) += 1;
    }

    // Find domains seen only once
    let mut issues = Vec::new();
    for (domain, count) in domain_counts {
        if count == 1 {
            let message = format!("Domain {} seen only once", domain);
            issues.push(Issue::warning(&message));
        }
    }

    issues
}

// Check for unknown sources in the inventory
fn check_unknown_sources(items: &[InventoryItem]) -> Vec<Issue> {
    // Define known sources
    let known_sources: HashSet<&str> = ["cli"].iter().copied().collect();

    // Find unknown sources
    let mut unknown_sources: HashSet<&String> = HashSet::new();

    for item in items {
        if !known_sources.contains(item.source.as_str()) {
            unknown_sources.insert(&item.source);
        }
    }

    // Create issues for unknown sources
    let mut issues = Vec::new();
    for source in unknown_sources {
        let message = format!("Unknown source: {}", source);
        issues.push(Issue::critical(&message));
    }

    issues
}

// Check for unnormalized or invalid domain data
fn check_invalid_data(items: &[InventoryItem]) -> Vec<Issue> {
    let mut issues = Vec::new();

    for item in items.iter().filter(|item| item.item_type == "domain") {
        // Check if domain is properly normalized
        let normalized = normalize_domain(&item.name);
        if normalized != item.name {
            let message = format!(
                "Domain {} is not normalized (should be {})",
                item.name, normalized
            );
            issues.push(Issue::warning(&message));
        }

        // Check for uppercase domains
        if item.name.chars().any(|c| c.is_uppercase()) {
            let message = format!("Domain {} contains uppercase characters", item.name);
            issues.push(Issue::warning(&message));
        }

        // Check if original domain contains http:// or www.
        if let Some(ref original) = item.original_input {
            if original.contains("http://")
                || original.contains("https://")
                || original.starts_with("www.")
            {
                // This is actually expected behavior, not an issue, so we don't report it
            }
        }
    }

    issues
}

// Check for suspicious subdomains that don't match expected patterns
fn check_subdomain_patterns(items: &[InventoryItem]) -> Vec<Issue> {
    // Common legitimate patterns for subdomains
    let allowed_patterns = vec![
        "www", "api", "dev", "staging", "test", "admin", "mail", "smtp", "ftp", "blog", "shop",
        "store", "support",
    ];

    let mut issues = Vec::new();

    for item in items.iter().filter(|item| item.item_type == "subdomain") {
        // Check if subdomain starts with any allowed pattern
        let subdomain_parts: Vec<&str> = item.name.split('.').collect();

        if subdomain_parts.is_empty() {
            continue;
        }

        // Get the first part of the subdomain (before first dot)
        let first_part = subdomain_parts[0];

        // If it doesn't match any allowed pattern, flag it
        if !allowed_patterns
            .iter()
            .any(|pattern| first_part.contains(pattern))
        {
            let message = format!("Suspicious subdomain pattern: {}", item.name);
            issues.push(Issue::warning(&message));
        }
    }

    issues
}

// Run all checks and collect issues
pub fn check_inventory(storage: &JsonlStorage) -> Result<Vec<Issue>, io::Error> {
    let items: Vec<InventoryItem> = storage.read_all()?;

    if items.is_empty() {
        return Ok(vec![Issue::warning("No items in inventory")]);
    }

    let mut issues = Vec::new();

    // Run each check
    issues.extend(check_staleness(&items, 14)); // 14 days threshold
    issues.extend(check_low_confidence(&items));
    issues.extend(check_unknown_sources(&items));
    issues.extend(check_invalid_data(&items));
    issues.extend(check_subdomain_patterns(&items)); // New check for suspicious subdomains

    Ok(issues)
}
