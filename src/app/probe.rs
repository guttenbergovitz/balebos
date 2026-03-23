use crate::inventory::InventoryItem;
use crate::storage::JsonlStorage;
use chrono::Utc;
use std::fs;
use std::io;
use std::process::Command;
use uuid::Uuid;

// Run subfinder to find subdomains
fn run_subfinder(domain: &str) -> Result<String, io::Error> {
    let output = Command::new("subfinder")
        .arg("-d")
        .arg(domain)
        .arg("-silent")
        .output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("subfinder failed: {}", error)));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_subfinder_batch(domains: &[String]) -> Result<String, io::Error> {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("balebos_subfinder_{}.txt", Uuid::new_v4()));
    fs::write(&temp_file, domains.join("\n"))?;

    let output = Command::new("subfinder")
        .arg("-dL")
        .arg(temp_file.to_str().unwrap())
        .arg("-silent")
        .output()?;

    let _ = fs::remove_file(&temp_file);

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("subfinder failed: {}", error)));
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
    let mut items = Vec::new();

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
        items.push(item);
    }

    storage.save_batch(&items)?;
    Ok(subdomains)
}

pub fn probe_subdomains_batch(
    domains: &[String],
    storage: &JsonlStorage,
) -> Result<Vec<String>, io::Error> {
    if domains.is_empty() {
        return Ok(Vec::new());
    }

    let output = run_subfinder_batch(domains)?;
    let subdomains = parse_subdomains(&output);

    if subdomains.is_empty() {
        return Ok(Vec::new());
    }

    let now = Utc::now();
    let mut items = Vec::new();

    for subdomain in &subdomains {
        let mut original_input = None;
        for d in domains {
            if subdomain.ends_with(d) {
                original_input = Some(d.clone());
                break;
            }
        }

        let item = InventoryItem {
            item_type: "subdomain".to_string(),
            id: format!("sub:{}", subdomain),
            name: subdomain.clone(),
            discovered_at: now,
            source: "subfinder".to_string(),
            original_input,
        };
        items.push(item);
    }

    storage.save_batch(&items)?;
    Ok(subdomains)
}
