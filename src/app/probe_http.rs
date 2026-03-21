use crate::storage::JsonlStorage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::io::Write;
use std::process::{Command, Stdio};
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

// Perform HTTP probe using external httpx binary
pub fn http_check(domain: &str, storage: &JsonlStorage) -> Result<HttpObservation, io::Error> {
    let clean_domain = domain
        .trim()
        .replace("http://", "")
        .replace("https://", "")
        .replace("www.", "");

    // Verify httpx is the ProjectDiscovery version, not the Python package
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

    if !String::from_utf8_lossy(&version_check.stdout).contains("projectdiscovery") && !String::from_utf8_lossy(&version_check.stderr).contains("projectdiscovery") {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "The 'httpx' command found in PATH appears to be the Python HTTP client, not the ProjectDiscovery tool. Please ensure the ProjectDiscovery httpx (often installed in ~/go/bin/) takes precedence in your PATH or alias it appropriately.",
        ));
    }

    let start = std::time::Instant::now();

    let mut child = Command::new("httpx")
        .args([
            "-json", 
            "-td",       // Tech detect
            "-title",    // Page title
            "-server",   // Web server
            "-cdn",      // CDN name
            "-favicon",  // Favicon hash
            "-fc", "301,302,307,308", // HTTP status codes to filter when following
            "-fr",       // Follow redirects
            "-include-chain", // Include the redirect chain in JSON
            "-silent"
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(format!("{}\n", clean_domain).as_bytes())?;
    }

    let output = child.wait_with_output()?;

    let duration_ms = start.elapsed().as_millis() as u64;

    // Check if process failed execution
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "httpx failed with status: {}. Error: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout_str.trim();

    // If httpx returns empty string, it silently failed to connect or found nothing
    if trimmed.is_empty() {
        let obs = HttpObservation {
            item_type: "http_check".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: clean_domain.to_string(),
            status: None,
            response_time_ms: duration_ms,
            final_url: None,
            redirect_chain: Vec::new(),
            tech: Vec::new(),
            webserver: None,
            title: None,
            cdn_name: None,
            favicon_hash: None,
            checked_at: Utc::now(),
            source: "httpx".to_string(),
            error_kind: Some(HttpErrorKind::ProbeError),
            error_message: Some("No output from httpx (unreachable)".to_string()),
        };
        obs.validate_invariant();
        storage.save(&obs)?;
        return Ok(obs);
    }

    // Parse httpx output
    let parsed: HttpxOutput = serde_json::from_str(trimmed).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to parse httpx JSON output: {}", e),
        )
    })?;

    // Map chain to our RedirectHop structure. httpx chain items are raw HTTP requests sometimes.
    let mut redirect_chain = Vec::new();
    if let (Some(codes), Some(chain)) = (&parsed.chain_status_codes, &parsed.chain) {
        for (i, code) in codes.iter().enumerate() {
            if i < chain.len() {
                // Extract URL from raw HTTP request if present, or fallback
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

    let is_failed = parsed.failed.unwrap_or(false);

    let observation = if is_failed || parsed.status_code.is_none() {
        let error_msg = parsed.error.unwrap_or_else(|| "Unknown error".to_string());
        let error_kind = categorize_httpx_error(&error_msg);

        HttpObservation {
            item_type: "http_check".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: clean_domain.to_string(),
            status: None,
            response_time_ms: duration_ms,
            final_url: None,
            redirect_chain: Vec::new(),
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
            target: clean_domain.to_string(),
            status: parsed.status_code,
            response_time_ms: duration_ms,
            final_url: parsed.url,
            redirect_chain,
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

    observation.validate_invariant();
    storage.save(&observation)?;

    Ok(observation)
}
