mod app;
mod cli;
mod domain;
mod storage;

use app::scan_domain;
use cli::handle_scan_command;
use storage::JsonlStorage;

fn main() {
    // Get domain from CLI arguments
    let domain_name = match handle_scan_command() {
        Some(domain) => domain,
        None => return,
    };

    // Create storage
    let storage = JsonlStorage::new("data/domains.jsonl");

    // Execute domain scan
    match scan_domain(&domain_name, &storage) {
        Ok(domain) => {
            println!("Domain scanned successfully: {}", domain.name);
            println!("Domain ID: {:?}", domain.id);
            println!("Discovered at: {}", domain.discovered_at);
        }
        Err(e) => {
            eprintln!("Error scanning domain: {}", e);
        }
    }
}
