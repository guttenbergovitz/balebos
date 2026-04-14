use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::inventory::ItemType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl std::fmt::Display for FindingEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingEventType::Acknowledged => write!(f, "Acknowledged"),
            FindingEventType::Resolved => write!(f, "Resolved"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finding_creation() {
        let finding = Finding::new(
            FindingSeverity::High,
            FindingCategory::Security,
            "test_check",
            "example.com",
            "Test finding message",
        );

        assert_eq!(finding.severity, FindingSeverity::High);
        assert_eq!(finding.category, FindingCategory::Security);
        assert_eq!(finding.check_name, "test_check");
        assert_eq!(finding.target, "example.com");
        assert_eq!(finding.message, "Test finding message");
        assert_eq!(finding.item_type, ItemType::Finding);
        assert!(!finding.id.is_empty());
    }

    #[test]
    fn test_finding_severity_display() {
        assert_eq!(FindingSeverity::Info.to_string(), "Info");
        assert_eq!(FindingSeverity::Low.to_string(), "Low");
        assert_eq!(FindingSeverity::Medium.to_string(), "Medium");
        assert_eq!(FindingSeverity::High.to_string(), "High");
        assert_eq!(FindingSeverity::Critical.to_string(), "Critical");
    }

    #[test]
    fn test_finding_category_display() {
        assert_eq!(FindingCategory::Security.to_string(), "Security");
        assert_eq!(FindingCategory::Compliance.to_string(), "Compliance");
        assert_eq!(FindingCategory::Financial.to_string(), "Financial");
        assert_eq!(FindingCategory::Lifecycle.to_string(), "Lifecycle");
        assert_eq!(FindingCategory::Operational.to_string(), "Operational");
        assert_eq!(FindingCategory::Governance.to_string(), "Governance");
    }

    #[test]
    fn test_finding_event_creation() {
        let finding_id = "test-finding-id";

        let ack_event = FindingEvent::acknowledge(finding_id);
        assert_eq!(ack_event.event_type, FindingEventType::Acknowledged);
        assert_eq!(ack_event.finding_id, finding_id);
        assert_eq!(ack_event.item_type, ItemType::FindingEvent);

        let resolve_event = FindingEvent::resolve(finding_id);
        assert_eq!(resolve_event.event_type, FindingEventType::Resolved);
        assert_eq!(resolve_event.finding_id, finding_id);
        assert_eq!(resolve_event.item_type, ItemType::FindingEvent);
    }

    #[test]
    fn test_finding_event_type_display() {
        assert_eq!(FindingEventType::Acknowledged.to_string(), "Acknowledged");
        assert_eq!(FindingEventType::Resolved.to_string(), "Resolved");
    }
}
