mod app;
mod cli;
mod domain;
mod error;
mod exit_codes;
mod hosting;
mod ignore;
mod inventory;
mod org;
mod storage;
mod tui;

use exit_codes::*;
use ignore::load_ignore_patterns;
use std::process;

use crate::app::check::get_open_findings_as_issues;
use crate::app::finding::{FindingCategory, FindingSeverity};
use crate::app::probe::HttpErrorKind;
use crate::app::{
    batch_process, check_inventory, display_domain_status, display_domain_tree, export_status_csv,
    export_status_html, export_status_markdown, generate_report, get_domain_stats,
    get_inventory_diff, http_check, list_domains, probe_subdomains, scan_domain, Severity,
};
use crate::storage::{JsonlStorage, StorageEngine, StorageEngineExt};
use cli::{
    parse_command, Command, FindingCommand, FindingSeverityFilter, FindingStatus, OrgCommand,
    ProbeKind, TagCommand,
};
use std::sync::Arc;

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  balebos scan <domain>              - Scan a domain and add to inventory");
    eprintln!(
        "  balebos batch <file>               - Batch process domains from file (full recon)"
    );
    eprintln!("  balebos list                       - List all domains in inventory");
    eprintln!("  balebos list --unique              - List unique domains in inventory");
    eprintln!("  balebos tree                       - Show domain hierarchy with subdomains");
    eprintln!("  balebos status                     - Show domain live status with HTTP codes");
    eprintln!("  balebos stats                      - Show basic domain statistics");
    eprintln!("  balebos stats --by-domain          - Show detailed statistics per domain");
    eprintln!("  balebos diff                       - Show domain changes (last 7 days)");
    eprintln!("  balebos check                      - Check for inventory issues and anomalies");
    eprintln!("  balebos report                     - Generate consolidated status report");
    eprintln!("  balebos ui                         - Launch interactive TUI dashboard");
    eprintln!("  balebos probe subdomains <domain>  - Discover subdomains using subfinder");
    eprintln!("  balebos probe http <domain>        - Check HTTP status of a domain");
    eprintln!();
    eprintln!("Finding management:");
    eprintln!("  balebos finding run                - Run checks and persist findings");
    eprintln!("  balebos finding list               - List findings");
    eprintln!("  balebos finding list --status open - List open findings");
    eprintln!("  balebos finding list --severity high - List high severity findings");
    eprintln!("  balebos finding show <id>          - Show finding details");
    eprintln!("  balebos finding acknowledge <id>   - Acknowledge a finding");
    eprintln!("  balebos finding resolve <id>       - Resolve a finding");
    eprintln!("  balebos finding stats              - Show findings statistics");
    eprintln!("  balebos finding cleanup [--days N] - Cleanup old findings (default: 90 days)");
    eprintln!();
    eprintln!("Tag management:");
    eprintln!("  balebos tag add <asset> <tag>      - Add a tag to an asset");
    eprintln!("  balebos tag add-bulk <tag> <asset1> <asset2> ... - Add tag to multiple assets");
    eprintln!("  balebos tag remove <asset> <tag>   - Remove a tag from an asset");
    eprintln!("  balebos tag list [asset]           - List tags (all or for specific asset)");
    eprintln!("  balebos tag list-tags              - List all unique tag names");
    eprintln!("  balebos tag by-tag <tag>           - List all assets with a specific tag");
    eprintln!();
    eprintln!("Organizational management:");
    eprintln!("  balebos org person add <first> <last> <email>  - Add a person");
    eprintln!("  balebos org person list                        - List all persons");
    eprintln!(
        "  balebos org company add <name> <country>       - Add a company (ISO 3166-1 alpha-2)"
    );
    eprintln!("  balebos org company list                       - List all companies");
    eprintln!("  balebos org brand add <name>                   - Add a brand");
    eprintln!("  balebos org brand list                         - List all brands");
    eprintln!("  balebos org project add <name> --brand <id> --company <id>");
    eprintln!("                                                 - Add a project");
    eprintln!("  balebos org project list                       - List all projects");
    eprintln!("  balebos org assign asset <host> --project <id> - Assign asset to project");
    eprintln!("  balebos org assign person <id> --project <id> --role <role>");
    eprintln!("                                                 - Assign person to project");
    eprintln!("  balebos org assign company <id> --project <id> --role <role>");
    eprintln!("                                                 - Assign company to project");
    eprintln!("  balebos org unassign <host>                    - Unassign asset from project");
    eprintln!();
    eprintln!("Asset lifecycle management:");
    eprintln!("  balebos asset lifecycle set <asset> <state> [--reason <text>]");
    eprintln!("                                     - Set asset lifecycle state");
    eprintln!("                                     - States: active, under_review, deprecated, decommissioned, abandoned");
    eprintln!("  balebos asset lifecycle show <asset>           - Show asset lifecycle info");
    eprintln!("  balebos asset lifecycle history <asset>        - Show lifecycle history");
    eprintln!("  balebos asset lifecycle list                   - List all assets by lifecycle state");
}

