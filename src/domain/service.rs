//! Domain types for externally observable network services, endpoint
//! fingerprints, and application infrastructure references (ADRs 0026–0029).
//!
//! Pure data structures with no storage, network or UI dependencies.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::normalize::normalize_fqdn;
use crate::domain::relationship::{EntityKind, RelationshipKind};
use crate::inventory::ItemType;

/// Transport layer protocol for a network service endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportProtocol {
    Tcp,
    Udp,
}

impl fmt::Display for TransportProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcp => write!(f, "tcp"),
            Self::Udp => write!(f, "udp"),
        }
    }
}

/// Canonical socket endpoint identity (`host_or_ip:port/transport`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SocketEndpoint {
    pub host_or_ip: String,
    pub port: u16,
    pub transport: TransportProtocol,
}

impl SocketEndpoint {
    pub fn new(host_or_ip: impl Into<String>, port: u16, transport: TransportProtocol) -> Self {
        let raw = host_or_ip.into();
        let trimmed = raw.trim().to_ascii_lowercase();
        let canonical = if trimmed.parse::<std::net::IpAddr>().is_ok() {
            trimmed
        } else {
            normalize_fqdn(&trimmed)
        };
        Self {
            host_or_ip: canonical,
            port,
            transport,
        }
    }

    /// Canonical identifier used as `EntityRef::id` for `EntityKind::NetworkService`.
    pub fn canonical_id(&self) -> String {
        format!("{}:{}/{}", self.host_or_ip, self.port, self.transport)
    }
}

impl fmt::Display for SocketEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical_id())
    }
}

/// Recognised application/service protocols.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationProtocol {
    Http,
    Https,
    Ssh,
    Ftp,
    Ftps,
    Smtp,
    Imap,
    Pop3,
    Dns,
    Postgresql,
    Mysql,
    Mongodb,
    Redis,
    Elasticsearch,
    Mssql,
    Mqtt,
    Amqp,
    Kafka,
    Vpn,
    Other(String),
}

impl fmt::Display for ApplicationProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http => write!(f, "HTTP"),
            Self::Https => write!(f, "HTTPS"),
            Self::Ssh => write!(f, "SSH"),
            Self::Ftp => write!(f, "FTP"),
            Self::Ftps => write!(f, "FTPS"),
            Self::Smtp => write!(f, "SMTP"),
            Self::Imap => write!(f, "IMAP"),
            Self::Pop3 => write!(f, "POP3"),
            Self::Dns => write!(f, "DNS"),
            Self::Postgresql => write!(f, "PostgreSQL"),
            Self::Mysql => write!(f, "MySQL"),
            Self::Mongodb => write!(f, "MongoDB"),
            Self::Redis => write!(f, "Redis"),
            Self::Elasticsearch => write!(f, "Elasticsearch"),
            Self::Mssql => write!(f, "MSSQL"),
            Self::Mqtt => write!(f, "MQTT"),
            Self::Amqp => write!(f, "AMQP"),
            Self::Kafka => write!(f, "Kafka"),
            Self::Vpn => write!(f, "VPN"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Distinguishes bare port hypotheses from banner or verified protocol evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolIdentification {
    /// Port number alone suggests a candidate protocol (never authoritative).
    PortHypothesis { guessed: ApplicationProtocol },
    /// External service index (Censys/Shodan) reported a protocol or banner.
    IndexedBanner {
        protocol: ApplicationProtocol,
        banner: String,
    },
    /// Protocol-specific response observed by an authorised check.
    ObservedProtocol {
        protocol: ApplicationProtocol,
        evidence: String,
    },
    /// Observed/indexed protocol disagrees with the well-known port number.
    Mismatch {
        port_implies: ApplicationProtocol,
        observed: ApplicationProtocol,
        evidence: String,
    },
    /// No protocol identification available.
    Unidentified,
}

impl ProtocolIdentification {
    /// Returns the primary protocol label if one has been hypothesised or identified.
    pub fn protocol(&self) -> Option<&ApplicationProtocol> {
        match self {
            Self::PortHypothesis { guessed } => Some(guessed),
            Self::IndexedBanner { protocol, .. } => Some(protocol),
            Self::ObservedProtocol { protocol, .. } => Some(protocol),
            Self::Mismatch { observed, .. } => Some(observed),
            Self::Unidentified => None,
        }
    }

    /// True only when protocol identification goes beyond a bare port guess.
    pub fn is_corroborated(&self) -> bool {
        matches!(
            self,
            Self::IndexedBanner { .. } | Self::ObservedProtocol { .. } | Self::Mismatch { .. }
        )
    }
}

/// Observed state of a network service at the time of its measurement/indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceObservationState {
    Open,
    Filtered,
    Closed,
    HistoricalOnly,
    Unknown,
}

