use crate::storage::JsonlStorage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
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
    #[serde(default)]
    subject_dn: String,
    #[serde(default)]
    issuer_dn: String,
    #[serde(default)]
    not_before: Option<chrono::DateTime<Utc>>,
    #[serde(default)]
    not_after: Option<chrono::DateTime<Utc>>,
    #[serde(default)]
    fingerprint_sha256: String,
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
    
    tls: Option<HttpxTls>,
    
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

/// Helper to extract URL from httpx raw request string
fn extract_url_from_raw_request(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.is_empty() { return "unknown".to_string(); }
    
    // Line 0 is usually "GET /path HTTP/1.1"
    let first_line = lines[0];
    let path = first_line.split_whitespace().nth(1).unwrap_or("/");
    
    // Find Host header
    let host = lines.iter()
        .find(|l| l.to_lowercase().starts_with("host: "))
        .map(|l| l[6..].trim())
        .unwrap_or("unknown");
        
    // We assume HTTPS for everything unless we see 80 in host, 
    // but httpx usually uses 443 by default for these probes.
    format!("https://{}{}", host, path)
}

/// Mapper to map Httpx JSON data into HttpObservation
fn map_httpx_to_observation(parsed: HttpxOutput, original_target: &str) -> HttpObservation {
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
        // chain_status_codes are the redirects (3xx). 
        // chain contains the full requests.
        // If there are N redirects, there are N+1 items in the chain.
        for (i, &code) in codes.iter().enumerate() {
            if i < chain.len() {
                let current_url = extract_url_from_raw_request(chain[i].request.as_deref().unwrap_or(""));
                
                // The next item in the chain is where we redirected TO
                let next_url = if i + 1 < chain.len() {
                    extract_url_from_raw_request(chain[i+1].request.as_deref().unwrap_or(""))
                } else {
                    parsed.url.clone().unwrap_or_else(|| "unknown".to_string())
                };

                redirect_chain.push(RedirectHop {
                    url: current_url,
                    status: code,
                    redirect_to: next_url,
                });
            }
        }
    }

    let tls = if let Some(grab) = parsed.tls {
        if let (Some(before), Some(after)) = (grab.not_before, grab.not_after) {
            Some(TlsObservation {
                subject_dn: grab.subject_dn,
                issuer_dn: grab.issuer_dn,
                not_before: before,
                not_after: after,
                fingerprint_sha256: grab.fingerprint_sha256,
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
            target: original_target.to_string(),
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
        // Determine the true final URL: if there's a redirect chain with 3xx hops,
        // use the redirect_to of the last 3xx hop (where we actually ended up).
        // parsed.url is just the probed input URL, NOT the final destination.
        let final_url = redirect_chain.iter()
            .rev()
            .find(|hop| (300..400).contains(&(hop.status as u32)))
            .map(|hop| hop.redirect_to.clone())
            .or(parsed.url);

        HttpObservation {
            item_type: "http_check".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: original_target.to_string(),
            status: parsed.status_code,
            response_time_ms: duration_ms,
            final_url,
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

/// Progress callback type for reporting scan progress without printing to stdout.
pub type ProgressCallback = Arc<dyn Fn(usize, usize, &str) + Send + Sync>;

/// Run a batch HTTP probe using a single httpx process for maximum concurrency.
/// Optionally accepts a progress callback `on_progress(completed, total, target_name)`.
pub fn probe_http_batch(
    targets: &[String],
    storage: &JsonlStorage,
) -> Result<Vec<HttpObservation>, io::Error> {
    probe_http_batch_with_progress(targets, storage, None)
}

/// Like `probe_http_batch` but with an optional progress callback for TUI integration.
pub fn probe_http_batch_with_progress(
    targets: &[String],
    storage: &JsonlStorage,
    on_progress: Option<ProgressCallback>,
) -> Result<Vec<HttpObservation>, io::Error> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    check_httpx_installation()?;

    let total = targets.len();

    let mut child = Command::new("httpx")
        .args([
            "-json", 
            "-td",
            "-title",
            "-server",
            "-cdn",
            "-favicon",
            "-tls-probe",
            "-fc", "301,302,307,308",
            "-fr",
            "-include-chain",
            "-silent"
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

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

    let mut target_idx = 0;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(parsed) = serde_json::from_str::<HttpxOutput>(&line) {
            let input_copy = parsed.input.clone();
            let original = if target_idx < targets.len() {
                &targets[target_idx]
            } else {
                &input_copy
            };
            
            let obs = map_httpx_to_observation(parsed, original);
            storage.save(&obs)?;
            target_idx += 1;

            if let Some(ref cb) = on_progress {
                cb(target_idx, total, &obs.target);
            }

            observations.push(obs);
        }
    }

    let _ = child.wait()?;
    Ok(observations)
}

// Perform HTTP probe (single domain wrapper around batch)
pub fn http_check(domain: &str, storage: &JsonlStorage) -> Result<HttpObservation, io::Error> {
    check_httpx_installation()?;
    
    let clean_domain = domain
        .trim()
        .replace("http://", "")
        .replace("https://", "")
        .replace("www.", "");

    let output = Command::new("httpx")
        .args([
            "-u", &clean_domain,
            "-json", 
            "-td",
            "-title",
            "-server",
            "-cdn",
            "-favicon",
            "-tls-probe",
            "-fc", "301,302,307,308",
            "-fr",
            "-include-chain",
            "-silent"
        ])
        .output()?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout_str.trim();

    if trimmed.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "No output from httpx"));
    }

    // httpx might output multiple JSON lines if it probes multiple endpoints (e.g. root + www).
    // We take the last one as it usually represents the final state or the most complete probe.
    let mut last_parsed = None;
    for line in trimmed.lines() {
        if let Ok(p) = serde_json::from_str::<HttpxOutput>(line) {
            last_parsed = Some(p);
        }
    }

    let parsed = last_parsed.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "No valid JSON records in httpx output")
    })?;

    let mut obs = map_httpx_to_observation(parsed, domain);
    obs.target = domain.to_string(); 
    storage.save(&obs)?;
    Ok(obs)
}
