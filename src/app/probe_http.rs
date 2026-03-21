use crate::storage::JsonlStorage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use uuid::Uuid;

// HTTP probe error types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpErrorKind {
    DnsNotFound,
    Timeout,
    ConnectionError,
    TlsError,
    ProbeError,
    TooManyRedirects,
}

// Represents a single hop in a redirect chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedirectHop {
    pub url: String,
    pub status: u16,
    pub redirect_to: String,
}

// TLS certificate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsObservation {
    pub subject_dn: String,
    pub issuer_dn: String,
    pub not_before: chrono::DateTime<Utc>,
    pub not_after: chrono::DateTime<Utc>,
    pub fingerprint_sha256: String,
}

// HTTP observation - always describes what happened (success or failure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpObservation {
    pub item_type: String,
    pub id: String,
    pub asset_type: String,
    pub target: String,

    // Result - XOR: either status or error, never both
    pub status: Option<u16>,
    pub response_time_ms: u64,

    // Redirect information
    #[serde(default)]
    pub final_url: Option<String>,
    #[serde(default)]
    pub redirect_chain: Vec<RedirectHop>,

    // TLS information
    #[serde(default)]
    pub tls: Option<TlsObservation>,

    // Fingerprinting and tech details
    #[serde(default)]
    pub tech: Vec<String>,
    #[serde(default)]
    pub webserver: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub cdn_name: Option<String>,
    #[serde(default)]
    pub favicon_hash: Option<String>,

    // Metadata
    pub checked_at: chrono::DateTime<Utc>,
    pub source: String,

    // Network error details (if any)
    pub error_kind: Option<HttpErrorKind>,
    pub error_message: Option<String>,
}

impl HttpObservation {
    // Enforce invariant: exactly one of (status, error_kind) must be Some
    fn validate_invariant(&self) {
        debug_assert!(
            self.status.is_some() ^ self.error_kind.is_some(),
            "HttpObservation must have exactly one of status or error_kind set (XOR)"
        );
    }
}

// DTO for httpx JSON output
#[derive(Deserialize, Debug)]
struct HttpxChainItem {
    request: Option<String>,
}

#[derive(Deserialize, Debug)]
struct HttpxTls {
    #[serde(rename = "subject_dn")]
    subject_dn: Option<String>,
    #[serde(rename = "issuer_dn")]
    issuer_dn: Option<String>,
    #[serde(rename = "not_before")]
    not_before: Option<chrono::DateTime<Utc>>,
    #[serde(rename = "not_after")]
    not_after: Option<chrono::DateTime<Utc>>,
    #[serde(rename = "fingerprint_sha256")]
    fingerprint_sha256: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct HttpxOutput {
    input: String,
    url: Option<String>,
    status_code: Option<u16>,
    time: Option<String>, // "100ms"
    failed: Option<bool>,
    error: Option<String>,
    
    tech: Option<Vec<String>>,
    webserver: Option<String>,
    title: Option<String>,
    cdn_name: Option<String>,
    favicon: Option<String>,
    
    #[serde(rename = "tls-grab")]
    tls_grab: Option<HttpxTls>,
    
    chain_status_codes: Option<Vec<u16>>,
    chain: Option<Vec<HttpxChainItem>>,
}

// Helper to convert httpx error strings to our enum
fn categorize_httpx_error(error_msg: &str) -> HttpErrorKind {
    let msg = error_msg.to_lowercase();
    if msg.contains("no address found") || msg.contains("dial") || msg.contains("dns") {
        HttpErrorKind::DnsNotFound
    } else if msg.contains("timeout") || msg.contains("context deadline exceeded") {
        HttpErrorKind::Timeout
    } else if msg.contains("tls") || msg.contains("certificate") {
        HttpErrorKind::TlsError
    } else if msg.contains("connection refused") || msg.contains("reset") {
        HttpErrorKind::ConnectionError
    } else {
        HttpErrorKind::ProbeError
    }
}

/// Helper function to check if httpx is correctly installed from ProjectDiscovery.
fn check_httpx_installation() -> Result<(), io::Error> {
    let version_check = Command::new("httpx")
        .arg("-version")
        .output()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "httpx not found in PATH. Please install it from ProjectDiscovery: https://github.com/projectdiscovery/httpx",
                )
            } else {
                e
            }
        })?;

    if !String::from_utf8_lossy(&version_check.stdout).contains("projectdiscovery") 
        && !String::from_utf8_lossy(&version_check.stderr).contains("projectdiscovery") {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "The 'httpx' command found in PATH appears to be the Python HTTP client, not the ProjectDiscovery tool. Please ensure the ProjectDiscovery httpx takes precedence.",
        ));
    }
    
    Ok(())
}

