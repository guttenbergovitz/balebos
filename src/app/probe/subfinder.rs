use crate::error::{ProbeError, Result};
use crate::inventory::{InventoryItem, ItemType};
use crate::storage::{StorageEngine, StorageEngineExt};
use chrono::Utc;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;
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

/// Detect auto-junk candidates from subdomain discovery results.
/// Returns candidates that are likely noise: wildcards, deep nesting, cross-domain.
pub fn detect_junk_candidates(subs: &[String], parent_domains: &[String]) -> Vec<String> {
    subs.iter()
        .filter(|s| is_likely_junk(s, parent_domains))
        .cloned()
        .collect()
}

fn is_likely_junk(subdomain: &str, parent_domains: &[String]) -> bool {
    // Wildcard pattern (starts with *.)
    if subdomain.starts_with("*.") {
        return true;
    }

    // Deep nesting: 5+ levels (e.g., www.stage.de.beta.green-petfood.com)
    let level = subdomain.matches('.').count();
    if level >= 5 {
        return true;
    }

    // Cross-domain: not actually a subdomain of any parent domain
    let is_subdomain_of_parent = parent_domains
        .iter()
        .any(|parent| subdomain == parent || subdomain.ends_with(&format!(".{}", parent)));
    !is_subdomain_of_parent
}

pub fn probe_subdomains(domain: &str, storage: &Arc<dyn StorageEngine>) -> Result<Vec<String>> {
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

fn run_subfinder_batch_inner(
    domains: &[String],
    save_to_storage: bool,
    storage: Option<&Arc<dyn StorageEngine>>,
) -> Result<(Vec<String>, Vec<InventoryItem>)> {
    let mut all_subdomains = Vec::new();
    let mut all_items = Vec::new();
    let chunk_size = 10; // Smaller chunks = faster completion, better timeout handling

    for (chunk_idx, chunk) in domains.chunks(chunk_size).enumerate() {
        let total_chunks = (domains.len() as f32 / chunk_size as f32).ceil() as usize;
        // Skip printing in TUI - status is handled by the caller
        if std::env::var("BALEBOS_STORAGE").is_err() {
            println!(
                "    ⟳ Processing chunk {}/{} ({} domains)...",
                chunk_idx + 1,
                total_chunks,
                chunk.len()
            );
        }

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("balebos_subfinder_{}.txt", Uuid::new_v4()));
        fs::write(&temp_file, chunk.join("\n"))?;

        let temp_file_str = temp_file.to_string_lossy();

        // Try chaos first if API key is available, fallback to subfinder
        let use_chaos = std::env::var("PDCP_API_KEY").is_ok();
        let tool_name = if use_chaos { "chaos" } else { "subfinder" };

        let mut cmd = if use_chaos {
            let mut c = Command::new("chaos");
            c.arg("-dL").arg(temp_file_str.as_ref()).arg("-silent");
            c
        } else {
            let mut c = Command::new("subfinder");
            c.arg("-dL")
                .arg(temp_file_str.as_ref())
                .arg("-silent")
                .arg("-timeout") // Add timeout
                .arg("30"); // 30 seconds max per domain
            c
        };

        let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

        // Add our own timeout in case subfinder ignores it
        let start_time = std::time::Instant::now();
        let timeout_duration = std::time::Duration::from_secs(25); // More aggressive timeout

        // Log chunk start
        if std::env::var("BALEBOS_STORAGE").is_err() {
            println!(
                "      🚀 Starting {} for {} domains...",
                tool_name,
                chunk.len()
            );
        }

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                eprintln!("      ⚠ Failed to get subfinder stdout");
                continue;
            }
        };
        let reader = BufReader::new(stdout);
        let now = Utc::now();
        let mut chunk_items = Vec::new();
        let mut chunk_subdomains = Vec::new();
        let mut found_in_chunk = 0;
        let mut last_log_time = start_time;

        for line in reader.lines() {
            // Check if we've exceeded our timeout
            let elapsed = start_time.elapsed();
            if elapsed > timeout_duration {
                let _ = child.kill(); // Kill tool if it's hanging
                eprintln!(
                    "      ⚠ {} timeout after {}s, moving to next chunk",
                    tool_name,
                    elapsed.as_secs()
                );
                break;
            }

            // Log progress every 5 seconds
            if elapsed.as_secs() - last_log_time.elapsed().as_secs() > 5 {
                println!(
                    "      ⏱ {} still scanning... {}s elapsed, found {} subdomains",
                    tool_name,
                    elapsed.as_secs(),
                    found_in_chunk
                );
                last_log_time = Instant::now();
            }

            let subdomain = line?;
            let trimmed = subdomain.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }

            found_in_chunk += 1;
            // Log every 5th subdomain for better visibility without spam
            if found_in_chunk % 5 == 0 {
                print!(
                    "\r      → Found {} subdomains in current chunk...",
                    found_in_chunk
                );
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }

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
                source: tool_name.to_string(),
                original_input,
            };

            chunk_subdomains.push(trimmed);
            chunk_items.push(item);

            // Periodically save to disk to keep memory low and data safe
            if save_to_storage && chunk_items.len() >= 100 {
                if let Some(s) = storage {
                    s.save_batch(&chunk_items)?;
                }
                chunk_items.clear();
            }
        }

        // Log chunk completion
        let total_elapsed = start_time.elapsed();
        if std::env::var("BALEBOS_STORAGE").is_err() {
            if found_in_chunk > 0 {
                println!(
                    "\r      ✅ Chunk completed: {} subdomains found in {}s",
                    found_in_chunk,
                    total_elapsed.as_secs()
                );
            } else {
                println!(
                    "      ℹ Chunk completed: no subdomains found in {}s",
                    total_elapsed.as_secs()
                );
            }
        }

        // Final save for the chunk
        if save_to_storage && !chunk_items.is_empty() {
            if let Some(s) = storage {
                s.save_batch(&chunk_items)?;
            }
        } else if !save_to_storage {
            all_items.extend(chunk_items);
        }
        println!(); // New line after chunk progress (only in CLI mode)

        let _ = child.wait();
        let _ = fs::remove_file(&temp_file);

        all_subdomains.extend(chunk_subdomains);
    }

    Ok((all_subdomains, all_items))
}

/// Discover subdomains without saving to storage.
/// Returns (subdomains, inventory_items) for batch buffering.
pub fn probe_subdomains_batch_no_save(
    domains: &[String],
) -> Result<(Vec<String>, Vec<InventoryItem>)> {
    if domains.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    run_subfinder_batch_inner(domains, false, None)
}
