use crate::storage::JsonlStorage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsObservation {
    pub item_type: String,
    pub id: String,
    pub asset_type: String,
    pub target: String,

    #[serde(default)]
    pub a: Vec<String>,
    #[serde(default)]
    pub aaaa: Vec<String>,
    #[serde(default)]
    pub cname: Vec<String>,

    pub checked_at: chrono::DateTime<Utc>,
    pub source: String,
}

#[derive(Deserialize)]
struct DnsxOutput {
    host: String,
    #[serde(default)]
    a: Vec<String>,
    #[serde(default)]
    aaaa: Vec<String>,
    #[serde(default)]
    cname: Vec<String>,
}

/// Helper function to check if dnsx is correctly installed.
fn check_dnsx_installation() -> Result<(), io::Error> {
    let _ = Command::new("dnsx")
        .arg("-version")
        .output()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "dnsx not found in PATH. Please install it from ProjectDiscovery.",
                )
            } else {
                e
            }
        })?;
    Ok(())
}

/// Run a batch DNS probe using a single dnsx process for efficiency.
pub fn probe_dns_batch(
    targets: &[String],
    storage: &JsonlStorage,
) -> Result<Vec<DnsObservation>, io::Error> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    check_dnsx_installation()?;

    let mut child = Command::new("dnsx")
        .args(["-json", "-a", "-aaaa", "-cname", "-silent"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Send all targets to the standard input of dnsx via a separate thread
    // to avoid blocking if the list is extremely large (though OS buffers often handle it).
    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let targets_string = targets.join("\n") + "\n";
    
    thread::spawn(move || {
        let _ = stdin.write_all(targets_string.as_bytes());
    });

    let stdout = child.stdout.take().expect("Failed to open stdout");
    let reader = BufReader::new(stdout);
    let mut observations = Vec::new();

    // Stream results as they arrive
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<DnsxOutput>(&line) {
            Ok(parsed) => {
                let obs = DnsObservation {
                    item_type: "dns_record".to_string(),
                    id: Uuid::new_v4().to_string(),
                    asset_type: "domain".to_string(),
                    target: parsed.host,
                    a: parsed.a,
                    aaaa: parsed.aaaa,
                    cname: parsed.cname,
                    checked_at: Utc::now(),
                    source: "dnsx".to_string(),
                };
                storage.save(&obs)?;
                observations.push(obs);
                
                // Print a progress indicator
                print!(".");
                let _ = io::stdout().flush();
            }
            Err(_) => {
                // Ignore parse errors on silent stdout for batch reliability
                continue;
            }
        }
    }

    let status = child.wait()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("dnsx batch process failed with status: {}", status),
        ));
    }

    Ok(observations)
}

/// Maintain compatibility with single probes by wrapping the batch implementation.
pub fn probe_dns(target: &str, storage: &JsonlStorage) -> Result<DnsObservation, io::Error> {
    let targets = vec![target.to_string()];
    let mut results = probe_dns_batch(&targets, storage)?;
    
    if let Some(obs) = results.pop() {
        Ok(obs)
    } else {
        // If dnsx returns nothing, the host probably has no records. Return empty observation.
        let obs = DnsObservation {
            item_type: "dns_record".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            a: Vec::new(),
            aaaa: Vec::new(),
            cname: Vec::new(),
            checked_at: Utc::now(),
            source: "dnsx".to_string(),
        };
        storage.save(&obs)?;
        Ok(obs)
    }
}
