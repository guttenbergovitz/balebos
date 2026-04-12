use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::inventory::ItemType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for FindingSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingSeverity::Info => write!(f, "Info"),
            FindingSeverity::Low => write!(f, "Low"),
            FindingSeverity::Medium => write!(f, "Medium"),
            FindingSeverity::High => write!(f, "High"),
            FindingSeverity::Critical => write!(f, "Critical"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    Security,
    Compliance,
    Financial,
    Lifecycle,
    Operational,
    Governance,
}

impl std::fmt::Display for FindingCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingCategory::Security => write!(f, "Security"),
            FindingCategory::Compliance => write!(f, "Compliance"),
            FindingCategory::Financial => write!(f, "Financial"),
            FindingCategory::Lifecycle => write!(f, "Lifecycle"),
            FindingCategory::Operational => write!(f, "Operational"),
            FindingCategory::Governance => write!(f, "Governance"),
        }
    }
}

/// A persisted finding produced by a check.
///
/// Findings are append-only records. Lifecycle transitions (acknowledge, resolve)
/// are stored as separate `FindingEvent` records referencing the finding ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub item_type: ItemType,
    pub id: String,
    pub severity: FindingSeverity,
    pub category: FindingCategory,
    pub check_name: String,
    pub target: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

impl Finding {
    pub fn new(
        severity: FindingSeverity,
        category: FindingCategory,
        check_name: &str,
        target: &str,
        message: &str,
    ) -> Self {
        Self {
            item_type: ItemType::Finding,
            id: Uuid::new_v4().to_string(),
            severity,
            category,
            check_name: check_name.to_string(),
            target: target.to_string(),
            message: message.to_string(),
            created_at: Utc::now(),
        }
    }
}

/// A lifecycle event for a finding (acknowledge or resolve).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingEvent {
    pub item_type: ItemType,
    pub id: String,
    pub finding_id: String,
    pub event_type: FindingEventType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingEventType {
    Acknowledged,
    Resolved,
}

impl FindingEvent {
    pub fn acknowledge(finding_id: &str) -> Self {
        Self {
            item_type: ItemType::FindingEvent,
            id: Uuid::new_v4().to_string(),
            finding_id: finding_id.to_string(),
            event_type: FindingEventType::Acknowledged,
            created_at: Utc::now(),
        }
    }

    pub fn resolve(finding_id: &str) -> Self {
        Self {
            item_type: ItemType::FindingEvent,
            id: Uuid::new_v4().to_string(),
            finding_id: finding_id.to_string(),
            event_type: FindingEventType::Resolved,
            created_at: Utc::now(),
        }
    }
}
