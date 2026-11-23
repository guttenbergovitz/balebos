use super::process;
use super::types::{HttpErrorKind, HttpObservation, RedirectHop, TlsObservation};
use crate::error::{ProbeError, Result};
use crate::inventory::ItemType;
use crate::storage::{StorageEngine, StorageEngineExt};
use chrono::Utc;
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use uuid::Uuid;

// DTO for httpx JSON output
#[derive(Deserialize, Debug, Clone)]
struct HttpxChainItem {
    request: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
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

#[derive(Deserialize, Debug, Clone)]
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
fn check_httpx_installation() -> Result<()> {
    let version_check = process::run_bounded(
        Command::new("httpx").arg("-version"),
        std::time::Duration::from_secs(10),
        process::DEFAULT_MAX_OUTPUT,
    )?;

    if !String::from_utf8_lossy(&version_check.stdout).contains("projectdiscovery")
        && !String::from_utf8_lossy(&version_check.stderr).contains("projectdiscovery")
    {
        return Err(ProbeError::InvalidOutput {
            tool: "httpx".to_string(),
            reason: "The 'httpx' command found in PATH appears to be the Python HTTP client, not the ProjectDiscovery tool. Please ensure the ProjectDiscovery httpx takes precedence.".to_string(),
        }.into());
    }

    Ok(())
}

/// Helper to extract URL from httpx raw request string
fn extract_url_from_raw_request(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.is_empty() {
        return "unknown".to_string();
    }

    // Line 0 is usually "GET /path HTTP/1.1"
    let first_line = lines[0];
    let path = first_line.split_whitespace().nth(1).unwrap_or("/");

    // Find Host header
    let host = lines
        .iter()
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
                let current_url =
                    extract_url_from_raw_request(chain[i].request.as_deref().unwrap_or(""));

                // The next item in the chain is where we redirected TO
                let next_url = if i + 1 < chain.len() {
                    extract_url_from_raw_request(chain[i + 1].request.as_deref().unwrap_or(""))
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
            item_type: ItemType::HttpCheck,
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
        let final_url = redirect_chain
            .iter()
            .rev()
            .find(|hop| (300..400).contains(&(hop.status as u32)))
            .map(|hop| hop.redirect_to.clone())
            .or(parsed.url);

        HttpObservation {
            item_type: ItemType::HttpCheck,
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
    storage: &Arc<dyn StorageEngine>,
) -> Result<Vec<HttpObservation>> {
    probe_http_batch_with_progress(targets, storage, None)
}

fn probe_http_batch_with_progress_inner(
    targets: &[String],
    save_to_storage: bool,
    storage: Option<&Arc<dyn StorageEngine>>,
    on_progress: Option<ProgressCallback>,
) -> Result<Vec<HttpObservation>> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    check_httpx_installation()?;

    let mut all_observations = Vec::new();
    let chunk_size = 100;

    for (chunk_idx, chunk) in targets.chunks(chunk_size).enumerate() {
        let total_chunks = (targets.len() as f32 / chunk_size as f32).ceil() as usize;
        // Skip printing in TUI - status is handled by the caller
        if std::env::var("BALEBOS_STORAGE").is_err() {
            println!(
                "    ⟳ HTTP: Processing chunk {}/{} ({} targets)...",
                chunk_idx + 1,
                total_chunks,
                chunk.len()
            );
        }

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
                "-silent",
                "-H", "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
                "-H", "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
                "-x", "get",
                "-retries", "2",
                "-timeout", "10",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().ok_or(ProbeError::ToolFailed {
            tool: "httpx".to_string(),
            stderr: "failed to open stdin pipe".to_string(),
        })?;

        let mut clean_targets = Vec::new();
        for target in chunk {
            let clean = target.trim().replace("http://", "").replace("https://", "");
            clean_targets.push(clean);
        }

        let targets_string = clean_targets.join("\n") + "\n";

        thread::spawn(move || {
            let _ = stdin.write_all(targets_string.as_bytes());
        });

        let stdout = child.stdout.take().ok_or(ProbeError::ToolFailed {
            tool: "httpx".to_string(),
            stderr: "failed to open stdout pipe".to_string(),
        })?;
        let reader = BufReader::new(stdout);
        let mut observations = Vec::new();

        for (line_idx, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(parsed) = serde_json::from_str::<HttpxOutput>(&line) {
                let input_from_httpx = parsed.input.clone();

                // Match input to the original target to preserve its casing/format
                let original = chunk
                    .iter()
                    .find(|&t| {
                        let clean = t.trim().replace("http://", "").replace("https://", "");
                        clean == input_from_httpx
                    })
                    .map(|s| s.as_str())
                    .unwrap_or(&input_from_httpx);

                let obs = map_httpx_to_observation(parsed, original);

                if let Some(ref cb) = on_progress {
                    // Global progress calculation if needed, here we just use chunk local
                    cb(line_idx + 1, chunk.len(), &obs.target);
                }

                observations.push(obs);
            }
        }

        if save_to_storage && !observations.is_empty() {
            if let Some(s) = storage {
                s.save_batch(&observations)?;
            }
        }
        all_observations.extend(observations);

        // Backstop envelope: the stdout reader above finishes when httpx
        // closes its output; if the process hangs regardless, kill and reap.
        let _ = process::wait_bounded(
            child,
            std::time::Duration::from_secs(300),
            process::DEFAULT_MAX_OUTPUT,
        );
    }

    Ok(all_observations)
}

/// Like `probe_http_batch` but with an optional progress callback for TUI integration.
pub fn probe_http_batch_with_progress(
    targets: &[String],
    storage: &Arc<dyn StorageEngine>,
    on_progress: Option<ProgressCallback>,
) -> Result<Vec<HttpObservation>> {
    probe_http_batch_with_progress_inner(targets, true, Some(storage), on_progress)
}

/// Run HTTP batch probe without saving to storage.
pub fn probe_http_batch_no_save(targets: &[String]) -> Result<Vec<HttpObservation>> {
    probe_http_batch_with_progress_inner(targets, false, None, None)
}

// Perform HTTP probe (single domain wrapper around batch)
pub fn http_check(domain: &str, storage: &Arc<dyn StorageEngine>) -> Result<HttpObservation> {
    check_httpx_installation()?;

    let clean_domain = domain.trim().replace("http://", "").replace("https://", "");

    let output = process::run_bounded(
        Command::new("httpx").args([
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
            "-silent",
            "-H", "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
            "-H", "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
            "-x", "get",
            "-retries", "2",
            "-timeout", "10",
        ]),
        std::time::Duration::from_secs(300),
        process::DEFAULT_MAX_OUTPUT,
    )?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout_str.trim();

    if trimmed.is_empty() {
        return Err(ProbeError::InvalidOutput {
            tool: "httpx".to_string(),
            reason: "No output from httpx".to_string(),
        }
        .into());
    }

    // httpx might output multiple JSON lines if it probes multiple endpoints (e.g. root + www).
    // We want to pick the one that matches our requested domain best, preferably a successful one.
    let mut results = Vec::new();
    for line in trimmed.lines() {
        if let Ok(p) = serde_json::from_str::<HttpxOutput>(line) {
            results.push(p);
        }
    }

    if results.is_empty() {
        return Err(ProbeError::InvalidOutput {
            tool: "httpx".to_string(),
            reason: "No valid JSON records in httpx output".to_string(),
        }
        .into());
    }

    // Pick the best result: prefer 200, then any status code, then whatever matches input
    let parsed = results
        .iter()
        .find(|r| r.status_code == Some(200))
        .or_else(|| results.iter().find(|r| r.status_code.is_some()))
        .unwrap_or(&results[results.len() - 1]);

    let mut obs = map_httpx_to_observation(parsed.clone(), domain);
    obs.target = domain.to_string();
    storage.save(&obs)?;
    Ok(obs)
}
