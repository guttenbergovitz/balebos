use super::types::DnsObservation;
use crate::error::{ProbeError, Result};
use crate::inventory::ItemType;
use crate::storage::{StorageEngine, StorageEngineExt};
use std::sync::Arc;
use chrono::Utc;
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
struct SoaRecord {
    ns: Option<String>,
    mailbox: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DnsxOutput {
    host: String,
    #[serde(default)]
    a: Vec<String>,
    #[serde(default)]
    aaaa: Vec<String>,
    #[serde(default)]
    cname: Vec<String>,
    #[serde(default)]
    mx: Vec<String>,
    #[serde(default)]
    ns: Vec<String>,
    #[serde(default)]
    txt: Vec<String>,
    #[serde(default)]
    soa: Option<Vec<SoaRecord>>,
}

/// Helper function to check if dnsx is correctly installed.
fn check_dnsx_installation() -> Result<()> {
    Command::new("dnsx").arg("-version").output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ProbeError::ToolNotFound {
                tool: "dnsx".to_string(),
            }
        } else {
            ProbeError::Execution(e)
        }
    })?;
    Ok(())
}

/// Run a batch DNS probe using a single dnsx process for efficiency.
pub fn probe_dns_batch(targets: &[String], storage: &Arc<dyn StorageEngine>) -> Result<Vec<DnsObservation>> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    check_dnsx_installation()?;

    let mut all_observations = Vec::new();
    let chunk_size = 10;

    for (chunk_idx, chunk) in targets.chunks(chunk_size).enumerate() {
        let total_chunks = (targets.len() as f32 / chunk_size as f32).ceil() as usize;
        println!(
            "    ⟳ DNS: Processing chunk {}/{} ({} targets)...",
            chunk_idx + 1,
            total_chunks,
            chunk.len()
        );

        // Use a temporary file for targets to avoid pipe issues
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("balebos_dns_{}.txt", Uuid::new_v4()));
        fs::write(&temp_file, chunk.join("\n"))?;

        let output = Command::new("dnsx")
            .args([
                "-l",
                &temp_file.to_string_lossy(),
                "-json",
                "-a",
                "-aaaa",
                "-cname",
                "-mx",
                "-ns",
                "-txt",
                "-soa",
                "-silent",
            ])
            .output()?;

        let _ = fs::remove_file(&temp_file);

        if !output.status.success() {
            eprintln!("      ✗ DNS chunk failed");
            continue;
        }

        let mut observations = Vec::new();
        let reader = BufReader::new(&output.stdout[..]);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<DnsxOutput>(&line) {
                Ok(parsed) => {
                    let soa_strings = parsed
                        .soa
                        .unwrap_or_default()
                        .into_iter()
                        .map(|s| {
                            format!(
                                "ns:{} mailbox:{}",
                                s.ns.unwrap_or_default(),
                                s.mailbox.unwrap_or_default()
                            )
                        })
                        .collect();

                    let obs = DnsObservation {
                        item_type: ItemType::DnsRecord,
                        id: Uuid::new_v4().to_string(),
                        asset_type: "domain".to_string(),
                        target: parsed.host,
                        a: parsed.a,
                        aaaa: parsed.aaaa,
                        cname: parsed.cname,
                        mx: parsed.mx,
                        ns: parsed.ns,
                        txt: parsed.txt,
                        soa: soa_strings,
                        checked_at: Utc::now(),
                        source: "dnsx".to_string(),
                    };
                    observations.push(obs);
                }
                Err(e) => {
                    eprintln!("Failed to parse dnsx output line: {}. Error: {}", line, e);
                }
            }
        }

        if !observations.is_empty() {
            storage.save_batch(&observations)?;
            all_observations.extend(observations);
        }
    }

    Ok(all_observations)
}

pub fn probe_dns(target: &str, storage: &Arc<dyn StorageEngine>) -> Result<DnsObservation> {
    let targets = vec![target.to_string()];
    let mut results = probe_dns_batch(&targets, storage)?;

    if let Some(obs) = results.pop() {
        Ok(obs)
    } else {
        let obs = DnsObservation {
            item_type: ItemType::DnsRecord,
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            a: Vec::new(),
            aaaa: Vec::new(),
            cname: Vec::new(),
            mx: Vec::new(),
            ns: Vec::new(),
            txt: Vec::new(),
            soa: Vec::new(),
            checked_at: Utc::now(),
            source: "dnsx".to_string(),
        };
        storage.save(&obs)?;
        Ok(obs)
    }
}
