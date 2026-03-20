use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use serde::Deserialize;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

// Lightweight header to identify record type without full deserialization
#[derive(Deserialize)]
struct RecordHeader {
    item_type: String,
}

// List all domains in the inventory
// This function:
// 1. Reads heterogeneous JSONL log (domains, observations, etc.)
// 2. Filters for only "domain" item_type records
// 3. Deserializes and extracts domain names
// 4. Optionally deduplicates the domains
pub fn list_domains(storage: &JsonlStorage, unique: bool) -> Result<Vec<String>, io::Error> {
    let file_path = storage.get_filepath();
    let path = Path::new(file_path);

    // If file doesn't exist, return empty list
    if !path.exists() {
        return Ok(vec![]);
    }

    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut domains = Vec::new();

    // Read line by line and filter for domain items
    for line_result in reader.lines() {
        let line = line_result?;

        if line.trim().is_empty() {
            continue;
        }

        // First pass: check record type without full deserialization
        let header = match serde_json::from_str::<RecordHeader>(&line) {
            Ok(h) => h,
            Err(_) => continue, // Skip malformed records
        };

        // Only process domain records
        if header.item_type != "domain" {
            continue;
        }

        // Second pass: deserialize the full domain record
        if let Ok(item) = serde_json::from_str::<InventoryItem>(&line) {
            domains.push(item.name);
        }
    }

    // If unique flag is set, deduplicate while preserving order
    if unique {
        domains.sort();
        domains.dedup();
    }

    Ok(domains)
}
