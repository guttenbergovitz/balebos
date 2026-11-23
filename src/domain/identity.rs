//! Stable entity identities.
//!
//! Identity atoms for assets, scans and observations (ADR 0004). These are
//! the stable references the future relationship graph (ADR 0011) will link
//! against, so they are validated newtypes — a malformed identity string is
//! a parse error, never a silently defaulted value.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Parse failure for identity newtypes: the input is not a valid UUID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} id: {input}")]
pub struct InvalidId {
    kind: &'static str,
    input: String,
}

macro_rules! id_type {
    ($name:ident, $kind:literal) => {
        /// Validated, copyable identity newtype over a v4 UUID.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a fresh identity.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Borrow the canonical string form (e.g. for persistence).
            pub fn as_str(&self) -> String {
                self.0.to_string()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = InvalidId;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self).map_err(|_| InvalidId {
                    kind: $kind,
                    input: s.to_string(),
                })
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Uuid {
                id.0
            }
        }
    };
}

id_type!(AssetId, "asset");
id_type!(ScanId, "scan");
id_type!(ObservationId, "observation");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_accepts_canonical_uuid() {
        let id = AssetId::from_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn from_str_rejects_garbage_and_empty() {
        for bad in ["", "not-a-uuid", "550e8400e29b41d4a71644665544000z"] {
            assert!(AssetId::from_str(bad).is_err(), "accepted {bad:?}");
            assert!(ScanId::from_str(bad).is_err(), "accepted {bad:?}");
            assert!(ObservationId::from_str(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn display_round_trips_through_from_str() {
        let id = ScanId::new();
        let parsed = ScanId::from_str(&id.to_string()).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn serde_round_trips_as_plain_string() {
        let id = ObservationId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{}\"", id));

        let back: ObservationId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn new_yields_distinct_copyable_ids() {
        let a = AssetId::new();
        let b = AssetId::new();
        assert_ne!(a, b);

        let copied = a;
        assert_eq!(copied, a);
    }

    #[test]
    fn error_names_the_kind() {
        let err = AssetId::from_str("junk").unwrap_err();
        assert!(err.to_string().contains("asset"));
        assert!(err.to_string().contains("junk"));
    }
}
