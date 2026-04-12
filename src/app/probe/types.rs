use crate::inventory::ItemType;
use chrono::Utc;
use serde::{Deserialize, Serialize};

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
    pub item_type: ItemType,
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
    pub fn validate_invariant(&self) {
        debug_assert!(
            self.status.is_some() ^ self.error_kind.is_some(),
            "HttpObservation must have exactly one of status or error_kind set (XOR)"
        );
    }
}

// DNS observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsObservation {
    pub item_type: ItemType,
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

// ASN observation - from asnmap probe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsnObservation {
    pub item_type: ItemType,
    pub id: String,
    pub target: String, // IP address that was looked up
    pub host: String,   // Original hostname (for correlation)

    pub asn: u32,
    pub org: String,
    pub country: String,

    pub checked_at: chrono::DateTime<Utc>,
    pub source: String,
}

// Confidence level for hosting detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
    Confirmed,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::Low => write!(f, "Low"),
            Confidence::Medium => write!(f, "Medium"),
            Confidence::High => write!(f, "High"),
            Confidence::Confirmed => write!(f, "Confirmed"),
        }
    }
}

// How the hosting provider was detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    CdnName,
    Cname,
    Asn,
    Header,
    Combined,
}

impl std::fmt::Display for DetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectionMethod::CdnName => write!(f, "CDN Name"),
            DetectionMethod::Cname => write!(f, "CNAME"),
            DetectionMethod::Asn => write!(f, "ASN"),
            DetectionMethod::Header => write!(f, "Header"),
            DetectionMethod::Combined => write!(f, "Combined"),
        }
    }
}

// Hosting detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingObservation {
    pub item_type: ItemType,
    pub id: String,
    pub target: String,

    // Provider info (UNKNOWN if cannot be determined)
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub provider_category: Option<String>,

    // CDN info (if behind CDN)
    pub cdn_detected: bool,
    pub cdn_name: Option<String>,

    // Detection details
    pub confidence: Confidence,
    pub detection_method: DetectionMethod,

    // Evidence trail for debugging
    pub evidence: Vec<String>,

    pub checked_at: chrono::DateTime<Utc>,
    pub source: String,
}
