use crate::app::finding::{
    Finding, FindingCategory, FindingEvent, FindingEventType, FindingSeverity,
};
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
        Issue {
            severity,
            message: message.to_string(),
        }
    }

    fn warning(message: &str) -> Self {
        Self::new(Severity::Warning, message)
    }

    #[allow(dead_code)]
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
    lifecycle_events: &[crate::inventory::LifecycleEvent],
) -> Vec<Finding> {
    let mut findings = Vec::new();

    findings.extend(check_staleness_findings(items, 14));
    findings.extend(check_low_confidence_findings(items));
    findings.extend(check_unknown_sources_findings(items));
    findings.extend(check_invalid_data_findings(items));
    findings.extend(check_subdomain_patterns_findings(items));
    findings.extend(check_http_status_findings(items, http_observations));
    findings.extend(check_abandoned_assets(
        items,
        lifecycle_events,
        http_observations,
    ));
    findings.extend(check_decommissioned_still_present(
        items,
        lifecycle_events,
        http_observations,
    ));

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

    // Load junk assets for filtering
    let junk_assets = crate::app::tags::get_junk_assets(&records.asset_tags);

    // Filter out ignored and junk items
    let filtered_items: Vec<InventoryItem> = items
        .into_iter()
        .filter(|item| {
            !crate::ignore::is_ignored(&item.name, ignore_patterns)
                && !crate::ignore::is_junk(&item.name, &junk_assets)
        })
        .collect();

    let filtered_http_observations: Vec<HttpObservation> = http_observations
        .into_iter()
        .filter(|obs| {
            !crate::ignore::is_ignored(&obs.target, ignore_patterns)
                && !crate::ignore::is_junk(&obs.target, &junk_assets)
        })
        .collect();

    // Run checks
    let candidate_findings = run_all_checks(
        &filtered_items,
        &filtered_http_observations,
        &records.lifecycle_events,
    );

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

    // Auto-resolve findings that are no longer valid
    auto_resolve_findings(storage, &filtered_items, &filtered_http_observations)?;

    Ok(new_findings)
}

