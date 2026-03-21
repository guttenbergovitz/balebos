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
    #[serde(default)]
    pub mx: Vec<String>,
    #[serde(default)]
    pub ns: Vec<String>,
    #[serde(default)]
    pub txt: Vec<String>,
    #[serde(default)]
    pub soa: Vec<String>,

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
    #[serde(default)]
    mx: Vec<String>,
    #[serde(default)]
    ns: Vec<String>,
    #[serde(default)]
    txt: Vec<String>,
    #[serde(default)]
    soa: Vec<String>,
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
        .args(["-json", "-a", "-aaaa", "-cname", "-mx", "-ns", "-txt", "-soa", "-silent"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let targets_string = targets.join("\n") + "\n";
    
    thread::spawn(move || {
        let _ = stdin.write_all(targets_string.as_bytes());
    });

    let stdout = child.stdout.take().expect("Failed to open stdout");
    let reader = BufReader::new(stdout);
    let mut observations = Vec::new();

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
                    mx: parsed.mx,
                    ns: parsed.ns,
                    txt: parsed.txt,
                    soa: parsed.soa,
                    checked_at: Utc::now(),
                    source: "dnsx".to_string(),
                };
                storage.save(&obs)?;
                observations.push(obs);
                
                print!(".");
                let _ = io::stdout().flush();
            }
            Err(_) => continue,
        }
    }

    let _ = child.wait()?;
    Ok(observations)
}

pub fn probe_dns(target: &str, storage: &JsonlStorage) -> Result<DnsObservation, io::Error> {
    let targets = vec![target.to_string()];
    let mut results = probe_dns_batch(&targets, storage)?;
    
    if let Some(obs) = results.pop() {
        Ok(obs)
    } else {
        let obs = DnsObservation {
            item_type: "dns_record".to_string(),
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