fn main() {
    let storage: Arc<dyn StorageEngine> = {
        #[cfg(feature = "duckdb")]
        {
            let use_duckdb = std::env::var("BALEBOS_STORAGE").unwrap_or_default() == "duckdb";
            if use_duckdb {
                Arc::new(
                    crate::storage::duckdb::DuckDbStorage::new("data/inventory.duckdb").unwrap(),
                )
            } else {
                Arc::new(JsonlStorage::new("data/inventory.jsonl"))
            }
        }
        #[cfg(not(feature = "duckdb"))]
        {
            Arc::new(JsonlStorage::new("data/inventory.jsonl"))
        }
    };
    let ignore_patterns = load_ignore_patterns("balebos.ignore");

    let command = match parse_command() {
        Some(cmd) => cmd,
        None => {
            print_usage();
            process::exit(EXIT_INVALID_ARGUMENT);
        }
    };

    match command {
        Command::Scan { domain } => cmd_scan(&domain, &storage),
        Command::Batch { file } => cmd_batch(&file, &storage, &ignore_patterns),
        Command::List { unique } => cmd_list(unique, &storage),
        Command::Tree => cmd_tree(&storage),
        Command::Status { export } => cmd_status(export, &storage),
        Command::Stats { by_domain } => cmd_stats(by_domain, &storage),
        Command::Diff => cmd_diff(&storage),
        Command::Check => cmd_check(&storage, &ignore_patterns),
        Command::Report => cmd_report(&storage, &ignore_patterns),
        Command::Probe { kind, target } => cmd_probe(kind, &target, &storage),
        Command::Ui => cmd_ui(storage),
        Command::Org(org_cmd) => cmd_org(org_cmd, &storage),
        Command::Tag(tag_cmd) => cmd_tag(tag_cmd, &storage),
        Command::Finding(finding_cmd) => cmd_finding(finding_cmd, &storage, &ignore_patterns),
        Command::Asset(asset_cmd) => cmd_asset(asset_cmd, &storage),
    }
}

