mod app;
mod cli;
mod domain;
mod inventory;
mod storage;

use app::{get_domain_stats, list_domains, scan_domain};
use cli::{handle_list_command, handle_scan_command, handle_stats_command};
use storage::JsonlStorage;

fn main() {
    // Create storage with the new inventory.jsonl file
    // This is our centralized inventory for all assets
    let storage = JsonlStorage::new("data/inventory.jsonl");

    // Check if we're handling a 'scan' command
    if let Some(domain_name) = handle_scan_command() {
        // Execute domain scan and save to inventory
        match scan_domain(&domain_name, &storage) {
            Ok(domain) => {
                println!("Domain scanned successfully: {}", domain.name);
                println!("Domain ID: {:?}", domain.id);
                println!("Discovered at: {}", domain.discovered_at);
            }
            Err(e) => {
                // Error handling is important! We don't panic, but inform the user
                eprintln!("Error scanning domain: {}", e);
            }
        }
        return;
    }

    // Check if we're handling a 'list' command
    if handle_list_command() {
        // List all domains from inventory
        match list_domains(&storage) {
            Ok(domains) => {
                if domains.is_empty() {
                    println!("No domains found in inventory.");
                } else {
                    println!("Domains in inventory:");
                    // Iterate through each domain and print it
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

    // Check if we're handling a 'stats' command
    if handle_stats_command() {
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

    // If we get here, no valid command was provided
    // We'll show a helpful message to guide the user
    eprintln!("Usage:");
    eprintln!("  balebos scan <domain>  - Scan a domain and add to inventory");
    eprintln!("  balebos list           - List all domains in inventory");
    eprintln!("  balebos stats          - Show domain statistics");
}
