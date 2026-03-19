use crate::app::probe_http::{HttpErrorKind, HttpObservation};
use crate::domain::normalize_domain;
use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader};

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

// Check for HTTP status issues
fn check_http_status(items: &[InventoryItem], http_observations: &[HttpObservation]) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Find the latest HTTP check for each domain
    let mut domain_latest_http: HashMap<String, &HttpObservation> = HashMap::new();

    for obs in http_observations {
        if obs.asset_type == "domain" {
            match domain_latest_http.get(&obs.target) {
                Some(existing) => {
                    if obs.checked_at > existing.checked_at {
                        domain_latest_http.insert(obs.target.clone(), obs);
                    }
                }
                None => {
                    domain_latest_http.insert(obs.target.clone(), obs);
                }
            }
        }
    }

    // Check HTTP status codes and errors
    for (domain, observation) in &domain_latest_http {
        // If there was a network error
        if let Some(error_kind) = &observation.error_kind {
            match error_kind {
                HttpErrorKind::DnsNotFound => {
                    issues.push(Issue::critical(&format!(
                        "Domain {} - DNS lookup failed",
                        domain
                    )));
                }
                HttpErrorKind::Timeout => {
                    issues.push(Issue::warning(&format!(
                        "Domain {} - HTTP check timed out",
                        domain
                    )));
                }
                HttpErrorKind::TlsError => {
                    issues.push(Issue::warning(&format!(
                        "Domain {} - TLS/certificate error",
                        domain
                    )));
                }
                HttpErrorKind::ConnectionError => {
                    issues.push(Issue::warning(&format!(
                        "Domain {} - Connection error",
                        domain
                    )));
                }
                HttpErrorKind::ProbeError => {
                    issues.push(Issue::warning(&format!(
                        "Domain {} - HTTP probe failed",
                        domain
                    )));
                }
            }
        }
        // If request succeeded, check HTTP status codes
        else if let Some(status) = observation.status {
            if status >= 500 {
                issues.push(Issue::critical(&format!(
                    "Domain {} returned HTTP status {} (server error)",
                    domain, status
                )));
            } else if status >= 400 {
                issues.push(Issue::warning(&format!(
                    "Domain {} returned HTTP status {} (client error)",
                    domain, status
                )));
            }
        }
    }

    // Check for domains without HTTP checks in the last 24 hours
    let now = Utc::now();
    let one_day = chrono::Duration::days(1);

    // Build set of domains in inventory
    let domains: HashSet<String> = items
        .iter()
        .filter(|item| item.item_type == "domain")
        .map(|item| item.name.clone())
        .collect();

    // Check which domains are missing HTTP observations
    for domain in domains {
        match domain_latest_http.get(&domain) {
            Some(obs) => {
                // Check if the last observation is older than 24 hours
                let age = now.signed_duration_since(obs.checked_at);
                if age > one_day {
                    let message = format!(
                        "No recent HTTP check for domain {} (last check: {})",
                        domain,
                        obs.checked_at.format("%Y-%m-%d %H:%M")
                    );
                    issues.push(Issue::warning(&message));
                }
            }
            None => {
                // This domain has no HTTP observations at all
                let message = format!("No HTTP checks found for domain {}", domain);
                issues.push(Issue::warning(&message));
            }
        }
    }

    issues
}

// Run all checks and collect issues
pub fn check_inventory(
    storage: &JsonlStorage,
    ignore_patterns: &[String],
) -> Result<Vec<Issue>, io::Error> {
    let items: Vec<InventoryItem> = storage.read_all()?;

    // Read HTTP observations - using direct file reading
    // to avoid serialization type issues
    let file_path = storage.get_filepath();
    let mut http_observations = Vec::new();

    if let Ok(file) = std::fs::File::open(file_path) {
        let reader = BufReader::new(file);
        for line_content in reader.lines().map_while(Result::ok) {
            if line_content.contains("\"item_type\":\"http_check\"") {
                match serde_json::from_str::<HttpObservation>(&line_content) {
                    Ok(obs) => http_observations.push(obs),
                    Err(e) => eprintln!("Error parsing HTTP observation: {}", e),
                }
            }
        }
    }

    if items.is_empty() {
        return Ok(vec![Issue::warning("No items in inventory")]);
    }

    // Filter out ignored items
    let mut ignored_count = 0;

    let filtered_items: Vec<InventoryItem> = items
        .into_iter()
        .filter(|item| {
            let should_ignore = crate::ignore::is_ignored(&item.name, ignore_patterns);
            if should_ignore {
                ignored_count += 1;
            }
            !should_ignore
        })
        .collect();

    // If we filtered out everything, special case
    if !filtered_items.is_empty() {
        println!(
            "Ignoring {} items based on balebos.ignore patterns",
            ignored_count
        );
    }

    let mut issues = Vec::new();

    // Run each check using filtered items
    issues.extend(check_staleness(&filtered_items, 14)); // 14 days threshold
    issues.extend(check_low_confidence(&filtered_items));
    issues.extend(check_unknown_sources(&filtered_items));
    issues.extend(check_invalid_data(&filtered_items));
    issues.extend(check_subdomain_patterns(&filtered_items)); // Check for suspicious subdomains

    // Filter HTTP observations for ignored domains
    let filtered_http_observations: Vec<HttpObservation> = http_observations
        .into_iter()
        .filter(|obs| !crate::ignore::is_ignored(&obs.target, ignore_patterns))
        .collect();

    issues.extend(check_http_status(
        &filtered_items,
        &filtered_http_observations,
    )); // Check HTTP status issues

    Ok(issues)
}