fn cmd_finding(
    finding_cmd: FindingCommand,
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
) {
    use crate::app::finding::{Finding, FindingEvent, FindingEventType};

    match finding_cmd {
        FindingCommand::Run => {
            match crate::app::check::run_checks_and_persist(storage, ignore_patterns) {
                Ok(findings) => {
                    if findings.is_empty() {
                        println!("No new findings detected.");
                        process::exit(EXIT_SUCCESS);
                    }

                    println!("Found {} new findings:", findings.len());
                    println!();

                    for finding in &findings {
                        println!(
                            "[{}] {} - {}",
                            finding.severity, finding.check_name, finding.target
                        );
                        println!("  {}", finding.message);
                        println!("  ID: {}", finding.id);
                        println!();
                    }

                    println!("All findings have been persisted to storage.");
                    println!("Use 'balebos finding list' to view all findings.");
                    println!("Use 'balebos finding acknowledge <id>' or 'balebos finding resolve <id>' to manage findings.");

                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error running checks: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        FindingCommand::List { status, severity } => {
            match storage.read_all_records() {
                Ok(records) => {
                    let findings = records.findings;
                    let events = records.finding_events;

                    // Filter by status
                    let filtered_by_status: Vec<&Finding> = match status {
                        Some(FindingStatus::Open) => {
                            let resolved_ids: std::collections::HashSet<&String> = events
                                .iter()
                                .filter(|e| e.event_type == FindingEventType::Resolved)
                                .map(|e| &e.finding_id)
                                .collect();
                            findings
                                .iter()
                                .filter(|f| !resolved_ids.contains(&f.id))
                                .collect()
                        }
                        Some(FindingStatus::Acknowledged) => {
                            let acknowledged_ids: std::collections::HashSet<&String> = events
                                .iter()
                                .filter(|e| e.event_type == FindingEventType::Acknowledged)
                                .map(|e| &e.finding_id)
                                .collect();
                            findings
                                .iter()
                                .filter(|f| acknowledged_ids.contains(&f.id))
                                .collect()
                        }
                        Some(FindingStatus::Resolved) => {
                            let resolved_ids: std::collections::HashSet<&String> = events
                                .iter()
                                .filter(|e| e.event_type == FindingEventType::Resolved)
                                .map(|e| &e.finding_id)
                                .collect();
                            findings
                                .iter()
                                .filter(|f| resolved_ids.contains(&f.id))
                                .collect()
                        }
                        None => findings.iter().collect(),
                    };

                    // Filter by severity
                    let filtered: Vec<&Finding> = match severity {
                        Some(FindingSeverityFilter::Info) => filtered_by_status
                            .iter()
                            .filter(|f| matches!(f.severity, FindingSeverity::Info))
                            .copied()
                            .collect(),
                        Some(FindingSeverityFilter::Low) => filtered_by_status
                            .iter()
                            .filter(|f| matches!(f.severity, FindingSeverity::Low))
                            .copied()
                            .collect(),
                        Some(FindingSeverityFilter::Medium) => filtered_by_status
                            .iter()
                            .filter(|f| matches!(f.severity, FindingSeverity::Medium))
                            .copied()
                            .collect(),
                        Some(FindingSeverityFilter::High) => filtered_by_status
                            .iter()
                            .filter(|f| matches!(f.severity, FindingSeverity::High))
                            .copied()
                            .collect(),
                        Some(FindingSeverityFilter::Critical) => filtered_by_status
                            .iter()
                            .filter(|f| matches!(f.severity, FindingSeverity::Critical))
                            .copied()
                            .collect(),
                        None => filtered_by_status,
                    };

                    if filtered.is_empty() {
                        println!("No findings found.");
                        process::exit(EXIT_SUCCESS);
                    }

                    println!("Findings ({}):", filtered.len());
                    println!();

                    for finding in filtered {
                        // Determine status
                        let is_resolved = events.iter().any(|e| {
                            e.finding_id == finding.id && e.event_type == FindingEventType::Resolved
                        });
                        let is_acknowledged = events.iter().any(|e| {
                            e.finding_id == finding.id
                                && e.event_type == FindingEventType::Acknowledged
                        });

                        let status_str = if is_resolved {
                            "[RESOLVED]"
                        } else if is_acknowledged {
                            "[ACKNOWLEDGED]"
                        } else {
                            "[OPEN]"
                        };

                        println!(
                            "{} [{}] {} - {}",
                            status_str, finding.severity, finding.check_name, finding.target
                        );
                        println!("  {}", finding.message);
                        println!("  ID: {}", finding.id);
                        println!("  Created: {}", finding.created_at.format("%Y-%m-%d %H:%M"));
                        println!();
                    }

                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error reading findings: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        FindingCommand::Show { id } => match storage.read_all_records() {
            Ok(records) => {
                let finding = records.findings.iter().find(|f| f.id == id);

                match finding {
                    Some(f) => {
                        let events: Vec<&FindingEvent> = records
                            .finding_events
                            .iter()
                            .filter(|e| e.finding_id == id)
                            .collect();

                        println!("Finding Details");
                        println!("================");
                        println!("ID: {}", f.id);
                        println!("Severity: {}", f.severity);
                        println!("Category: {}", f.category);
                        println!("Check: {}", f.check_name);
                        println!("Target: {}", f.target);
                        println!("Message: {}", f.message);
                        println!("Created: {}", f.created_at.format("%Y-%m-%d %H:%M:%S"));
                        println!();

                        if events.is_empty() {
                            println!("Status: OPEN (no events)");
                        } else {
                            println!("Events ({}):", events.len());
                            for event in events {
                                println!(
                                    "  - {} at {}",
                                    event.event_type,
                                    event.created_at.format("%Y-%m-%d %H:%M:%S")
                                );
                            }
                        }

                        process::exit(EXIT_SUCCESS);
                    }
                    None => {
                        eprintln!("Finding with ID '{}' not found.", id);
                        process::exit(EXIT_NOT_FOUND);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading findings: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        FindingCommand::Acknowledge { id } => match storage.read_all_records() {
            Ok(records) => {
                let finding_exists = records.findings.iter().any(|f| f.id == id);
                if !finding_exists {
                    eprintln!("Finding with ID '{}' not found.", id);
                    process::exit(EXIT_NOT_FOUND);
                }

                let event = FindingEvent::acknowledge(&id);
                match storage.save(&event) {
                    Ok(()) => {
                        println!("Finding {} acknowledged.", id);
                        process::exit(EXIT_SUCCESS);
                    }
                    Err(e) => {
                        eprintln!("Error acknowledging finding: {}", e);
                        process::exit(EXIT_STORAGE_ERROR);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading findings: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        FindingCommand::Resolve { id } => match storage.read_all_records() {
            Ok(records) => {
                let finding_exists = records.findings.iter().any(|f| f.id == id);
                if !finding_exists {
                    eprintln!("Finding with ID '{}' not found.", id);
                    process::exit(EXIT_NOT_FOUND);
                }

                let event = FindingEvent::resolve(&id);
                match storage.save(&event) {
                    Ok(()) => {
                        println!("Finding {} resolved.", id);
                        process::exit(EXIT_SUCCESS);
                    }
                    Err(e) => {
                        eprintln!("Error resolving finding: {}", e);
                        process::exit(EXIT_STORAGE_ERROR);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading findings: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        FindingCommand::Stats => match storage.read_all_records() {
            Ok(records) => {
                let total_findings = records.findings.len();

                let open_findings: Vec<&Finding> = records
                    .findings
                    .iter()
                    .filter(|f| {
                        !records.finding_events.iter().any(|e| {
                            e.finding_id == f.id && e.event_type == FindingEventType::Resolved
                        })
                    })
                    .collect();

                let acknowledged_findings = records
                    .finding_events
                    .iter()
                    .filter(|e| e.event_type == FindingEventType::Acknowledged)
                    .count();

                let resolved_findings = records
                    .finding_events
                    .iter()
                    .filter(|e| e.event_type == FindingEventType::Resolved)
                    .count();

                // Group by severity
                let mut by_severity: std::collections::HashMap<FindingSeverity, usize> =
                    std::collections::HashMap::new();
                for finding in &open_findings {
                    *by_severity.entry(finding.severity).or_insert(0) += 1;
                }

                // Group by check name
                let mut by_check: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                for finding in &open_findings {
                    *by_check.entry(finding.check_name.clone()).or_insert(0) += 1;
                }

                // Group by category
                let mut by_category: std::collections::HashMap<FindingCategory, usize> =
                    std::collections::HashMap::new();
                for finding in &open_findings {
                    *by_category.entry(finding.category).or_insert(0) += 1;
                }

                println!("Findings Statistics");
                println!("===================");
                println!();

                println!("Summary:");
                println!("- Total findings: {}", total_findings);
                println!("- Open findings: {}", open_findings.len());
                println!("- Acknowledged: {}", acknowledged_findings);
                println!("- Resolved: {}", resolved_findings);

                if total_findings > 0 {
                    let resolution_rate =
                        (resolved_findings as f64 / total_findings as f64) * 100.0;
                    println!("- Resolution rate: {:.1}%", resolution_rate);
                }
                println!();

                if !open_findings.is_empty() {
                    println!("Open Findings by Severity:");
                    let mut severities: Vec<_> = by_severity.iter().collect();
                    severities.sort_by_key(|(severity, _)| match severity {
                        FindingSeverity::Critical => 0,
                        FindingSeverity::High => 1,
                        FindingSeverity::Medium => 2,
                        FindingSeverity::Low => 3,
                        FindingSeverity::Info => 4,
                    });

                    for (severity, count) in severities {
                        println!("  - {}: {}", severity, count);
                    }
                    println!();

                    println!("Open Findings by Check:");
                    let mut checks: Vec<_> = by_check.iter().collect();
                    checks.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending

                    for (check, count) in checks {
                        println!("  - {}: {}", check, count);
                    }
                    println!();

                    println!("Open Findings by Category:");
                    let mut categories: Vec<_> = by_category.iter().collect();
                    categories.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending

                    for (category, count) in categories {
                        println!("  - {}: {}", category, count);
                    }
                }

                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error reading findings: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        FindingCommand::Cleanup { days } => {
            let max_age_days = days.unwrap_or(90); // Default 90 days retention

            println!("Cleaning up findings older than {} days...", max_age_days);

            match storage.cleanup_old_records(max_age_days) {
                Ok(()) => {
                    println!("Cleanup completed successfully.");
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error during cleanup: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
    }
}

fn cmd_ui(storage: Arc<dyn StorageEngine>) {
    if let Err(e) = tui::run(storage) {
        eprintln!("TUI error: {}", e);
        process::exit(EXIT_GENERAL_ERROR);
    }
    process::exit(EXIT_SUCCESS);
}

fn cmd_scan(domain_name: &str, storage: &Arc<dyn StorageEngine>) {
    match scan_domain(domain_name, storage) {
        Ok(domain) => {
            println!("Domain scanned successfully: {}", domain.name);
            println!("Domain ID: {:?}", domain.id);
            println!("Discovered at: {}", domain.discovered_at);

            if domain_name != domain.name {
                println!("Normalized from: {}", domain_name);
            }
            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error scanning domain: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    }
}

fn cmd_batch(file: &str, storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
    match batch_process(file, storage, ignore_patterns) {
        Ok(_) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error during batch processing: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    }
}

fn cmd_list(unique: bool, storage: &Arc<dyn StorageEngine>) {
    match list_domains(storage, unique) {
        Ok(domains) => {
            if domains.is_empty() {
                println!("No domains found in inventory.");
            } else {
                let label = if unique { "Unique" } else { "All" };
                println!("{} domains in inventory ({}):", label, domains.len());
                for domain in domains {
                    println!("- {}", domain);
                }
            }
            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error listing domains: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_tree(storage: &Arc<dyn StorageEngine>) {
    match display_domain_tree(storage) {
        Ok(()) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error displaying domain tree: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_status(export: Option<cli::ExportArgs>, storage: &Arc<dyn StorageEngine>) {
    if let Some(export_args) = export {
        let output_file = export_args.output_file.unwrap_or_else(|| {
            let ext = match export_args.format.as_str() {
                "csv" => "csv",
                "markdown" | "md" => "md",
                "html" => "html",
                _ => {
                    eprintln!("Unknown format: {}", export_args.format);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            format!("domain-status-report.{}", ext)
        });

        let result = match export_args.format.as_str() {
            "csv" => export_status_csv(storage, &output_file),
            "markdown" | "md" => export_status_markdown(storage, &output_file),
            "html" => export_status_html(storage, &output_file),
            _ => {
                eprintln!("Unknown format: {}", export_args.format);
                process::exit(EXIT_INVALID_ARGUMENT);
            }
        };

        match result {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error exporting status report: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        }
    }

    match display_domain_status(storage) {
        Ok(()) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error displaying domain status: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_stats(by_domain: bool, storage: &Arc<dyn StorageEngine>) {
    match get_domain_stats(storage) {
        Ok(stats) => {
            if by_domain {
                println!("Domain Statistics By Domain");
                println!("---------------------------");
            } else {
                println!("Domain Statistics");
                println!("----------------");
            }

            println!("Total domain entries: {}", stats.total_entries);
            println!("Unique domains: {}", stats.unique_domains);

            if by_domain {
                if !stats.top_domains.is_empty() {
                    println!("\nTop Domains:");
                    for (domain, count) in stats.top_domains {
                        println!("- {} ({} entries)", domain, count);
                    }
                } else {
                    println!("\nNo domains found in inventory.");
                }
            } else if let Some(latest) = stats.latest_domain {
                println!("Latest domain: {}", latest);
                if let Some(timestamp) = stats.latest_timestamp {
                    println!("Added on: {}", timestamp);
                }
            } else {
                println!("No domains found in inventory.");
            }

            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error getting domain statistics: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_diff(storage: &Arc<dyn StorageEngine>) {
    match get_inventory_diff(storage, None) {
        Ok(diff) => {
            println!("Domain Changes (last 7 days)");
            println!("---------------------------");

            if !diff.new_domains.is_empty() {
                println!("\nNew domains:");
                for domain in &diff.new_domains {
                    println!("+ {}", domain);
                }
            }

            if !diff.missing_domains.is_empty() {
                println!("\nMissing domains (not seen recently):");
                for domain in &diff.missing_domains {
                    println!("- {}", domain);
                }
            }

            if diff.new_domains.is_empty() && diff.missing_domains.is_empty() {
                println!("No changes detected.");
            }

            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error getting inventory diff: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_check(storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
    // Get current issues from check_inventory (real-time checks)
    match check_inventory(storage, ignore_patterns) {
        Ok(current_issues) => {
            // Get open findings from storage
            let open_findings_issues_result = get_open_findings_as_issues(storage, ignore_patterns);

            // Combine current issues with open findings
            let mut all_issues = current_issues;
            let open_findings_count = match &open_findings_issues_result {
                Ok(issues) => {
                    all_issues.extend(issues.clone());
                    issues.len()
                }
                Err(e) => {
                    eprintln!("Error reading open findings: {}", e);
                    0
                }
            };

            // Remove duplicates (same message)
            let mut unique_issues = Vec::new();
            let mut seen_messages = std::collections::HashSet::new();

            for issue in all_issues {
                if !seen_messages.contains(&issue.message) {
                    seen_messages.insert(issue.message.clone());
                    unique_issues.push(issue);
                }
            }

            if unique_issues.is_empty() {
                println!("No issues found in inventory.");
                println!("Note: Run 'balebos finding run' to persist findings for tracking.");
                process::exit(EXIT_SUCCESS);
            }

            let critical_count = unique_issues
                .iter()
                .filter(|i| i.severity == Severity::Critical)
                .count();
            let warning_count = unique_issues
                .iter()
                .filter(|i| i.severity == Severity::Warning)
                .count();

            println!("Inventory Check Results");
            println!("----------------------");
            println!(
                "Found {} issues ({} critical, {} warnings)",
                unique_issues.len(),
                critical_count,
                warning_count
            );

            // Show note about findings if there are open findings
            if open_findings_count > 0 {
                println!(
                    "(Includes {} open findings from previous checks)",
                    open_findings_count
                );
                println!(
                    "Use 'balebos finding list' to see details or 'balebos finding run' to update."
                );
            }

            if critical_count > 0 {
                println!("\nCRITICAL ISSUES:");
                for issue in unique_issues
                    .iter()
                    .filter(|i| i.severity == Severity::Critical)
                {
                    println!("! {}", issue.message);
                }
            }

            if warning_count > 0 {
                println!("\nWARNINGS:");
                for issue in unique_issues
                    .iter()
                    .filter(|i| i.severity == Severity::Warning)
                {
                    println!("- {}", issue.message);
                }
            }

            println!("\nManagement commands:");
            println!("- 'balebos finding run' - Run checks and persist findings");
            println!("- 'balebos finding list' - List all open findings");
            println!("- 'balebos finding acknowledge <id>' - Acknowledge a finding");
            println!("- 'balebos finding resolve <id>' - Resolve a finding");

            if critical_count > 0 {
                process::exit(EXIT_CRITICAL_ISSUES);
            } else if warning_count > 0 {
                process::exit(EXIT_WARNING_ISSUES);
            }
            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error checking inventory: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_report(storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
    match generate_report(storage, ignore_patterns) {
        Ok(report) => {
            println!("Balebos Report (last 7 days)");
            println!("----------------------------");
            println!();

            println!("Inventory:");
            println!("- total domains: {}", report.total_domains);
            println!("- unique domains: {}", report.unique_domains);
            println!();

            println!("Changes:");
            print_count_line("+", "new domain", report.new_domains);
            print_count_line("-", "missing domain", report.missing_domains);

            if report.new_subdomains > 0 {
                print_count_line("+", "new subdomain", report.new_subdomains);
            }
            if report.missing_subdomains > 0 {
                print_count_line("-", "missing subdomain", report.missing_subdomains);
            }
            println!();

            println!("Issues:");
            let total_issues = report.critical_issues + report.warning_issues;

            if total_issues == 0 {
                println!("- no issues detected");
            } else {
                print_issue_line("stale domain", report.stale_domains, " (>14 days)");
                print_issue_line("low confidence domain", report.low_confidence_domains, "");
                print_issue_line("unknown source", report.unknown_sources, "");
                print_issue_line("normalization issue", report.invalid_data, "");
                print_issue_line(
                    "suspicious subdomain pattern",
                    report.suspicious_subdomains,
                    "",
                );
                print_issue_line("HTTP status error", report.http_errors, "");
                print_issue_line(
                    "domain without recent HTTP checks",
                    report.missing_http_checks,
                    "",
                );
            }

            println!("\nFindings:");
            if report.open_findings == 0
                && report.acknowledged_findings == 0
                && report.resolved_findings == 0
            {
                println!("- no findings tracked");
            } else {
                print_issue_line("open findings", report.open_findings, "");
                print_issue_line("acknowledged findings", report.acknowledged_findings, "");
                print_issue_line("resolved findings", report.resolved_findings, "");
            }
            println!();

            println!("Summary:");
            if report.needs_attention {
                println!("⚠️ attention required");
            } else {
                println!("✅ all systems normal");
            }
            println!();

            println!("Run specific commands for details:");
            println!("- 'balebos diff' for change details");
            println!("- 'balebos check' for issue details");

            if report.needs_attention {
                process::exit(EXIT_WARNING_ISSUES);
            }
            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error generating report: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_probe(kind: ProbeKind, target: &str, storage: &Arc<dyn StorageEngine>) {
    match kind {
        ProbeKind::Subdomains => match probe_subdomains(target, storage) {
            Ok(subdomains) => {
                if subdomains.is_empty() {
                    println!("No subdomains found for {}", target);
                } else {
                    println!("Found {} subdomains for {}:", subdomains.len(), target);
                    for subdomain in &subdomains {
                        println!("- {}", subdomain);
                    }
                    println!("\nAll subdomains added to inventory.");
                    println!(
                        "Run 'balebos diff' to see changes or 'balebos check' to detect anomalies."
                    );
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error probing subdomains: {}", e);
                eprintln!(
                    "\nMake sure subfinder is installed: https://github.com/projectdiscovery/subfinder"
                );
                process::exit(EXIT_EXTERNAL_TOOL_ERROR);
            }
        },
        ProbeKind::Http => match http_check(target, storage) {
            Ok(observation) => {
                println!("HTTP check completed for {}", target);

                if let Some(status) = observation.status {
                    println!("Status: {}", status);
                } else if let Some(error_kind) = observation.error_kind {
                    println!("Probe failed: {:?}", error_kind);
                    println!(
                        "Error: {}",
                        observation.error_message.as_deref().unwrap_or("unknown")
                    );
                }

                if !observation.redirect_chain.is_empty() {
                    println!(
                        "\nRedirect chain ({} hops):",
                        observation.redirect_chain.len()
                    );
                    for (i, hop) in observation.redirect_chain.iter().enumerate() {
                        println!(
                            "  {}. {} ({}) -> {}",
                            i + 1,
                            hop.url,
                            hop.status,
                            hop.redirect_to
                        );
                    }
                    if let Some(final_url) = &observation.final_url {
                        println!("  Final URL: {}", final_url);
                    }
                }

                println!("\nDetails:");
                if let Some(title) = &observation.title {
                    println!("  Title: {}", title);
                }
                if let Some(server) = &observation.webserver {
                    println!("  Server: {}", server);
                }
                if !observation.tech.is_empty() {
                    println!("  Tech: {}", observation.tech.join(", "));
                }

                println!("Response time: {}ms", observation.response_time_ms);

                println!("\nObservation added to inventory.");
                println!(
                    "Run 'balebos check' to detect anomalies or 'balebos report' for a status summary."
                );

                if let Some(error_kind) = observation.error_kind {
                    match error_kind {
                        HttpErrorKind::DnsNotFound => process::exit(EXIT_DOMAIN_NOT_FOUND),
                        _ => process::exit(EXIT_PROBE_ERROR),
                    }
                } else if let Some(status) = observation.status {
                    if status >= 500 {
                        process::exit(EXIT_HTTP_SERVER_ERROR);
                    } else if status >= 400 {
                        process::exit(EXIT_HTTP_CLIENT_ERROR);
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error checking HTTP status: {}", e);
                process::exit(EXIT_HTTP_ERROR);
            }
        },
        ProbeKind::Dns => match crate::app::probe::probe_dns(target, storage) {
            Ok(obs) => {
                println!("DNS check completed for {}", obs.target);

                if !obs.a.is_empty() {
                    println!("A: {}", obs.a.join(", "));
                }
                if !obs.aaaa.is_empty() {
                    println!("AAAA: {}", obs.aaaa.join(", "));
                }
                if !obs.cname.is_empty() {
                    println!("CNAME: {}", obs.cname.join(", "));
                }

                if obs.a.is_empty() && obs.aaaa.is_empty() && obs.cname.is_empty() {
                    println!("No DNS records found.");
                }

                println!("\nObservation added to inventory.");
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error checking DNS records: {}", e);
                process::exit(EXIT_EXTERNAL_TOOL_ERROR);
            }
        },
        ProbeKind::Asn => {
            // First run DNS probe to get IPs
            match crate::app::probe::probe_dns(target, storage) {
                Ok(dns_obs) => {
                    if dns_obs.a.is_empty() && dns_obs.aaaa.is_empty() {
                        eprintln!("No IP addresses found for {}. Run DNS probe first.", target);
                        process::exit(EXIT_DOMAIN_NOT_FOUND);
                    }

                    // Build IP to host map
                    let ip_map = crate::app::probe::asn::build_ip_host_map(&[dns_obs.clone()]);

                    match crate::app::probe::probe_asn_batch(&ip_map, storage) {
                        Ok(observations) => {
                            println!("ASN lookup completed for {}", target);

                            for obs in &observations {
                                println!(
                                    "  {} -> AS{} {} ({})",
                                    obs.target, obs.asn, obs.org, obs.country
                                );
                            }

                            if observations.is_empty() {
                                println!("No ASN information found.");
                            } else {
                                println!(
                                    "\n{} ASN records added to inventory.",
                                    observations.len()
                                );
                            }

                            process::exit(EXIT_SUCCESS);
                        }
                        Err(e) => {
                            eprintln!("Error looking up ASN: {}", e);
                            eprintln!("\nMake sure asnmap is installed: https://github.com/projectdiscovery/asnmap");
                            process::exit(EXIT_EXTERNAL_TOOL_ERROR);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error resolving DNS: {}", e);
                    process::exit(EXIT_EXTERNAL_TOOL_ERROR);
                }
            }
        }
        ProbeKind::Hosting => {
            use std::collections::HashMap;

            // Run DNS probe first
            let dns_obs = match crate::app::probe::probe_dns(target, storage) {
                Ok(obs) => Some(obs),
                Err(e) => {
                    eprintln!("Warning: DNS probe failed: {}", e);
                    None
                }
            };

            // Run HTTP probe
            let http_obs = match crate::app::probe::http_check(target, storage) {
                Ok(obs) => Some(obs),
                Err(e) => {
                    eprintln!("Warning: HTTP probe failed: {}", e);
                    None
                }
            };

            // Run ASN probe if we have IPs
            let mut asn_by_ip: HashMap<String, crate::app::probe::AsnObservation> = HashMap::new();
            if let Some(ref dns) = dns_obs {
                let ip_map = crate::app::probe::asn::build_ip_host_map(&[dns.clone()]);
                if !ip_map.is_empty() {
                    if let Ok(asn_obs) = crate::app::probe::probe_asn_batch(&ip_map, storage) {
                        for obs in asn_obs {
                            asn_by_ip.insert(obs.target.clone(), obs);
                        }
                    }
                }
            }

            // Run hosting detection
            match crate::hosting::detect_hosting(
                target,
                dns_obs.as_ref(),
                http_obs.as_ref(),
                &asn_by_ip,
                storage,
            ) {
                Ok(obs) => {
                    println!("Hosting detection completed for {}", target);
                    println!();

                    if let Some(provider) = &obs.provider_name {
                        let category = obs.provider_category.as_deref().unwrap_or("Unknown");
                        println!("Provider: {} ({})", provider, category);
                    } else {
                        println!("Provider: UNKNOWN");
                    }

                    if obs.cdn_detected {
                        let cdn = obs.cdn_name.as_deref().unwrap_or("Unknown");
                        println!("CDN:      {} (detected)", cdn);
                    }

                    println!(
                        "Confidence: {} (via {})",
                        obs.confidence, obs.detection_method
                    );

                    if !obs.evidence.is_empty() {
                        println!("\nEvidence:");
                        for ev in &obs.evidence {
                            println!("  - {}", ev);
                        }
                    }

                    println!("\nHosting observation added to inventory.");
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error detecting hosting: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
    }
}

/// Print a line like `+ 3 new domains` or `+ no new domains`.
fn print_count_line(prefix: &str, label: &str, count: usize) {
    if count > 0 {
        let suffix = if count == 1 { "" } else { "s" };
        println!("{} {} {}{}", prefix, count, label, suffix);
    } else {
        println!("{} no {}s", prefix, label);
    }
}

/// Print issue line only if count > 0.
fn print_issue_line(label: &str, count: usize, extra: &str) {
    if count > 0 {
        let suffix = if count == 1 { "" } else { "s" };
        println!("- {} {}{}{}", count, label, suffix, extra);
    }
}

fn cmd_org(org_cmd: OrgCommand, storage: &Arc<dyn StorageEngine>) {
    use app::org::*;
    use org::{BrandId, CompanyId, CountryCode, PersonId, ProjectId, Role};

    match org_cmd {
        OrgCommand::PersonAdd {
            first_name,
            last_name,
            email,
        } => match add_person(storage, &first_name, &last_name, &email, None) {
            Ok(rec) => {
                println!(
                    "Person added: {} {} <{}>",
                    rec.first_name, rec.last_name, rec.email
                );
                println!("ID: {}", rec.id);
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error adding person: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        OrgCommand::PersonList => match list_persons(storage) {
            Ok(persons) => {
                if persons.is_empty() {
                    println!("No persons found.");
                } else {
                    println!("Persons ({}):", persons.len());
                    for p in persons {
                        println!(
                            "  {} - {} {} <{}>",
                            p.id, p.first_name, p.last_name, p.email
                        );
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error listing persons: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        OrgCommand::CompanyAdd {
            name,
            country,
            brand_id,
        } => {
            let country_code = match CountryCode::for_alpha2(&country) {
                Ok(code) => code,
                Err(_) => {
                    eprintln!(
                        "Invalid country code: {}. Use ISO 3166-1 alpha-2 (e.g., DE, PL, US)",
                        country
                    );
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let brand = match brand_id {
                Some(id_str) => match id_str.parse::<BrandId>() {
                    Ok(id) => Some(id),
                    Err(_) => {
                        eprintln!("Invalid brand ID: {}", id_str);
                        process::exit(EXIT_INVALID_ARGUMENT);
                    }
                },
                None => None,
            };
            match add_company(storage, &name, country_code, brand) {
                Ok(rec) => {
                    println!("Company added: {} ({})", rec.name, rec.country.name());
                    println!("ID: {}", rec.id);
                    if rec.is_external() {
                        println!("Type: External contractor");
                    } else if let Some(brand_id) = rec.brand {
                        println!("Brand ID: {}", brand_id);
                    }
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error adding company: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::CompanyList => match list_companies(storage) {
            Ok(companies) => {
                if companies.is_empty() {
                    println!("No companies found.");
                } else {
                    println!("Companies ({}):", companies.len());
                    for c in companies {
                        let brand_info = if c.is_external() {
                            " [external]".to_string()
                        } else if let Some(brand_id) = c.brand {
                            format!(" [brand: {}]", brand_id)
                        } else {
                            String::new()
                        };
                        println!(
                            "  {} - {} ({}){}",
                            c.id,
                            c.name,
                            c.country.name(),
                            brand_info
                        );
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error listing companies: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        OrgCommand::BrandAdd { name } => match add_brand(storage, &name) {
            Ok(rec) => {
                println!("Brand added: {}", rec.name);
                println!("ID: {}", rec.id);
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error adding brand: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        OrgCommand::BrandList => match list_brands(storage) {
            Ok(brands) => {
                if brands.is_empty() {
                    println!("No brands found.");
                } else {
                    println!("Brands ({}):", brands.len());
                    for b in brands {
                        println!("  {} - {}", b.id, b.name);
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error listing brands: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        OrgCommand::ProjectAdd { name, company_id } => {
            let company = match company_id.parse::<CompanyId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid company ID: {}", company_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match add_project(storage, &name, company) {
                Ok(rec) => {
                    println!("Project added: {}", rec.name);
                    println!("ID: {}", rec.id);
                    println!("Company ID: {}", rec.company);
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error adding project: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::ProjectList => match list_projects(storage) {
            Ok(projects) => {
                if projects.is_empty() {
                    println!("No projects found.");
                } else {
                    println!("Projects ({}):", projects.len());
                    for p in projects {
                        println!("  {} - {} (company: {})", p.id, p.name, p.company);
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error listing projects: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        OrgCommand::AssignAsset { host, project_id } => {
            let project = match project_id.parse::<ProjectId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid project ID: {}", project_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match assign_asset(storage, &host, project) {
                Ok(rec) => {
                    println!("Asset assigned: {} -> project {}", rec.host, rec.project);
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error assigning asset: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::AssignPerson {
            person_id,
            project_id,
            role,
        } => {
            let person = match person_id.parse::<PersonId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid person ID: {}", person_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let project = match project_id.parse::<ProjectId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid project ID: {}", project_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let role_enum = match role.to_lowercase().as_str() {
                "owner" => Role::Owner,
                "manager" => Role::Manager,
                "technician" => Role::Technician,
                _ => {
                    eprintln!(
                        "Invalid role: {}. Valid roles: owner, manager, technician",
                        role
                    );
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match assign_person_to_project(storage, person, project, role_enum) {
                Ok(rec) => {
                    println!(
                        "Person {} assigned to project {} as {}",
                        person, rec.project, rec.role
                    );
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error assigning person: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::AssignCompany {
            company_id,
            project_id,
            role,
        } => {
            let company = match company_id.parse::<CompanyId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid company ID: {}", company_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let project = match project_id.parse::<ProjectId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid project ID: {}", project_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let role_enum = match role.to_lowercase().as_str() {
                "owner" => Role::Owner,
                "manager" => Role::Manager,
                "technician" => Role::Technician,
                _ => {
                    eprintln!(
                        "Invalid role: {}. Valid roles: owner, manager, technician",
                        role
                    );
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match assign_company_to_project(storage, company, project, role_enum) {
                Ok(rec) => {
                    println!(
                        "Company {} assigned to project {} as {}",
                        company, rec.project, rec.role
                    );
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error assigning company: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::Unassign { host } => match unassign_asset(storage, &host) {
            Ok(found) => {
                if found {
                    println!("Asset {} unassigned.", host);
                    println!("Note: The unassignment is recorded but the original assignment remains in the log.");
                } else {
                    println!("Asset {} was not assigned to any project.", host);
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error unassigning asset: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
    }
}

fn cmd_tag(tag_cmd: TagCommand, storage: &Arc<dyn StorageEngine>) {
    use app::tags::*;

    match tag_cmd {
        TagCommand::Add { asset_name, tag } => match add_tag(storage, &asset_name, &tag) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error adding tag: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        TagCommand::AddBulk { asset_names, tag } => {
            let mut success_count = 0;
            let mut error_count = 0;

            for asset_name in &asset_names {
                match add_tag(storage, asset_name, &tag) {
                    Ok(()) => {
                        println!("✓ Tagged '{}' with '{}'", asset_name, tag);
                        success_count += 1;
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to tag '{}': {}", asset_name, e);
                        error_count += 1;
                    }
                }
            }

            println!("\nBulk tagging complete:");
            println!("  Success: {}", success_count);
            if error_count > 0 {
                println!("  Failed:  {}", error_count);
            }

            if error_count > 0 {
                process::exit(EXIT_GENERAL_ERROR);
            } else {
                process::exit(EXIT_SUCCESS);
            }
        }
        TagCommand::Remove { asset_name, tag } => match remove_tag(storage, &asset_name, &tag) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error removing tag: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        TagCommand::List { asset_name } => match list_tags(storage, asset_name.as_deref()) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error listing tags: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        TagCommand::ListTags => match list_all_tag_names(storage) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error listing tags: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        TagCommand::ListByTag { tag } => match list_assets_by_tag(storage, &tag) {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error listing assets: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
    }
}

fn cmd_asset(asset_cmd: crate::cli::router::AssetCommand, storage: &Arc<dyn StorageEngine>) {
    use app::{
        display_lifecycle_info, display_lifecycle_summary, get_lifecycle_history, set_lifecycle,
    };
    use cli::router::AssetCommand;
    use inventory::AssetLifecycle;
    use std::process;

    match asset_cmd {
        AssetCommand::LifecycleSet {
            asset_name,
            state,
            reason,
        } => {
            let lifecycle_state = match AssetLifecycle::from_str(&state) {
                Some(s) => s,
                None => {
                    eprintln!("Invalid lifecycle state: {}", state);
                    eprintln!("Valid states: active, under_review, deprecated, decommissioned, abandoned");
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };

            match set_lifecycle(storage, &asset_name, lifecycle_state, reason.as_deref(), "cli") {
                Ok(event) => {
                    let from_str = event
                        .from_state
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "(none)".to_string());
                    println!(
                        "Lifecycle updated: {} → {}",
                        from_str, event.to_state
                    );
                    println!("Asset: {}", asset_name);
                    if let Some(ref r) = reason {
                        println!("Reason: {}", r);
                    }
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error setting lifecycle: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        AssetCommand::LifecycleShow { asset_name } => {
            match display_lifecycle_info(storage, &asset_name) {
                Ok(()) => process::exit(EXIT_SUCCESS),
                Err(e) => {
                    eprintln!("Error displaying lifecycle: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        AssetCommand::LifecycleHistory { asset_name } => {
            match get_lifecycle_history(storage, &asset_name) {
                Ok(history) => {
                    if history.is_empty() {
                        println!("No lifecycle history for '{}'", asset_name);
                    } else {
                        println!("Lifecycle history for '{}':", asset_name);
                        println!("{}", "-".repeat(60));
                        for event in history {
                            let from_str = event
                                .from_state
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "(initial)".to_string());
                            println!(
                                "[{}] {} → {}",
                                event.created_at.format("%Y-%m-%d %H:%M"),
                                from_str,
                                event.to_state
                            );
                            println!("  By: {}", event.changed_by);
                            if let Some(ref reason) = event.reason {
                                println!("  Reason: {}", reason);
                            }
                        }
                    }
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error getting lifecycle history: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
        AssetCommand::LifecycleList => {
            match display_lifecycle_summary(storage) {
                Ok(()) => process::exit(EXIT_SUCCESS),
                Err(e) => {
                    eprintln!("Error displaying lifecycle summary: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
        }
    }
}
