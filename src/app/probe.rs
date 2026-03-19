use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use chrono::Utc;
use std::io;
use std::process::Command;

// Run subfinder to find subdomains
fn run_subfinder(domain: &str) -> Result<String, io::Error> {
    let output = Command::new("subfinder")
        .arg("-d")
        .arg(domain)
        .arg("-silent")
        .output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("subfinder failed: {}", error),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// Parse subfinder output into list of subdomains
fn parse_subdomains(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

// Probe for subdomains and add them to inventory
pub fn probe_subdomains(domain: &str, storage: &JsonlStorage) -> Result<Vec<String>, io::Error> {
    // Run subfinder to get subdomains
    let output = run_subfinder(domain)?;

    // Parse output into list of subdomains
    let subdomains = parse_subdomains(&output);

    if subdomains.is_empty() {
        return Ok(vec![]);
    }

    // Current timestamp for all discoveries
    let now = Utc::now();

    // Add each subdomain to inventory
    for subdomain in &subdomains {
        let item = InventoryItem {
            item_type: "subdomain".to_string(),
            id: format!("sub:{}", subdomain), // Simple ID format
            name: subdomain.clone(),
            discovered_at: now,
            source: "subfinder".to_string(),
            original_input: Some(domain.to_string()),
        };

        storage.save(&item)?;
    }

    Ok(subdomains)
}
