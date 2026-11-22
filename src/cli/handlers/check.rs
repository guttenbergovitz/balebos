use std::process;
use std::sync::Arc;

use crate::app::check::get_open_findings_as_issues;
use crate::app::{check_inventory, generate_report, Severity};
use crate::exit_codes::*;
use crate::storage::StorageEngine;

#[allow(dead_code)]
pub fn cmd_check(storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
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

#[allow(dead_code)]
pub fn cmd_report(storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
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

/// Print a line like `+ 3 new domains` or `+ no new domains`.
#[allow(dead_code)]
fn print_count_line(prefix: &str, label: &str, count: usize) {
    if count > 0 {
        let suffix = if count == 1 { "" } else { "s" };
        println!("{} {} {}{}", prefix, count, label, suffix);
    } else {
        println!("{} no {}s", prefix, label);
    }
}

/// Print issue line only if count > 0.
#[allow(dead_code)]
fn print_issue_line(label: &str, count: usize, extra: &str) {
    if count > 0 {
        let suffix = if count == 1 { "" } else { "s" };
        println!("- {} {}{}{}", count, label, suffix, extra);
    }
}