/// Automatically resolve findings that are no longer valid
fn auto_resolve_findings(
    storage: &Arc<dyn StorageEngine>,
    items: &[InventoryItem],
    http_observations: &[HttpObservation],
) -> Result<()> {
    use std::collections::HashMap;

    let records = storage.read_all_records()?;

    // Get all open findings
    let open_findings: Vec<&Finding> = records
        .findings
        .iter()
        .filter(|f| {
            // Check if finding is resolved
            !records
                .finding_events
                .iter()
                .any(|e| e.finding_id == f.id && e.event_type == FindingEventType::Resolved)
        })
        .collect();

    // Group items by domain for quick lookup
    let mut domain_items: HashMap<&String, Vec<&InventoryItem>> = HashMap::new();
    for item in items {
        if item.item_type == ItemType::Domain {
            domain_items.entry(&item.name).or_default().push(item);
        }
    }

    // Group HTTP observations by domain
    let mut domain_http_obs: HashMap<&String, Vec<&HttpObservation>> = HashMap::new();
    for obs in http_observations {
        domain_http_obs.entry(&obs.target).or_default().push(obs);
    }

    let now = Utc::now();

    for finding in open_findings {
        let should_auto_resolve = match finding.check_name.as_str() {
            "staleness" => {
                // Check if domain is no longer stale
                if let Some(domain_items_list) = domain_items.get(&finding.target) {
                    // Find the latest item for this domain
                    let latest_item = domain_items_list.iter().max_by_key(|i| i.discovered_at);
                    if let Some(item) = latest_item {
                        let age = now.signed_duration_since(item.discovered_at);
                        let age_days = age.num_seconds() as u64 / (24 * 60 * 60);
                        age_days < 14 // No longer stale if seen within 14 days
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "low_confidence" => {
                // Check if domain now has multiple observations
                if let Some(domain_items_list) = domain_items.get(&finding.target) {
                    domain_items_list.len() > 1
                } else {
                    false
                }
            }
            "missing_http_check" => {
                // Check if there's now a recent HTTP observation
                if let Some(obs_list) = domain_http_obs.get(&finding.target) {
                    let latest_obs = obs_list.iter().max_by_key(|o| o.checked_at);
                    if let Some(obs) = latest_obs {
                        let age = now.signed_duration_since(obs.checked_at);
                        let age_hours = age.num_hours();
                        age_hours < 24 // Has recent check (within 24h)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "http_server_error" | "http_client_error" | "tls_error" | "connection_error"
            | "timeout" => {
                // Check if the latest HTTP observation no longer has the error
                if let Some(obs_list) = domain_http_obs.get(&finding.target) {
                    let latest_obs = obs_list.iter().max_by_key(|o| o.checked_at);
                    if let Some(obs) = latest_obs {
                        // Check if the error is resolved
                        match finding.check_name.as_str() {
                            "http_server_error" => obs.status.map(|s| s < 500).unwrap_or(false),
                            "http_client_error" => obs.status.map(|s| s < 400).unwrap_or(false),
                            "tls_error" => obs.error_kind != Some(HttpErrorKind::TlsError),
                            "connection_error" => {
                                obs.error_kind != Some(HttpErrorKind::ConnectionError)
                            }
                            "timeout" => obs.error_kind != Some(HttpErrorKind::Timeout),
                            _ => false,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "abandoned_asset" => {
                // Check if asset is no longer abandoned (recent observations)
                if let Some(obs_list) = domain_http_obs.get(&finding.target) {
                    let latest_obs = obs_list.iter().max_by_key(|o| o.checked_at);
                    if let Some(obs) = latest_obs {
                        let age = now.signed_duration_since(obs.checked_at);
                        let age_days = age.num_seconds() as u64 / (24 * 60 * 60);
                        age_days < 30 // Has recent observation
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "decommissioned_still_present" => {
                // Check if asset is no longer responding (no recent observations)
                if let Some(obs_list) = domain_http_obs.get(&finding.target) {
                    let latest_obs = obs_list.iter().max_by_key(|o| o.checked_at);
                    if let Some(obs) = latest_obs {
                        let age = now.signed_duration_since(obs.checked_at);
                        let age_days = age.num_seconds() as u64 / (24 * 60 * 60);
                        age_days >= 7 // No recent observations (7+ days)
                    } else {
                        true // No observations at all
                    }
                } else {
                    true // No observations at all
                }
            }
            _ => false, // Don't auto-resolve other check types
        };

        if should_auto_resolve {
            let event = FindingEvent::resolve(&finding.id);
            storage.save(&event)?;
        }
    }

    Ok(())
}

/// Get open findings as issues (for display in check command)
pub fn get_open_findings_as_issues(
    storage: &Arc<dyn StorageEngine>,
    _ignore_patterns: &[String],
) -> Result<Vec<Issue>> {
    let records = storage.read_all_records()?;

    // Get all open findings
    let open_findings: Vec<&Finding> = records
        .findings
        .iter()
        .filter(|f| {
            // Check if finding is resolved
            !records
                .finding_events
                .iter()
                .any(|e| e.finding_id == f.id && e.event_type == FindingEventType::Resolved)
        })
        .collect();

    // Convert to issues
    let issues: Vec<Issue> = open_findings.iter().map(|f| Issue::from(*f)).collect();

    Ok(issues)
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

    // Load junk assets for filtering
    let junk_assets = crate::app::tags::get_junk_assets(&records.asset_tags);

    // Filter out ignored and junk items
    let mut ignored_count = 0;
    let mut junk_count = 0;

    let filtered_items: Vec<InventoryItem> = items
        .into_iter()
        .filter(|item| {
            let should_ignore = crate::ignore::is_ignored(&item.name, ignore_patterns);
            let is_junk = crate::ignore::is_junk(&item.name, &junk_assets);
            if should_ignore {
                ignored_count += 1;
            }
            if is_junk {
                junk_count += 1;
            }
            !should_ignore && !is_junk
        })
        .collect();

    // If we filtered out anything, report it
    if ignored_count > 0 || junk_count > 0 {
        println!(
            "Filtering: {} ignored by pattern, {} junk assets excluded",
            ignored_count, junk_count
        );
    }

    let filtered_http_observations: Vec<HttpObservation> = http_observations
        .into_iter()
        .filter(|obs| {
            !crate::ignore::is_ignored(&obs.target, ignore_patterns)
                && !crate::ignore::is_junk(&obs.target, &junk_assets)
        })
        .collect();

    // Run checks via Finding-based internals, then convert to Issue
    let findings = run_all_checks(
        &filtered_items,
        &filtered_http_observations,
        &records.lifecycle_events,
    );
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

#[cfg(test)]
fn check_low_confidence(items: &[InventoryItem]) -> Vec<Issue> {
    check_low_confidence_findings(items)
        .iter()
        .map(Issue::from)
        .collect()
}

#[cfg(test)]
fn check_unknown_sources(items: &[InventoryItem]) -> Vec<Issue> {
    check_unknown_sources_findings(items)
        .iter()
        .map(Issue::from)
        .collect()
}

#[cfg(test)]
fn check_invalid_data(items: &[InventoryItem]) -> Vec<Issue> {
    check_invalid_data_findings(items)
        .iter()
        .map(Issue::from)
        .collect()
}

#[cfg(test)]
fn check_subdomain_patterns(items: &[InventoryItem]) -> Vec<Issue> {
    check_subdomain_patterns_findings(items)
        .iter()
        .map(Issue::from)
        .collect()
}

// --- Lifecycle checks ---

/// Check for abandoned assets (stale + no observations + no owner)
fn check_abandoned_assets(
    items: &[InventoryItem],
    lifecycle_events: &[crate::inventory::LifecycleEvent],
    http_observations: &[HttpObservation],
) -> Vec<Finding> {
    use crate::inventory::{get_current_lifecycle, AssetLifecycle};

    let mut findings = Vec::new();
    let now = Utc::now();

    // Group items by domain
    let mut domain_items: HashMap<&String, Vec<&InventoryItem>> = HashMap::new();
    for item in items.iter().filter(|i| i.item_type == ItemType::Domain) {
        domain_items.entry(&item.name).or_default().push(item);
    }

    // Group HTTP observations by domain
    let mut domain_http_obs: HashMap<&String, Vec<&HttpObservation>> = HashMap::new();
    for obs in http_observations {
        domain_http_obs.entry(&obs.target).or_default().push(obs);
    }

    for (domain, domain_item_list) in domain_items {
        // Skip if already marked as abandoned
        let current_lifecycle = get_current_lifecycle(lifecycle_events, domain);
        if current_lifecycle == Some(AssetLifecycle::Abandoned) {
            continue;
        }

        // Check staleness (90+ days)
        let latest_item = domain_item_list.iter().max_by_key(|i| i.discovered_at);
        let is_stale = if let Some(item) = latest_item {
            let age = now.signed_duration_since(item.discovered_at);
            let age_days = age.num_seconds() as u64 / (24 * 60 * 60);
            age_days >= 90
        } else {
            false
        };

        if !is_stale {
            continue;
        }

        // Check for recent HTTP observations (within 30 days)
        let has_recent_obs = if let Some(obs_list) = domain_http_obs.get(domain) {
            let latest_obs = obs_list.iter().max_by_key(|o| o.checked_at);
            if let Some(obs) = latest_obs {
                let age = now.signed_duration_since(obs.checked_at);
                let age_days = age.num_seconds() as u64 / (24 * 60 * 60);
                age_days < 30
            } else {
                false
            }
        } else {
            false
        };

        if has_recent_obs {
            continue;
        }

        // Asset is stale with no recent observations - likely abandoned
        let message = format!(
            "Asset '{}' appears abandoned: not seen in 90+ days, no recent HTTP checks",
            domain
        );
        findings.push(Finding::new(
            FindingSeverity::High,
            FindingCategory::Lifecycle,
            "abandoned_asset",
            domain,
            &message,
        ));
    }

    findings
}

/// Check for decommissioned assets that are still being observed
fn check_decommissioned_still_present(
    items: &[InventoryItem],
    lifecycle_events: &[crate::inventory::LifecycleEvent],
    http_observations: &[HttpObservation],
) -> Vec<Finding> {
    use crate::inventory::{get_current_lifecycle, AssetLifecycle};

    let mut findings = Vec::new();
    let now = Utc::now();

    for item in items.iter().filter(|i| i.item_type == ItemType::Domain) {
        let current_lifecycle = get_current_lifecycle(lifecycle_events, &item.name);

        // Only check assets marked as decommissioned
        if current_lifecycle != Some(AssetLifecycle::Decommissioned) {
            continue;
        }

        // Check if there are recent HTTP observations
        let recent_obs: Vec<&HttpObservation> = http_observations
            .iter()
            .filter(|o| o.target == item.name)
            .filter(|o| {
                let age = now.signed_duration_since(o.checked_at);
                let age_days = age.num_seconds() as u64 / (24 * 60 * 60);
                age_days < 7 // Recent = within 7 days
            })
            .collect();

        if !recent_obs.is_empty() {
            let message = format!(
                "Decommissioned asset '{}' is still responding ({} recent observations)",
                item.name,
                recent_obs.len()
            );
            findings.push(Finding::new(
                FindingSeverity::Medium,
                FindingCategory::Operational,
                "decommissioned_still_present",
                &item.name,
                &message,
            ));
        }
    }

    findings
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
