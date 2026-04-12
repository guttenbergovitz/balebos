use crate::app::finding::{Finding, FindingCategory, FindingEvent, FindingEventType, FindingSeverity};
use crate::app::probe::{HttpErrorKind, HttpObservation};
use crate::domain::normalize_domain;
use crate::error::Result;
use crate::inventory::{InventoryItem, ItemType};
use crate::storage::{StorageEngine, StorageEngineExt};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

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

impl From<&Finding> for Issue {
    fn from(finding: &Finding) -> Self {
        let severity = match finding.severity {
            FindingSeverity::Critical | FindingSeverity::High => Severity::Critical,
            _ => Severity::Warning,
        };
        Issue::new(severity, &finding.message)
    }
}

// --- Internal check functions producing Finding ---

fn check_staleness_findings(items: &[InventoryItem], days_threshold: u64) -> Vec<Finding> {
    let mut domain_latest: HashMap<&String, DateTime<Utc>> = HashMap::new();

    for item in items
        .iter()
        .filter(|item| item.item_type == ItemType::Domain)
    {
        let entry = domain_latest
            .entry(&item.name)
            .or_insert(item.discovered_at);
        if item.discovered_at > *entry {
            *entry = item.discovered_at;
        }
    }

    let now = Utc::now();
    let mut findings = Vec::new();

    for (domain, latest) in domain_latest {
        let age = now.signed_duration_since(latest);
        let age_days = age.num_seconds() as u64 / (24 * 60 * 60);

        if age_days >= days_threshold {
            let message = format!("Domain {} last seen {} days ago", domain, age_days);
            findings.push(Finding::new(
                FindingSeverity::Medium,
                FindingCategory::Lifecycle,
                "staleness",
                domain,
                &message,
            ));
        }
    }

    findings
}

fn check_low_confidence_findings(items: &[InventoryItem]) -> Vec<Finding> {
    let mut domain_counts: HashMap<&String, u32> = HashMap::new();

    for item in items
        .iter()
        .filter(|item| item.item_type == ItemType::Domain)
    {
        *domain_counts.entry(&item.name).or_insert(0) += 1;
    }

    let mut findings = Vec::new();
    for (domain, count) in domain_counts {
        if count == 1 {
            let message = format!("Domain {} seen only once", domain);
            findings.push(Finding::new(
                FindingSeverity::Low,
                FindingCategory::Operational,
                "low_confidence",
                domain,
                &message,
            ));
        }
    }

    findings
}

fn check_unknown_sources_findings(items: &[InventoryItem]) -> Vec<Finding> {
    let known_sources: HashSet<&str> = ["cli"].iter().copied().collect();
    let mut unknown_sources: HashSet<&String> = HashSet::new();

    for item in items {
        if !known_sources.contains(item.source.as_str()) {
            unknown_sources.insert(&item.source);
        }
    }

    let mut findings = Vec::new();
    for source in unknown_sources {
        let message = format!("Unknown source: {}", source);
        findings.push(Finding::new(
            FindingSeverity::High,
            FindingCategory::Security,
            "unknown_source",
            source,
            &message,
        ));
    }

    findings
}

fn check_invalid_data_findings(items: &[InventoryItem]) -> Vec<Finding> {
    let mut findings = Vec::new();

    for item in items
        .iter()
        .filter(|item| item.item_type == ItemType::Domain)
    {
        let normalized = normalize_domain(&item.name);
        if normalized != item.name {
            let message = format!(
                "Domain {} is not normalized (should be {})",
                item.name, normalized
            );
            findings.push(Finding::new(
                FindingSeverity::Low,
                FindingCategory::Operational,
                "invalid_data",
                &item.name,
                &message,
            ));
        }

        if item.name.chars().any(|c| c.is_uppercase()) {
            let message = format!("Domain {} contains uppercase characters", item.name);
            findings.push(Finding::new(
                FindingSeverity::Low,
                FindingCategory::Operational,
                "invalid_data",
                &item.name,
                &message,
            ));
        }
    }

    findings
}

