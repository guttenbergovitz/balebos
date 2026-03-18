use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use std::collections::HashSet;
use std::io;

// List all domains in the inventory
// This function:
// 1. Reads all inventory items from storage
// 2. Filters out only domain items
// 3. Returns a Result with a vector of domain names
// 4. Optionally deduplicates the domains
pub fn list_domains(storage: &JsonlStorage, unique: bool) -> Result<Vec<String>, io::Error> {
    // Read all inventory items
    let inventory_items: Vec<InventoryItem> = storage.read_all()?;

    // Filter for domain items
    let domains: Vec<String> = inventory_items
        .iter()
        .filter_map(|item| {
            if item.item_type == "domain" {
                Some(item.name.clone())
            } else {
                None
            }
        })
        .collect();

    // If unique flag is set, deduplicate domains using a HashSet
    if unique {
        // Convert to HashSet to remove duplicates and back to Vec
        let unique_domains: HashSet<String> = domains.into_iter().collect();
        Ok(unique_domains.into_iter().collect())
    } else {
        Ok(domains)
    }
}
