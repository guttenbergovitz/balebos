use crate::domain::Domain;
use crate::storage::JsonlStorage;

pub fn scan_domain(domain_name: &str, storage: &JsonlStorage) -> Result<Domain, std::io::Error> {
    // Create a domain entity
    let domain = Domain::new(domain_name);

    // Store the domain in JSONL storage
    storage.save(&domain)?;

    Ok(domain)
}
