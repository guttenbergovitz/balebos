mod app;
mod cli;
mod domain;
mod exit_codes;
mod ignore;
mod inventory;
mod storage;

use exit_codes::*;
use ignore::load_ignore_patterns;
use std::process;

use app::probe_http::HttpErrorKind;
use app::{
    batch_process, check_inventory, display_domain_status, display_domain_tree, export_status_csv,
    export_status_html, export_status_markdown, generate_report, get_domain_stats,
    get_inventory_diff, http_check, list_domains, probe_subdomains, scan_domain, Severity,
};
use cli::{parse_command, Command, ProbeKind};
use storage::JsonlStorage;

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
    eprintln!("  balebos probe subdomains <domain>  - Discover subdomains using subfinder");
    eprintln!("  balebos probe http <domain>        - Check HTTP status of a domain");
}

fn main() {
    let storage = JsonlStorage::new("data/inventory.jsonl");
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
    }
}

fn cmd_scan(domain_name: &str, storage: &JsonlStorage) {
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

fn cmd_batch(file: &str, storage: &JsonlStorage, ignore_patterns: &[String]) {
    match batch_process(file, storage, ignore_patterns) {
        Ok(_) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error during batch processing: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    }
}

fn cmd_list(unique: bool, storage: &JsonlStorage) {
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

fn cmd_tree(storage: &JsonlStorage) {
    match display_domain_tree(storage) {
        Ok(()) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error displaying domain tree: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_status(export: Option<cli::ExportArgs>, storage: &JsonlStorage) {
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

fn cmd_stats(by_domain: bool, storage: &JsonlStorage) {
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

fn cmd_diff(storage: &JsonlStorage) {
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

fn cmd_check(storage: &JsonlStorage, ignore_patterns: &[String]) {
    match check_inventory(storage, ignore_patterns) {
        Ok(issues) => {
            if issues.is_empty() {
                println!("No issues found in inventory.");
                process::exit(EXIT_SUCCESS);
            }

            let critical_count = issues
                .iter()
                .filter(|i| i.severity == Severity::Critical)
                .count();
            let warning_count = issues
                .iter()
                .filter(|i| i.severity == Severity::Warning)
                .count();

            println!("Inventory Check Results");
            println!("----------------------");
            println!(
                "Found {} issues ({} critical, {} warnings)",
                issues.len(),
                critical_count,
                warning_count
            );

            if critical_count > 0 {
                println!("\nCRITICAL ISSUES:");
                for issue in issues.iter().filter(|i| i.severity == Severity::Critical) {
                    println!("! {}", issue.message);
                }
            }

            if warning_count > 0 {
                println!("\nWARNINGS:");
                for issue in issues.iter().filter(|i| i.severity == Severity::Warning) {
                    println!("- {}", issue.message);
                }
            }

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

fn cmd_report(storage: &JsonlStorage, ignore_patterns: &[String]) {
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

fn cmd_probe(kind: ProbeKind, target: &str, storage: &JsonlStorage) {
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
                    println!("\nRedirect chain ({} hops):", observation.redirect_chain.len());
                    for (i, hop) in observation.redirect_chain.iter().enumerate() {
                        println!("  {}. {} ({}) -> {}", i + 1, hop.url, hop.status, hop.redirect_to);
                    }
                    if let Some(final_url) = &observation.final_url {
                        println!("  Final URL: {}", final_url);
                    }
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
