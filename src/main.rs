mod app;
mod cli;
mod domain;
mod inventory;
mod storage;

use app::{
    check_inventory, generate_report, get_domain_stats, get_inventory_diff, list_domains,
    scan_domain, Severity,
};
use cli::list::ListOptions;
use cli::stats::StatsOptions;
use cli::{
    handle_check_command, handle_diff_command, handle_list_command, handle_report_command,
    handle_scan_command, handle_stats_command,
};
use storage::JsonlStorage;

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  balebos scan <domain>        - Scan a domain and add to inventory");
    eprintln!("  balebos list                 - List all domains in inventory");
    eprintln!("  balebos list --unique        - List unique domains in inventory");
    eprintln!("  balebos stats                - Show basic domain statistics");
    eprintln!("  balebos stats --by-domain    - Show detailed statistics per domain");
    eprintln!("  balebos diff                 - Show domain changes (last 7 days)");
    eprintln!("  balebos check                - Check for inventory issues and anomalies");
    eprintln!("  balebos report               - Generate consolidated status report");
}

fn main() {
    // Create storage with the inventory.jsonl file
    let storage = JsonlStorage::new("data/inventory.jsonl");

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
            }
        }
        return;
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
                }
            }
            return;
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
                }
            }
            return;
        }
        ListOptions::None => {}
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
                }
            }
            return;
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
                }
            }
            return;
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
            }
        }
        return;
    }

    // Check if we're handling a 'check' command
    if handle_check_command() {
        match check_inventory(&storage) {
            Ok(issues) => {
                if issues.is_empty() {
                    println!("No issues found in inventory.");
                    return;
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
            }
            Err(e) => {
                eprintln!("Error checking inventory: {}", e);
            }
        }
        return;
    }

    // Check if we're handling a 'report' command
    if handle_report_command() {
        match generate_report(&storage) {
            Ok(report) => {
                println!("Balebos Report (last 7 days)");
                println!("----------------------------");
                println!();

                println!("Inventory:");
                println!("- total domains: {}", report.total_domains);
                println!("- unique domains: {}", report.unique_domains);
                println!();

                println!("Changes:");
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
            }
            Err(e) => {
                eprintln!("Error generating report: {}", e);
            }
        }
        return;
    }

    // If we get here, no valid command was provided
    print_usage();
}
