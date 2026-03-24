use crate::error::{ProbeError, Result};
use crate::inventory::{InventoryItem, ItemType};
use crate::storage::JsonlStorage;
use chrono::Utc;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use uuid::Uuid;

fn run_subfinder(domain: &str) -> Result<String> {
    let output = Command::new("subfinder")
        .arg("-d")
        .arg(domain)
        .arg("-silent")
        .output()
        .map_err(ProbeError::Execution)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(ProbeError::ToolFailed {
            tool: "subfinder".to_string(),
            stderr,
        }
        .into());
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

pub fn probe_subdomains(domain: &str, storage: &JsonlStorage) -> Result<Vec<String>> {
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
            item_type: ItemType::Subdomain,
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

fn run_subfinder_batch(domains: &[String], storage: &JsonlStorage) -> Result<Vec<String>> {
    let mut all_subdomains = Vec::new();
    let chunk_size = 10; // Frequent updates for better feedback

    for (chunk_idx, chunk) in domains.chunks(chunk_size).enumerate() {
        let total_chunks = (domains.len() as f32 / chunk_size as f32).ceil() as usize;
        println!(
            "    ⟳ Processing chunk {}/{} ({} domains)...",
            chunk_idx + 1,
            total_chunks,
            chunk.len()
        );

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("balebos_subfinder_{}.txt", Uuid::new_v4()));
        fs::write(&temp_file, chunk.join("\n"))?;

        let temp_file_str = temp_file.to_string_lossy();
        let mut child = Command::new("subfinder")
            .arg("-dL")
            .arg(temp_file_str.as_ref())
            .arg("-silent")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => continue,
        };
        let reader = BufReader::new(stdout);
        let now = Utc::now();
        let mut chunk_items = Vec::new();
        let mut chunk_subdomains = Vec::new();
        let mut found_in_chunk = 0;

        for line in reader.lines() {
            let subdomain = line?;
            let trimmed = subdomain.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            
            found_in_chunk += 1;
            print!("\r      → Found {} subdomains in current chunk...", found_in_chunk);
            use std::io::Write;
            let _ = std::io::stdout().flush();

            // Find original apex domain for context
            let mut original_input = None;
            for d in chunk {
                if trimmed.ends_with(d) {
                    original_input = Some(d.clone());
                    break;
                }
            }

            let item = InventoryItem {
                item_type: ItemType::Subdomain,
                id: format!("sub:{}", trimmed),
                name: trimmed.clone(),
                discovered_at: now,
                source: "subfinder".to_string(),
                original_input,
            };

            chunk_subdomains.push(trimmed);
            chunk_items.push(item);

            // Periodically save to disk to keep memory low and data safe
            if chunk_items.len() >= 100 {
                storage.save_batch(&chunk_items)?;
                chunk_items.clear();
            }
        }

        // Final save for the chunk
        if !chunk_items.is_empty() {
            storage.save_batch(&chunk_items)?;
        }
        println!(); // New line after chunk progress

        let _ = child.wait();
        let _ = fs::remove_file(&temp_file);
        
        all_subdomains.extend(chunk_subdomains);
    }

    Ok(all_subdomains)
}

pub fn probe_subdomains_batch(
    domains: &[String],
    storage: &JsonlStorage,
) -> Result<Vec<String>> {
    if domains.is_empty() {
        return Ok(Vec::new());
    }

    // Now returns the collected list, while internal function handles streaming save
    run_subfinder_batch(domains, storage)
}
