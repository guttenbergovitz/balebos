use std::process;
use std::sync::Arc;

use crate::app::{
    display_domain_status, display_domain_tree, export_status_csv, export_status_html,
    export_status_markdown, get_domain_stats, get_inventory_diff, list_domains,
};
use crate::exit_codes::*;
use crate::storage::StorageEngine;

pub fn cmd_list(unique: bool, storage: &Arc<dyn StorageEngine>) {
    match list_domains(storage, unique) {
        Ok(domains) => {
            if domains.is_empty() {
                println!("No domains found in inventory.");
            } else {
                let label = if unique { "Unique" } else { "All" };
                println!("{} domains in inventory ({}):", label, domains.len());
                let records = storage.read_all_records().ok();
                for domain in domains {
                    let lifecycle = records.as_ref().and_then(|r| {
                        crate::inventory::get_current_lifecycle(&r.lifecycle_events, &domain)
                    });
                    match lifecycle {
                        Some(state) => {
                            println!("- {} ({})", domain, state);
                        }
                        None => {
                            println!("- {}", domain);
                        }
                    }
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

pub fn cmd_tree(storage: &Arc<dyn StorageEngine>) {
    match display_domain_tree(storage) {
        Ok(()) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error displaying domain tree: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

pub fn cmd_status(export: Option<crate::cli::ExportArgs>, storage: &Arc<dyn StorageEngine>) {
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

pub fn cmd_stats(by_domain: bool, storage: &Arc<dyn StorageEngine>) {
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

pub fn cmd_diff(storage: &Arc<dyn StorageEngine>) {
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
