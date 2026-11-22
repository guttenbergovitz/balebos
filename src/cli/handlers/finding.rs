use std::process;
use std::sync::Arc;

use crate::app::finding::{
    Finding, FindingCategory, FindingEvent, FindingEventType, FindingSeverity,
};
use crate::cli::{FindingCommand, FindingSeverityFilter, FindingStatus};
use crate::exit_codes::*;
use crate::storage::{StorageEngine, StorageEngineExt};

#[allow(dead_code)]
pub fn cmd_finding(
    finding_cmd: FindingCommand,
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
) {
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

                    let _ = storage.flush();
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
                        let _ = storage.flush();
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
                        let _ = storage.flush();
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
                    let _ = storage.flush();
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
