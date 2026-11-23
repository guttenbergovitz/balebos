use super::process;
use super::types::DnsObservation;
use crate::error::Result;
use crate::inventory::ItemType;
use crate::storage::{StorageEngine, StorageEngineExt};
use chrono::Utc;
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
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
    #[serde(default)]
    srv: Vec<String>,
}

/// Helper function to check if dnsx is correctly installed.
fn check_dnsx_installation() -> Result<()> {
    process::run_bounded(
        Command::new("dnsx").arg("-version"),
        std::time::Duration::from_secs(10),
        process::DEFAULT_MAX_OUTPUT,
    )?;
    Ok(())
}

fn probe_dns_batch_inner(
    targets: &[String],
    save_to_storage: bool,
    storage: Option<&Arc<dyn StorageEngine>>,
    on_target: Option<&dyn Fn(&str)>,
) -> Result<Vec<DnsObservation>> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    check_dnsx_installation()?;

    let mut all_observations = Vec::new();
    let chunk_size = 100;

    for (chunk_idx, chunk) in targets.chunks(chunk_size).enumerate() {
        let total_chunks = (targets.len() as f32 / chunk_size as f32).ceil() as usize;
        // Skip printing in TUI - status is handled by the caller
        if !super::is_quiet() {
            println!(
                "    ⟳ DNS: Processing chunk {}/{} ({} targets)...",
                chunk_idx + 1,
                total_chunks,
                chunk.len()
            );
        }

        // Use a temporary file for targets to avoid pipe issues
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("balebos_dns_{}.txt", Uuid::new_v4()));
        fs::write(&temp_file, chunk.join("\n"))?;

        let mut child = Command::new("dnsx")
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
                "-srv",
                "-silent",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("failed to open dnsx stdout pipe"))?;
        let mut observations = Vec::new();
        let reader = BufReader::new(stdout);

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
                        srv: parsed.srv,
                        checked_at: Utc::now(),
                        source: "dnsx".to_string(),
                    };
                    if let Some(cb) = on_target {
                        cb(&obs.target);
                    }
                    observations.push(obs);
                }
                Err(e) => {
                    eprintln!("Failed to parse dnsx output line: {}. Error: {}", line, e);
                }
            }
        }

        let _ = process::wait_bounded(
            child,
            std::time::Duration::from_secs(300),
            process::DEFAULT_MAX_OUTPUT,
        );
        let _ = fs::remove_file(&temp_file);

        if save_to_storage && !observations.is_empty() {
            if let Some(s) = storage {
                s.save_batch(&observations)?;
            }
        }
        all_observations.extend(observations);
    }

    Ok(all_observations)
}

/// Run a batch DNS probe using a single dnsx process for efficiency.
pub fn probe_dns_batch(
    targets: &[String],
    storage: &Arc<dyn StorageEngine>,
) -> Result<Vec<DnsObservation>> {
    probe_dns_batch_inner(targets, true, Some(storage), None)
}

/// Run a batch DNS probe while streaming each resolved target to `on_target`.
pub fn probe_dns_batch_with_progress(
    targets: &[String],
    storage: &Arc<dyn StorageEngine>,
    on_target: &dyn Fn(&str),
) -> Result<Vec<DnsObservation>> {
    probe_dns_batch_inner(targets, true, Some(storage), Some(on_target))
}

/// Run a batch DNS probe without saving to storage.
pub fn probe_dns_batch_no_save(targets: &[String]) -> Result<Vec<DnsObservation>> {
    probe_dns_batch_inner(targets, false, None, None)
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
            srv: Vec::new(),
            checked_at: Utc::now(),
            source: "dnsx".to_string(),
        };
        storage.save(&obs)?;
        Ok(obs)
    }
}
