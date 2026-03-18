use crate::domain::Domain;
use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use std::io;

// Scan a domain and save it to inventory
// This function:
// 1. Creates a domain object
// 2. Converts it to an inventory item
// 3. Saves it to storage
// 4. Returns the domain if successful
pub fn scan_domain(domain_name: &str, storage: &JsonlStorage) -> Result<Domain, io::Error> {
    // Create a domain entity - this is our in-memory representation
    let domain = Domain::new(domain_name);

    // Convert the domain to an inventory item for storage
    // This demonstrates the difference between our domain model (how we think about data in code)
    // and our storage model (how we save data to disk)
    let inventory_item = InventoryItem::from_domain(&domain, "cli");

    // Store the inventory item in JSONL storage
    // The ? operator propagates any errors that occur
    storage.save(&inventory_item)?;

    // Return the domain object for use in the application
    Ok(domain)
}