fn check_subdomain_patterns_findings(items: &[InventoryItem]) -> Vec<Finding> {
    let allowed_patterns = vec![
        "www", "api", "dev", "staging", "test", "admin", "mail", "smtp", "ftp", "blog", "shop",
        "store", "support",
    ];

    let mut findings = Vec::new();

    for item in items
        .iter()
        .filter(|item| item.item_type == ItemType::Subdomain)
    {
        let subdomain_parts: Vec<&str> = item.name.split('.').collect();

        if subdomain_parts.is_empty() {
            continue;
        }

        let first_part = subdomain_parts[0];

        if !allowed_patterns
            .iter()
            .any(|pattern| first_part.contains(pattern))
        {
            let message = format!("Suspicious subdomain pattern: {}", item.name);
            findings.push(Finding::new(
                FindingSeverity::Medium,
                FindingCategory::Security,
                "suspicious_subdomain",
                &item.name,
                &message,
            ));
        }
    }

    findings
}

fn check_http_status_findings(
    items: &[InventoryItem],
    http_observations: &[HttpObservation],
) -> Vec<Finding> {
    let mut findings = Vec::new();

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
        if let Some(error_kind) = &observation.error_kind {
            match error_kind {
                HttpErrorKind::DnsNotFound => {
                    findings.push(Finding::new(
                        FindingSeverity::Critical,
                        FindingCategory::Operational,
                        "dns_not_found",
                        domain,
                        &format!("Domain {} - DNS lookup failed", domain),
                    ));
                }
                HttpErrorKind::Timeout => {
                    findings.push(Finding::new(
                        FindingSeverity::Medium,
                        FindingCategory::Operational,
                        "http_timeout",
                        domain,
                        &format!("Domain {} - HTTP check timed out", domain),
                    ));
                }
                HttpErrorKind::TlsError => {
                    findings.push(Finding::new(
                        FindingSeverity::Medium,
                        FindingCategory::Security,
                        "tls_error",
                        domain,
                        &format!("Domain {} - TLS/certificate error", domain),
                    ));
                }
                HttpErrorKind::ConnectionError => {
                    findings.push(Finding::new(
                        FindingSeverity::Medium,
                        FindingCategory::Operational,
                        "connection_error",
                        domain,
                        &format!("Domain {} - Connection error", domain),
                    ));
                }
                HttpErrorKind::TooManyRedirects => {
                    findings.push(Finding::new(
                        FindingSeverity::Low,
                        FindingCategory::Operational,
                        "too_many_redirects",
                        domain,
                        &format!("Domain {} - Too many redirects", domain),
                    ));
                }
                HttpErrorKind::ProbeError => {
                    findings.push(Finding::new(
                        FindingSeverity::Low,
                        FindingCategory::Operational,
                        "probe_error",
                        domain,
                        &format!("Domain {} - HTTP probe failed", domain),
                    ));
                }
            }
        } else if let Some(status) = observation.status {
            if status >= 500 {
                findings.push(Finding::new(
                    FindingSeverity::Critical,
                    FindingCategory::Operational,
                    "http_server_error",
                    domain,
                    &format!(
                        "Domain {} returned HTTP status {} (server error)",
                        domain, status
                    ),
                ));
            } else if status >= 400 {
                findings.push(Finding::new(
                    FindingSeverity::Medium,
                    FindingCategory::Operational,
                    "http_client_error",
                    domain,
                    &format!(
                        "Domain {} returned HTTP status {} (client error)",
                        domain, status
                    ),
                ));
            }
        }

        // Check TLS expiration
        if let Some(tls) = &observation.tls {
            let now = Utc::now();
            let time_until_expiry = tls.not_after.signed_duration_since(now);
            let days_until_expiry = time_until_expiry.num_days();

            if days_until_expiry < 0 {
                findings.push(Finding::new(
                    FindingSeverity::Critical,
                    FindingCategory::Security,
                    "tls_expired",
                    domain,
                    &format!(
                        "Domain {} - TLS certificate EXPIRED on {}",
                        domain,
                        tls.not_after.format("%Y-%m-%d")
                    ),
                ));
            } else if days_until_expiry < 14 {
                findings.push(Finding::new(
                    FindingSeverity::High,
                    FindingCategory::Security,
                    "tls_expiring_soon",
                    domain,
                    &format!(
                        "Domain {} - TLS certificate expires in {} days ({})",
                        domain,
                        days_until_expiry,
                        tls.not_after.format("%Y-%m-%d")
                    ),
                ));
            } else if days_until_expiry < 30 {
                findings.push(Finding::new(
                    FindingSeverity::Medium,
                    FindingCategory::Security,
                    "tls_expiring_soon",
                    domain,
                    &format!(
                        "Domain {} - TLS certificate expires in {} days ({})",
                        domain,
                        days_until_expiry,
                        tls.not_after.format("%Y-%m-%d")
                    ),
                ));
            }
        }
    }

    // Check for domains without HTTP checks in the last 24 hours
    let now = Utc::now();
    let one_day = chrono::Duration::days(1);

    let domains: HashSet<String> = items
        .iter()
        .filter(|item| item.item_type == ItemType::Domain)
        .map(|item| item.name.clone())
        .collect();

    for domain in domains {
        match domain_latest_http.get(&domain) {
            Some(obs) => {
                let age = now.signed_duration_since(obs.checked_at);
                if age > one_day {
                    let message = format!(
                        "No recent HTTP check for domain {} (last check: {})",
                        domain,
                        obs.checked_at.format("%Y-%m-%d %H:%M")
                    );
                    findings.push(Finding::new(
                        FindingSeverity::Low,
                        FindingCategory::Operational,
                        "missing_http_check",
                        &domain,
                        &message,
                    ));
                }
            }
            None => {
                let message = format!("No HTTP checks found for domain {}", domain);
                findings.push(Finding::new(
                    FindingSeverity::Low,
                    FindingCategory::Operational,
                    "missing_http_check",
                    &domain,
                    &message,
                ));
            }
        }
    }

    findings
}

