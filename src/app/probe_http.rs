use crate::storage::JsonlStorage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::time::{Duration, Instant};
use url::Url;
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
        .redirect(reqwest::redirect::Policy::none()) // We handle redirects manually
        .build()
        .map_err(|e| io::Error::other(e.to_string()))?;

    let start = Instant::now();

    let mut current_url = https_url.clone();
    let mut redirect_chain = Vec::new();
    let mut final_status = None;
    let mut error_result = None;
    let max_redirects = 10;
    
    // First, try HTTPS. If it fails on the very first request (e.g. connection refused), fallback to HTTP.
    let mut tried_http_fallback = false;

    for _ in 0..max_redirects {
        let response_result = client.get(&current_url).send();

        match response_result {
            Ok(response) => {
                let status = response.status().as_u16();

                if status >= 300 && status < 400 {
                    if let Some(location) = response.headers().get(reqwest::header::LOCATION) {
                        if let Ok(loc_str) = location.to_str() {
                            // Resolve relative URLs using the current URL as a base
                            let next_url = if let Ok(base) = Url::parse(&current_url) {
                                if let Ok(joined) = base.join(loc_str) {
                                    joined.to_string()
                                } else {
                                    loc_str.to_string()
                                }
                            } else {
                                loc_str.to_string()
                            };

                            redirect_chain.push(RedirectHop {
                                url: current_url.clone(),
                                status,
                                redirect_to: next_url.clone(),
                            });

                            current_url = next_url;
                            continue;
                        }
                    }
                }

                // If we get here, it's not a redirect or it's a redirect missing a Location header
                final_status = Some(status);
                break;
            }
            Err(error) => {
                if !tried_http_fallback && redirect_chain.is_empty() {
                    // Try HTTP fallback only if this is the very first request and HTTPS failed
                    tried_http_fallback = true;
                    current_url = http_url.clone();
                    // Don't break, continue loop to try HTTP
                } else {
                    // It failed during a redirect chain or after HTTP fallback
                    error_result = Some(error);
                    break;
                }
            }
        }
    }

    let duration = start.elapsed().as_millis() as u64;

    let observation = if error_result.is_none() && final_status.is_none() {
        HttpObservation {
            item_type: "http_check".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: clean_domain.to_string(),
            status: None,
            response_time_ms: duration,
            final_url: Some(current_url),
            redirect_chain,
            checked_at: Utc::now(),
            source: "probe_http".to_string(),
            error_kind: Some(HttpErrorKind::TooManyRedirects),
            error_message: Some("Too many redirects".to_string()),
        }
    } else if let Some(error) = error_result {
        let error_kind = categorize_request_error(&error);
        let error_message = error.to_string();

        HttpObservation {
            item_type: "http_check".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: clean_domain.to_string(),
            status: None,
            response_time_ms: duration,
            final_url: Some(current_url),
            redirect_chain,
            checked_at: Utc::now(),
            source: "probe_http".to_string(),
            error_kind: Some(error_kind),
            error_message: Some(error_message),
        }
    } else {
        HttpObservation {
            item_type: "http_check".to_string(),
            id: Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: clean_domain.to_string(),
            status: final_status,
            response_time_ms: duration,
            final_url: Some(current_url),
            redirect_chain,
            checked_at: Utc::now(),
            source: "probe_http".to_string(),
            error_kind: None,
            error_message: None,
        }
    };

    // Enforce invariant
    observation.validate_invariant();

    // Save to storage - ALWAYS, even on error
    storage.save(&observation)?;

    Ok(observation)
}