/// Mapper to map Httpx JSON data into HttpObservation
fn map_httpx_to_observation(parsed: HttpxOutput) -> HttpObservation {
    let target = parsed.input.clone();
    
    // Convert duration strings like "108.04ms" or "1.5s" to ms
    let duration_ms = if let Some(t) = &parsed.time {
        if t.ends_with("ms") {
            t.trim_end_matches("ms").parse::<f64>().unwrap_or(0.0) as u64
        } else if t.ends_with("s") {
            (t.trim_end_matches("s").parse::<f64>().unwrap_or(0.0) * 1000.0) as u64
        } else {
            0
        }
    } else {
        0
    };

    let mut redirect_chain = Vec::new();
    if let (Some(codes), Some(chain)) = (&parsed.chain_status_codes, &parsed.chain) {
        for (i, code) in codes.iter().enumerate() {
            if i < chain.len() {
                let raw_req = chain[i].request.clone().unwrap_or_default();
                let url = if let Some(host_line) = raw_req.lines().find(|l| l.starts_with("Host: ")) {
                    format!("https://{}", host_line.replace("Host: ", "").trim())
                } else {
                    format!("Hop #{}", i + 1)
                };

                let redirect_to = if i + 1 < chain.len() {
                    let next_req = chain[i + 1].request.clone().unwrap_or_default();
                    if let Some(host_line) = next_req.lines().find(|l| l.starts_with("Host: ")) {
                        format!("https://{}", host_line.replace("Host: ", "").trim())
                    } else {
                        parsed.url.clone().unwrap_or_default()
                    }
                } else {
                    parsed.url.clone().unwrap_or_default()
                };

                redirect_chain.push(RedirectHop {
                    url,
                    status: *code,
                    redirect_to,
                });
            }
        }
    }

    let tls = if let Some(grab) = parsed.tls_grab {
        if let (Some(subj), Some(issuer), Some(before), Some(after), Some(fp)) = 
            (grab.subject_dn, grab.issuer_dn, grab.not_before, grab.not_after, grab.fingerprint_sha256) {
            Some(TlsObservation {
                subject_dn: subj,
                issuer_dn: issuer,
                not_before: before,
                not_after: after,
                fingerprint_sha256: fp,
            })
        } else {
            None
        }
    } else {
        None
    };

    let is_failed = parsed.failed.unwrap_or(false);

    let obs = if is_failed || parsed.status_code.is_none() {
        let error_msg = parsed.error.unwrap_or_else(|| "Unknown error".to_string());
        let error_kind = categorize_httpx_error(&error_msg);

        HttpObservation {
            item_type: "http_check".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target,
            status: None,
            response_time_ms: duration_ms,
            final_url: None,
            redirect_chain: Vec::new(),
            tls,
            tech: Vec::new(),
            webserver: None,
            title: None,
            cdn_name: None,
            favicon_hash: None,
            checked_at: Utc::now(),
            source: "httpx".to_string(),
            error_kind: Some(error_kind),
            error_message: Some(error_msg),
        }
    } else {
        HttpObservation {
            item_type: "http_check".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target,
            status: parsed.status_code,
            response_time_ms: duration_ms,
            final_url: parsed.url,
            redirect_chain,
            tls,
            tech: parsed.tech.unwrap_or_default(),
            webserver: parsed.webserver,
            title: parsed.title,
            cdn_name: parsed.cdn_name,
            favicon_hash: parsed.favicon,
            checked_at: Utc::now(),
            source: "httpx".to_string(),
            error_kind: None,
            error_message: None,
        }
    };

    obs.validate_invariant();
    obs
}

/// Run a batch HTTP probe using a single httpx process for maximum concurrency.
pub fn probe_http_batch(
    targets: &[String],
    storage: &JsonlStorage,
) -> Result<Vec<HttpObservation>, io::Error> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    check_httpx_installation()?;

    let mut child = Command::new("httpx")
        .args([
            "-json", 
            "-td",       // Tech detect
            "-title",    // Page title
            "-server",   // Web server
            "-cdn",      // CDN name
            "-favicon",  // Favicon hash
            "-tls-probe", // TLS certificate details
            "-fc", "301,302,307,308", // Filter out just pure redirects without following
            "-fr",       // Follow redirects
            "-include-chain", // Include the redirect chain in JSON
            "-silent"
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Send targets to the standard input via a separate thread
    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut clean_targets = Vec::new();
    for target in targets {
        let clean = target
            .trim()
            .replace("http://", "")
            .replace("https://", "")
            .replace("www.", "");
        clean_targets.push(clean);
    }
    
    let targets_string = clean_targets.join("\n") + "\n";
    
    thread::spawn(move || {
        let _ = stdin.write_all(targets_string.as_bytes());
    });

    let stdout = child.stdout.take().expect("Failed to open stdout");
    let reader = BufReader::new(stdout);
    let mut observations = Vec::new();

    // Stream and process results on the fly
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(parsed) = serde_json::from_str::<HttpxOutput>(&line) {
            let obs = map_httpx_to_observation(parsed);
            storage.save(&obs)?;
            observations.push(obs);
            
            // Print a progress indicator
            print!(".");
            let _ = io::stdout().flush();
        }
    }

    let _ = child.wait()?;
    Ok(observations)
}

// Perform HTTP probe (single domain wrapper around batch)
pub fn http_check(domain: &str, storage: &JsonlStorage) -> Result<HttpObservation, io::Error> {
    let targets = vec![domain.to_string()];
    let mut results = probe_http_batch(&targets, storage)?;
    
    if let Some(obs) = results.pop() {
        Ok(obs)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "No output from httpx (unreachable)".to_string(),
        ))
    }
}
