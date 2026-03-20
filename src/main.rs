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
    batch_process, check_inventory, display_domain_status, display_domain_tree, generate_report,
    get_domain_stats, get_inventory_diff, http_check, list_domains, probe_subdomains, scan_domain,
    Severity,
};
use cli::list::ListOptions;
use cli::stats::StatsOptions;
use cli::{
    handle_check_command, handle_diff_command, handle_list_command, handle_probe_command,
    handle_report_command, handle_scan_command, handle_stats_command,
};
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
    // Create storage with the inventory.jsonl file
    let storage = JsonlStorage::new("data/inventory.jsonl");

    // Load ignore patterns from balebos.ignore (if present)
    let ignore_patterns = load_ignore_patterns("balebos.ignore");

    // Check if we're handling a 'scan' command
    if let Some(domain_name) = handle_scan_command() {
        // Execute domain scan and save to inventory
        match scan_domain(&domain_name, &storage) {
            Ok(domain) => {
                println!("Domain scanned successfully: {}", domain.name);
                println!("Domain ID: {:?}", domain.id);
                println!("Discovered at: {}", domain.discovered_at);

                // If the domain was normalized, show the original input
                if domain_name != domain.name {
                    println!("Normalized from: {}", domain_name);
                }
            }
            Err(e) => {
                eprintln!("Error scanning domain: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        }
        process::exit(EXIT_SUCCESS);
    }

    // Check if we're handling a 'batch' command
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "batch" {
        if args.len() < 3 {
            eprintln!("Usage: balebos batch <file>");
            process::exit(EXIT_INVALID_ARGUMENT);
        }

        match batch_process(&args[2], &storage, &ignore_patterns) {
            Ok(_stats) => {
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error during batch processing: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        }
    }

    // Check if we're handling a 'list' command
    match handle_list_command() {
        ListOptions::All => {
            // List all domains from inventory
            match list_domains(&storage, false) {
                Ok(domains) => {
                    if domains.is_empty() {
                        println!("No domains found in inventory.");
                    } else {
                        println!("All domains in inventory ({}):", domains.len());
                        for domain in domains {
                            println!("- {}", domain);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error listing domains: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
            process::exit(EXIT_SUCCESS);
        }
        ListOptions::Unique => {
            // List unique domains from inventory
            match list_domains(&storage, true) {
                Ok(domains) => {
                    if domains.is_empty() {
                        println!("No domains found in inventory.");
                    } else {
                        println!("Unique domains in inventory ({}):", domains.len());
                        for domain in domains {
                            println!("- {}", domain);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error listing domains: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
            process::exit(EXIT_SUCCESS);
        }
        ListOptions::None => {}
    }

    // Check if we're handling a 'tree' command
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "tree" {
        match display_domain_tree(&storage) {
            Ok(_) => {
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error displaying domain tree: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
    }

    // Check if we're handling a 'status' command
    if args.len() > 1 && args[1] == "status" {
        match display_domain_status(&storage) {
            Ok(_) => {
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error displaying domain status: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
    }

    // Check if we're handling a 'stats' command
    match handle_stats_command() {
        StatsOptions::Basic => {
            // Get domain statistics from inventory
            match get_domain_stats(&storage) {
                Ok(stats) => {
                    println!("Domain Statistics");
                    println!("----------------");
                    println!("Total domain entries: {}", stats.total_entries);
                    println!("Unique domains: {}", stats.unique_domains);

                    if let Some(latest) = stats.latest_domain {
                        println!("Latest domain: {}", latest);

                        if let Some(timestamp) = stats.latest_timestamp {
                            println!("Added on: {}", timestamp);
                        }
                    } else {
                        println!("No domains found in inventory.");
                    }
                }
                Err(e) => {
                    eprintln!("Error getting domain statistics: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
            process::exit(EXIT_SUCCESS);
        }
        StatsOptions::ByDomain => {
            // Get domain statistics with detailed counts
            match get_domain_stats(&storage) {
                Ok(stats) => {
                    println!("Domain Statistics By Domain");
                    println!("---------------------------");
                    println!("Total domain entries: {}", stats.total_entries);
                    println!("Unique domains: {}", stats.unique_domains);

                    if !stats.top_domains.is_empty() {
                        println!("\nTop Domains:");
                        for (domain, count) in stats.top_domains {
                            println!("- {} ({} entries)", domain, count);
                        }
                    } else {
                        println!("\nNo domains found in inventory.");
                    }
                }
                Err(e) => {
                    eprintln!("Error getting domain statistics: {}", e);
                    process::exit(EXIT_STORAGE_ERROR);
                }
            }
            process::exit(EXIT_SUCCESS);
        }
        StatsOptions::None => {}
    }

    // Check if we're handling a 'diff' command
    if handle_diff_command() {
        match get_inventory_diff(&storage, None) {
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
            }
            Err(e) => {
                eprintln!("Error getting inventory diff: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
        process::exit(EXIT_SUCCESS);
    }

    // Check if we're handling a 'check' command
    if handle_check_command() {
        match check_inventory(&storage, &ignore_patterns) {
            Ok(issues) => {
                if issues.is_empty() {
                    println!("No issues found in inventory.");
                    process::exit(EXIT_SUCCESS);
                }

                // Count issues by severity
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

                // Group and display by severity
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

                // Return appropriate exit code based on severity
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

    // Check if we're handling a 'report' command
    if handle_report_command() {
        match generate_report(&storage, &ignore_patterns) {
            Ok(report) => {
                println!("Balebos Report (last 7 days)");
                println!("----------------------------");
                println!();

                println!("Inventory:");
                println!("- total domains: {}", report.total_domains);
                println!("- unique domains: {}", report.unique_domains);
                println!();

                println!("Changes:");
                // Domain changes
                if report.new_domains > 0 {
                    println!(
                        "+ {} new domain{}",
                        report.new_domains,
                        if report.new_domains == 1 { "" } else { "s" }
                    );
                } else {
                    println!("+ no new domains");
                }

                if report.missing_domains > 0 {
                    println!(
                        "- {} missing domain{}",
                        report.missing_domains,
                        if report.missing_domains == 1 { "" } else { "s" }
                    );
                } else {
                    println!("- no missing domains");
                }

                // Subdomain changes (if available)
                if report.new_subdomains > 0 {
                    println!(
                        "+ {} new subdomain{}",
                        report.new_subdomains,
                        if report.new_subdomains == 1 { "" } else { "s" }
                    );
                }

                if report.missing_subdomains > 0 {
                    println!(
                        "- {} missing subdomain{}",
                        report.missing_subdomains,
                        if report.missing_subdomains == 1 {
                            ""
                        } else {
                            "s"
                        }
                    );
                }
                println!();

                println!("Issues:");
                let total_issues = report.critical_issues + report.warning_issues;

                if total_issues == 0 {
                    println!("- no issues detected");
                } else {
                    if report.stale_domains > 0 {
                        println!(
                            "- {} stale domain{} (>14 days)",
                            report.stale_domains,
                            if report.stale_domains == 1 { "" } else { "s" }
                        );
                    }

                    if report.low_confidence_domains > 0 {
                        println!(
                            "- {} low confidence domain{}",
                            report.low_confidence_domains,
                            if report.low_confidence_domains == 1 {
                                ""
                            } else {
                                "s"
                            }
                        );
                    }

                    if report.unknown_sources > 0 {
                        println!(
                            "- {} unknown source{}",
                            report.unknown_sources,
                            if report.unknown_sources == 1 { "" } else { "s" }
                        );
                    }

                    if report.invalid_data > 0 {
                        println!(
                            "- {} normalization issue{}",
                            report.invalid_data,
                            if report.invalid_data == 1 { "" } else { "s" }
                        );
                    }

                    if report.suspicious_subdomains > 0 {
                        println!(
                            "- {} suspicious subdomain pattern{}",
                            report.suspicious_subdomains,
                            if report.suspicious_subdomains == 1 {
                                ""
                            } else {
                                "s"
                            }
                        );
                    }

                    if report.http_errors > 0 {
                        println!(
                            "- {} HTTP status error{}",
                            report.http_errors,
                            if report.http_errors == 1 { "" } else { "s" }
                        );
                    }

                    if report.missing_http_checks > 0 {
                        println!(
                            "- {} domain{} without recent HTTP checks",
                            report.missing_http_checks,
                            if report.missing_http_checks == 1 {
                                ""
                            } else {
                                "s"
                            }
                        );
                    }
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

                // Return appropriate exit code based on needs_attention
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

    // Check if we're handling a 'probe' command
    if let Some((probe_type, domain)) = handle_probe_command() {
        if probe_type == "subdomains" {
            match probe_subdomains(&domain, &storage) {
                Ok(subdomains) => {
                    if subdomains.is_empty() {
                        println!("No subdomains found for {}", domain);
                    } else {
                        println!("Found {} subdomains for {}:", subdomains.len(), domain);
                        for subdomain in &subdomains {
                            println!("- {}", subdomain);
                        }
                        println!("\nAll subdomains added to inventory.");
                        println!("Run 'balebos diff' to see changes or 'balebos check' to detect anomalies.");
                    }
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error probing subdomains: {}", e);
                    eprintln!("\nMake sure subfinder is installed: https://github.com/projectdiscovery/subfinder");
                    process::exit(EXIT_EXTERNAL_TOOL_ERROR);
                }
            }
        } else if probe_type == "http" {
            match http_check(&domain, &storage) {
                Ok(observation) => {
                    println!("HTTP check completed for {}", domain);

                    // Display result based on success or error
                    if let Some(status) = observation.status {
                        println!("Status: {}", status);
                        println!("Response time: {}ms", observation.response_time_ms);
                    } else if let Some(error_kind) = observation.error_kind {
                        println!("Probe failed: {:?}", error_kind);
                        println!(
                            "Error: {}",
                            observation.error_message.as_deref().unwrap_or("unknown")
                        );
                        println!("Response time: {}ms", observation.response_time_ms);
                    }

                    println!("\nObservation added to inventory.");
                    println!(
                        "Run 'balebos check' to detect anomalies or 'balebos report' for a status summary."
                    );

                    // Map observation to exit code
                    if let Some(error_kind) = observation.error_kind {
                        match error_kind {
                            HttpErrorKind::DnsNotFound => {
                                process::exit(EXIT_DOMAIN_NOT_FOUND);
                            }
                            HttpErrorKind::Timeout | HttpErrorKind::ConnectionError => {
                                process::exit(EXIT_PROBE_ERROR);
                            }
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
            }
        }
    }

    // If we get here, no valid command was provided
    print_usage();
    process::exit(EXIT_INVALID_ARGUMENT);
}
