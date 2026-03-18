use crate::domain::{normalize_domain, Domain};
use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use std::io;

// Scan a domain and save it to inventory
// This function:
// 1. Normalizes the domain name
// 2. Creates a domain object
// 3. Converts it to an inventory item
// 4. Saves it to storage
// 5. Returns the domain if successful
pub fn scan_domain(domain_name: &str, storage: &JsonlStorage) -> Result<Domain, io::Error> {
    // Normalize the domain name before creating a domain entity
    // This ensures consistent data regardless of input format (http://, www., etc.)
    let normalized_name = normalize_domain(domain_name);

    // Create a domain entity with the normalized name
    let domain = Domain::new(&normalized_name);

    // Store the original input as well for reference
    let original_input = domain_name.to_string();

    // Convert the domain to an inventory item for storage
    let inventory_item = InventoryItem::from_domain(&domain, "cli", Some(original_input));

    // Store the inventory item in JSONL storage
    storage.save(&inventory_item)?;

    // Return the domain object for use in the application
    Ok(domain)
}