impl fmt::Display for ServiceObservationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Filtered => write!(f, "filtered"),
            Self::Closed => write!(f, "closed"),
            Self::HistoricalOnly => write!(f, "historical_only"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Functional category of a discovered service or candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceCategory {
    NetworkService,
    Database,
    Messaging,
    ManagementInterface,
    ApiEndpoint,
    Storage,
    IdentityProvider,
}

impl fmt::Display for ServiceCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkService => write!(f, "network_service"),
            Self::Database => write!(f, "database"),
            Self::Messaging => write!(f, "messaging"),
            Self::ManagementInterface => write!(f, "management_interface"),
            Self::ApiEndpoint => write!(f, "api_endpoint"),
            Self::Storage => write!(f, "storage"),
            Self::IdentityProvider => write!(f, "identity_provider"),
        }
    }
}

/// Discriminating strength of an endpoint fingerprint (ADR 0029).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintDiscriminatingPower {
    /// Cryptographic or rare service identity (e.g. SSH host key, unique SPKI).
    Discriminating,
    /// Characteristic product/favicon/interface signature.
    Moderate,
    /// Common server banner or default login page (never implies linkage alone).
    Generic,
}

/// Kind of network or application fingerprint attached to an endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintKind {
    TlsCertSha256,
    TlsSpkiSha256,
    SshHostKey,
    HttpFaviconHash,
    ManagementInterfaceSignature,
    GenericBanner,
}

/// Structured fingerprint observation with explicit discriminating power.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointFingerprint {
    pub kind: FingerprintKind,
    pub value: String,
    pub power: FingerprintDiscriminatingPower,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Epistemic status of a service or reference observation (ADR 0028 / Section 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Observed,
    Indexed,
    Declared,
    Referenced,
    Inferred,
    Verified,
    OperatorConfirmed,
    Unverified,
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observed => write!(f, "observed"),
            Self::Indexed => write!(f, "indexed"),
            Self::Declared => write!(f, "declared"),
            Self::Referenced => write!(f, "referenced"),
            Self::Inferred => write!(f, "inferred"),
            Self::Verified => write!(f, "verified"),
            Self::OperatorConfirmed => write!(f, "operator_confirmed"),
            Self::Unverified => write!(f, "unverified"),
        }
    }
}

/// Persisted observation of an externally observable network service (ADR 0026).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceObservation {
    pub item_type: ItemType,
    pub id: String,
    pub endpoint: SocketEndpoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_ip: Option<String>,
    #[serde(default)]
    pub associated_hosts: Vec<String>,
    pub category: ServiceCategory,
    pub protocol_id: ProtocolIdentification,
    pub state: ServiceObservationState,
    pub verification: VerificationStatus,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_recorded_at: Option<DateTime<Utc>>,
    pub ingested_at: DateTime<Utc>,
    #[serde(default)]
    pub fingerprints: Vec<EndpointFingerprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_required: Option<bool>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<String>,
    #[serde(default)]
    pub in_scope: bool,
}

/// Persisted observation of an application/infrastructure reference (ADR 0028).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationReferenceObservation {
    pub item_type: ItemType,
    pub id: String,
    pub origin_asset: String,
    pub referenced_target: String,
    pub target_kind: EntityKind,
    pub relationship_kind: RelationshipKind,
    pub reference_type: String,
    pub discovered_in: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_recorded_at: Option<DateTime<Utc>>,
    pub ingested_at: DateTime<Utc>,
    pub verification: VerificationStatus,
    #[serde(default)]
    pub in_scope: bool,
    #[serde(default)]
    pub deprecated_hint: bool,
    #[serde(default)]
    pub dev_or_placeholder_hint: bool,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<String>,
}
