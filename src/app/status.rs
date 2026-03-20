use crate::storage::JsonlStorage;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Deserialize)]
struct RecordHeader {
    item_type: String,
}

#[derive(Deserialize, Clone, Debug)]
struct HttpCheck {
    target: String,
    status: Option<u16>,
    response_time_ms: u32,
    checked_at: String,
    error_kind: Option<String>,
    error_message: Option<String>,
}

/// Display status of all domains/subdomains with their HTTP probe results
/// Shows live/dead status with HTTP status codes and error information
pub fn display_domain_status(storage: &JsonlStorage) -> Result<(), io::Error> {
    let file_path = storage.get_filepath();
    let path = Path::new(file_path);

    if !path.exists() {
        println!("No inventory found.");
        return Ok(());
    }

    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    // Collect latest HTTP check for each target (dedup by most recent)
    let mut status_map: BTreeMap<String, HttpCheck> = BTreeMap::new();

    for line_result in reader.lines() {
        let line = line_result?;

        if line.trim().is_empty() {
            continue;
        }

        let header = match serde_json::from_str::<RecordHeader>(&line) {
            Ok(h) => h,
            Err(_) => continue,
        };

        if header.item_type != "http_check" {
            continue;
        }

        if let Ok(check) = serde_json::from_str::<HttpCheck>(&line) {
            // Keep the latest check for each target (sorted by checked_at)
            status_map.insert(check.target.clone(), check);
        }
    }

    if status_map.is_empty() {
        println!("No HTTP checks found in inventory.");
        return Ok(());
    }

    // Categorize by status
    let mut alive = Vec::new();
    let mut client_errors = Vec::new();
    let mut server_errors = Vec::new();
    let mut network_errors = Vec::new();
    let mut unchecked = Vec::new();

    for (_, check) in status_map.iter() {
        match check.status {
            Some(200) => alive.push(check.clone()),
            Some(code) if code >= 300 && code < 400 => client_errors.push(check.clone()),
            Some(code) if code >= 400 && code < 500 => client_errors.push(check.clone()),
            Some(code) if code >= 500 && code < 600 => server_errors.push(check.clone()),
            Some(_) => unchecked.push(check.clone()),
            None => {
                if check.error_kind.is_some() {
                    network_errors.push(check.clone());
                } else {
                    unchecked.push(check.clone());
                }
            }
        }
    }

    let total = status_map.len();
    println!("Domain Status Report");
    println!("====================");
    println!("Total assets: {}\n", total);

    // Print statistics
    println!("Status Summary:");
    println!(
        "  🟢 Alive (HTTP 200):        {:4} ({:5.1}%)",
        alive.len(),
        (alive.len() as f64 / total as f64) * 100.0
    );
    println!(
        "  🟡 Client errors (4xx):     {:4} ({:5.1}%)",
        client_errors.len(),
        (client_errors.len() as f64 / total as f64) * 100.0
    );
    println!(
        "  🔴 Server errors (5xx):     {:4} ({:5.1}%)",
        server_errors.len(),
        (server_errors.len() as f64 / total as f64) * 100.0
    );
    println!(
        "  ⚠️  Network errors:          {:4} ({:5.1}%)",
        network_errors.len(),
        (network_errors.len() as f64 / total as f64) * 100.0
    );
    println!(
        "  ⚪ Unchecked:               {:4} ({:5.1}%)\n",
        unchecked.len(),
        (unchecked.len() as f64 / total as f64) * 100.0
    );

    // Print detailed listings
    if !alive.is_empty() {
        println!("🟢 ALIVE (HTTP 200) - {} assets", alive.len());
        println!("─────────────────────────────────");
        for check in alive.iter().take(50) {
            println!("  {} ({} ms)", check.target, check.response_time_ms);
        }
        if alive.len() > 50 {
            println!("  ... and {} more", alive.len() - 50);
        }
        println!();
    }

    if !server_errors.is_empty() {
        println!("🔴 SERVER ERRORS (5xx) - {} assets", server_errors.len());
        println!("──────────────────────────────────");
        for check in server_errors.iter().take(30) {
            let status_str = check.status.map(|s| s.to_string()).unwrap_or_default();
            println!("  {} - HTTP {}", check.target, status_str);
        }
        if server_errors.len() > 30 {
            println!("  ... and {} more", server_errors.len() - 30);
        }
        println!();
    }

    if !client_errors.is_empty() {
        println!(
            "🟡 CLIENT ERRORS (3xx/4xx) - {} assets",
            client_errors.len()
        );
        println!("─────────────────────────────────────");
        for check in client_errors.iter().take(30) {
            let status_str = check.status.map(|s| s.to_string()).unwrap_or_default();
            println!("  {} - HTTP {}", check.target, status_str);
        }
        if client_errors.len() > 30 {
            println!("  ... and {} more", client_errors.len() - 30);
        }
        println!();
    }

    if !network_errors.is_empty() {
        println!("⚠️  NETWORK ERRORS - {} assets", network_errors.len());
        println!("─────────────────────────────");
        for check in network_errors.iter().take(30) {
            let error_kind = check.error_kind.as_deref().unwrap_or("unknown");
            println!(
                "  {} - {} ({}ms)",
                check.target, error_kind, check.response_time_ms
            );
        }
        if network_errors.len() > 30 {
            println!("  ... and {} more", network_errors.len() - 30);
        }
        println!();
    }

    Ok(())
}
