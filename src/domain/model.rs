use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainId(Uuid);

impl Default for DomainId {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: DomainId,
    pub name: String,
    pub discovered_at: DateTime<Utc>,
}

impl Domain {
    pub fn new(name: &str) -> Self {
        Self {
            id: DomainId::new(),
            name: name.to_string(),
            discovered_at: Utc::now(),
        }
    }
}