// Run all checks and return findings (internal helper)
fn run_all_checks(
    items: &[InventoryItem],
    http_observations: &[HttpObservation],
) -> Vec<Finding> {
    let mut findings = Vec::new();

    findings.extend(check_staleness_findings(items, 14));
    findings.extend(check_low_confidence_findings(items));
    findings.extend(check_unknown_sources_findings(items));
    findings.extend(check_invalid_data_findings(items));
    findings.extend(check_subdomain_patterns_findings(items));
    findings.extend(check_http_status_findings(items, http_observations));

    findings
}

// Determine whether a finding is "open" (no Resolved event exists for it)
fn open_finding_keys(
    findings: &[Finding],
    finding_events: &[FindingEvent],
) -> HashSet<(String, String)> {
    let resolved_ids: HashSet<&String> = finding_events
        .iter()
        .filter(|e| e.event_type == FindingEventType::Resolved)
        .map(|e| &e.finding_id)
        .collect();

    findings
        .iter()
        .filter(|f| !resolved_ids.contains(&f.id))
        .map(|f| (f.check_name.clone(), f.target.clone()))
        .collect()
}

/// Run all checks and persist new (non-duplicate) findings to storage.
///
/// A finding is considered a duplicate if an open finding with the same
/// `check_name` and `target` already exists.
pub fn run_checks_and_persist(
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
) -> Result<Vec<Finding>> {
    let records = storage.read_all_records()?;
    let items = records.items;
    let http_observations = records.http_observations;

    if items.is_empty() {
        return Ok(Vec::new());
    }

    // Filter out ignored items
    let filtered_items: Vec<InventoryItem> = items
        .into_iter()
        .filter(|item| !crate::ignore::is_ignored(&item.name, ignore_patterns))
        .collect();

    let filtered_http_observations: Vec<HttpObservation> = http_observations
        .into_iter()
        .filter(|obs| !crate::ignore::is_ignored(&obs.target, ignore_patterns))
        .collect();

    // Run checks
    let candidate_findings = run_all_checks(&filtered_items, &filtered_http_observations);

    // Determine which check_name+target combos are already open
    let existing_keys = open_finding_keys(&records.findings, &records.finding_events);

    // Deduplicate
    let new_findings: Vec<Finding> = candidate_findings
        .into_iter()
        .filter(|f| !existing_keys.contains(&(f.check_name.clone(), f.target.clone())))
        .collect();

    // Persist each new finding
    for finding in &new_findings {
        storage.save(finding)?;
    }

    Ok(new_findings)
}

