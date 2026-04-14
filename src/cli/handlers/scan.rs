use std::process;
use std::sync::Arc;

use crate::app::{batch_process, scan_domain};
use crate::exit_codes::*;
use crate::storage::StorageEngine;

pub fn cmd_scan(domain_name: &str, storage: &Arc<dyn StorageEngine>) {
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

pub fn cmd_batch(file: &str, storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
    match batch_process(file, storage, ignore_patterns) {
        Ok(_) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error during batch processing: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    }
}
