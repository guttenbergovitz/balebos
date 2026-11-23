//! Evidence-backed infrastructure relationship domain types (ADR 0011).
//!
//! Pure data structures with no storage, probe or UI dependencies.
//! Technical relationships are projections over immutable observations;
//! shared infrastructure never implies common ownership automatically.

use std::collections::BTreeSet;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Kind of infrastructure entity participating in a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Domain,
    Fqdn,
    IpAddress,
    Asn,
    TlsCertificate,
    Endpoint,
    Provider,
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain => write!(f, "domain"),
            Self::Fqdn => write!(f, "fqdn"),
            Self::IpAddress => write!(f, "ip"),
            Self::Asn => write!(f, "asn"),
            Self::TlsCertificate => write!(f, "tls_cert"),
            Self::Endpoint => write!(f, "endpoint"),
            Self::Provider => write!(f, "provider"),
        }
    }
}

/// Stable reference to an infrastructure entity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityRef {
    pub kind: EntityKind,
    /// Canonical normalised identifier (e.g. lowercase hostname, IP, `AS13335`, SHA-256).
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl EntityRef {
    pub fn new(kind: EntityKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl fmt::Display for EntityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.label {
            Some(l) if l != &self.id => write!(f, "{}:{} ({})", self.kind, self.id, l),
            _ => write!(f, "{}:{}", self.kind, self.id),
        }
    }
}

/// Typed technical or attribution relationship between two entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    ResolvesTo,
    AliasesTo,
    RedirectsTo,
    PresentsCertificate,
    BelongsToAsn,
    UsesNameserver,
    RoutesMailTo,
    HostedBy,
}

impl fmt::Display for RelationshipKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResolvesTo => write!(f, "resolves_to"),
            Self::AliasesTo => write!(f, "aliases_to"),
            Self::RedirectsTo => write!(f, "redirects_to"),
            Self::PresentsCertificate => write!(f, "presents_certificate"),
            Self::BelongsToAsn => write!(f, "belongs_to_asn"),
            Self::UsesNameserver => write!(f, "uses_nameserver"),
            Self::RoutesMailTo => write!(f, "routes_mail_to"),
            Self::HostedBy => write!(f, "hosted_by"),
        }
    }
}

/// Explicit signal strength category without uncalibrated numerical scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalCategory {
    /// Technically established from direct probe output (e.g. DNS A, TLS cert).
    DirectlyObserved,
    /// Plausible connection supported by multiple independent signals.
    StrongCandidate,
    /// Shared infrastructure clue (shared IP, CDN, ASN, NS) — never proves ownership.
    WeakSharedClue,
    /// Evidence is partial, stale or ambiguous.
    InsufficientEvidence,
    /// Explicitly confirmed by the operator.
    OperatorConfirmed,
}

impl fmt::Display for SignalCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectlyObserved => write!(f, "directly observed"),
            Self::StrongCandidate => write!(f, "strong candidate"),
            Self::WeakSharedClue => write!(f, "weak/shared clue"),
            Self::InsufficientEvidence => write!(f, "insufficient evidence"),
            Self::OperatorConfirmed => write!(f, "operator confirmed"),
        }
    }
}

/// Immutable provenance backing a projected relationship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipEvidence {
    pub observation_ids: Vec<String>,
    pub sources: BTreeSet<String>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<String>,
}

/// Evidence-backed relationship projected from persisted observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedRelationship {
    pub source: EntityRef,
    pub kind: RelationshipKind,
    pub target: EntityRef,
    pub signal: SignalCategory,
    pub evidence: RelationshipEvidence,
    /// True when the latest observation of `(source, kind)` still includes `target`.
    pub is_current: bool,
}
