use crate::domain::normalize::{normalize_domain, validate_domain};
use crate::domain::Domain;
use crate::error::{BalebosError, Result};
use crate::inventory::InventoryItem;
use crate::storage::{StorageEngine, StorageEngineExt};
use std::sync::Arc;

pub fn scan_domain(domain_name: &str, storage: &Arc<dyn StorageEngine>) -> Result<Domain> {
    let (domain, item) = scan_domain_no_save(domain_name)?;
    storage.save(&item)?;
    Ok(domain)
}

pub fn scan_domain_no_save(domain_name: &str) -> Result<(Domain, InventoryItem)> {
    let normalized_name = normalize_domain(domain_name);

    if let Err(reason) = validate_domain(domain_name) {
        return Err(BalebosError::Validation(format!(
            "invalid domain '{}': {}",
            domain_name, reason
        )));
    }

    let domain = Domain::new(&normalized_name);
    let original_input = domain_name.to_string();
    let inventory_item = InventoryItem::from_domain(&domain, "cli", Some(original_input));

    Ok((domain, inventory_item))
}
