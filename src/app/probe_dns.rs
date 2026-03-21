use crate::storage::JsonlStorage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::io::Write;
use std::process::{Command, Stdio};
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

pub fn probe_dns(target: &str, storage: &JsonlStorage) -> Result<DnsObservation, io::Error> {
    let mut child = Command::new("dnsx")
        .args(["-json", "-a", "-aaaa", "-cname", "-silent"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
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

    // Write the target domain to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(format!("{}\n", target).as_bytes())?;
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "dnsx failed with status: {}. Error: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    
    // Check if dnsx actually found something. If not, it outputs empty line or nothing.
    let trimmed = stdout_str.trim();
    if trimmed.is_empty() {
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
        return Ok(obs);
    }

    // Try to parse the JSON output from dnsx
    match serde_json::from_str::<DnsxOutput>(trimmed) {
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
            Ok(obs)
        }
        Err(e) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to parse dnsx JSON output: {}", e),
        )),
    }
}
