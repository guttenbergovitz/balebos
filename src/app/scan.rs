use crate::domain::normalize::{normalize_domain, validate_domain};
use crate::domain::Domain;
use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use std::io;

pub fn scan_domain(domain_name: &str, storage: &JsonlStorage) -> Result<Domain, io::Error> {
    let normalized_name = normalize_domain(domain_name);

    if let Err(reason) = validate_domain(domain_name) {
        return Err(io::Error::other(format!(
            "invalid domain '{}': {}",
            domain_name, reason
        )));
    }

    let domain = Domain::new(&normalized_name);
    let original_input = domain_name.to_string();
    let inventory_item = InventoryItem::from_domain(&domain, "cli", Some(original_input));
    storage.save(&inventory_item)?;

    Ok(domain)
}
