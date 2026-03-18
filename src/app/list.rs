use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use std::io;

// List all domains in the inventory
// This function:
// 1. Reads all inventory items from storage
// 2. Filters out only domain items
// 3. Returns a Result with a vector of domain names
pub fn list_domains(storage: &JsonlStorage) -> Result<Vec<String>, io::Error> {
    // Read all inventory items, handling any errors using the ? operator
    // The ? operator is Rust's way of saying "if this returns an Err, exit the function with that Err"
    // It's much cleaner than nesting match or if-let statements
    let inventory_items: Vec<InventoryItem> = storage.read_all()?;

    // Filter for domain items and collect their names
    // We're using functional programming concepts here - filter_map is both filtering and transforming
    let domains: Vec<String> = inventory_items
        .iter()
        .filter_map(|item| {
            // Only include items with type "domain"
            if item.item_type == "domain" {
                Some(item.name.clone())
            } else {
                None
            }
        })
        .collect();

    Ok(domains)
}
