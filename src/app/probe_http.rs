use crate::storage::JsonlStorage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::time::{Duration, Instant};
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

// Categorize reqwest error into our enum
// Order matters: DNS must be checked before connection errors
fn categorize_request_error(error: &reqwest::Error) -> HttpErrorKind {
    let msg = error.to_string().to_lowercase();

    // 1. Check error message - DNS errors have characteristic keywords
    if msg.contains("dns") || msg.contains("resolve") || msg.contains("lookup") {
        return HttpErrorKind::DnsNotFound;
    }

    // 2. Timeout has its own method in reqwest
    if error.is_timeout() {
        return HttpErrorKind::Timeout;
    }

    // 3. TLS/Certificate errors
    if msg.contains("tls") || msg.contains("certificate") || msg.contains("ssl") {
        return HttpErrorKind::TlsError;
    }

    // 4. Connection errors
    if error.is_connect() {
        return HttpErrorKind::ConnectionError;
    }

    // 5. Everything else
    HttpErrorKind::ProbeError
}

// Perform HTTP probe and always return an observation
// Never returns Err for network problems - network errors are data, not exceptions
// Err is only returned for internal errors (storage, client build)
pub fn http_check(domain: &str, storage: &JsonlStorage) -> Result<HttpObservation, io::Error> {
    let clean_domain = domain
        .trim()
        .replace("http://", "")
        .replace("https://", "")
        .replace("www.", "");

    let https_url = format!("https://{}", clean_domain);
    let http_url = format!("http://{}", clean_domain);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let start = Instant::now();

    // Try HTTPS first, fallback to HTTP
    let response_result = match client.get(&https_url).send() {
        Ok(resp) => Ok(resp),
        Err(_https_err) => {
            // HTTPS failed, don't log here - it will be in the observation
            match client.get(&http_url).send() {
                Ok(resp) => Ok(resp),
                Err(http_err) => Err(http_err),
            }
        }
    };

    let duration = start.elapsed().as_millis() as u64;

    // Create observation - always Ok(observation), never Err for network issues
    let observation = match response_result {
        Ok(response) => {
            let status = response.status().as_u16();
            HttpObservation {
                item_type: "http_check".to_string(),
                id: Uuid::new_v4().to_string(),
                asset_type: "domain".to_string(),
                target: clean_domain.to_string(),
                status: Some(status),
                response_time_ms: duration,
                checked_at: Utc::now(),
                source: "probe_http".to_string(),
                error_kind: None,
                error_message: None,
            }
        }
        Err(error) => {
            let error_kind = categorize_request_error(&error);
            let error_message = error.to_string();

            HttpObservation {
                item_type: "http_check".to_string(),
                id: Uuid::new_v4().to_string(),
                asset_type: "domain".to_string(),
                target: clean_domain.to_string(),
                status: None,
                response_time_ms: duration,
                checked_at: Utc::now(),
                source: "probe_http".to_string(),
                error_kind: Some(error_kind),
                error_message: Some(error_message),
            }
        }
    };

    // Enforce invariant
    observation.validate_invariant();

    // Save to storage - ALWAYS, even on error
    storage.save(&observation)?;

    Ok(observation)
}