// Run all checks and collect issues (backward-compatible)
pub fn check_inventory(
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
) -> Result<Vec<Issue>> {
    let records = storage.read_all_records()?;
    let items = records.items;
    let http_observations = records.http_observations;

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

    let filtered_http_observations: Vec<HttpObservation> = http_observations
        .into_iter()
        .filter(|obs| !crate::ignore::is_ignored(&obs.target, ignore_patterns))
        .collect();

    // Run checks via Finding-based internals, then convert to Issue
    let findings = run_all_checks(&filtered_items, &filtered_http_observations);
    let issues: Vec<Issue> = findings.iter().map(Issue::from).collect();

    Ok(issues)
}

#[cfg(test)]
fn check_staleness(items: &[InventoryItem], days_threshold: u64) -> Vec<Issue> {
    check_staleness_findings(items, days_threshold)
        .iter()
        .map(Issue::from)
        .collect()
}

fn check_low_confidence(items: &[InventoryItem]) -> Vec<Issue> {
    check_low_confidence_findings(items)
        .iter()
        .map(Issue::from)
        .collect()
}

fn check_unknown_sources(items: &[InventoryItem]) -> Vec<Issue> {
    check_unknown_sources_findings(items)
        .iter()
        .map(Issue::from)
        .collect()
}

fn check_invalid_data(items: &[InventoryItem]) -> Vec<Issue> {
    check_invalid_data_findings(items)
        .iter()
        .map(Issue::from)
        .collect()
}

fn check_subdomain_patterns(items: &[InventoryItem]) -> Vec<Issue> {
    check_subdomain_patterns_findings(items)
        .iter()
        .map(Issue::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_item(name: &str, item_type: ItemType, source: &str, age_days: i64) -> InventoryItem {
        InventoryItem {
            item_type,
            id: format!("test-{}", name),
            name: name.to_string(),
            discovered_at: Utc::now() - Duration::days(age_days),
            source: source.to_string(),
            original_input: None,
        }
    }

    fn make_domain(name: &str, age_days: i64) -> InventoryItem {
        make_item(name, ItemType::Domain, "cli", age_days)
    }

    #[test]
    fn test_staleness_fresh_domain() {
        let items = vec![make_domain("example.com", 1)];
        let issues = check_staleness(&items, 14);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_staleness_stale_domain() {
        let items = vec![make_domain("example.com", 20)];
        let issues = check_staleness(&items, 14);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("last seen"));
    }

    #[test]
    fn test_low_confidence_single_observation() {
        let items = vec![make_domain("example.com", 1)];
        let issues = check_low_confidence(&items);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("seen only once"));
    }

    #[test]
    fn test_low_confidence_multiple_observations() {
        let items = vec![make_domain("example.com", 1), make_domain("example.com", 5)];
        let issues = check_low_confidence(&items);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_unknown_source_cli_is_known() {
        let items = vec![make_domain("example.com", 1)];
        let issues = check_unknown_sources(&items);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_unknown_source_detected() {
        let items = vec![make_item("example.com", ItemType::Domain, "mystery", 1)];
        let issues = check_unknown_sources(&items);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Critical);
    }

    #[test]
    fn test_invalid_data_uppercase() {
        let items = vec![make_item("Example.COM", ItemType::Domain, "cli", 1)];
        let issues = check_invalid_data(&items);
        assert!(issues.iter().any(|i| i.message.contains("uppercase")));
    }

    #[test]
    fn test_invalid_data_normalized() {
        let items = vec![make_domain("example.com", 1)];
        let issues = check_invalid_data(&items);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_suspicious_subdomain() {
        let items = vec![make_item(
            "xyzrandom.example.com",
            ItemType::Subdomain,
            "subfinder",
            1,
        )];
        let issues = check_subdomain_patterns(&items);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("Suspicious"));
    }

    #[test]
    fn test_allowed_subdomain_pattern() {
        let items = vec![make_item(
            "api.example.com",
            ItemType::Subdomain,
            "subfinder",
            1,
        )];
        let issues = check_subdomain_patterns(&items);
        assert!(issues.is_empty());
    }
}
