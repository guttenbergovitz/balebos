//! Asset lifecycle management
//!
//! Assets have a lifecycle state that transitions over time:
//! - Active: Normal operations
//! - UnderReview: Being evaluated (e.g., for decommissioning)
//! - Deprecated: Scheduled for removal
//! - Decommissioned: Removed but still tracked
//! - Abandoned: No owner, stale, potentially forgotten
//!
//! Lifecycle transitions are stored as append-only events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::inventory::ItemType;

/// Lifecycle state of an asset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetLifecycle {
    /// Normal operations
    Active,
    /// Being evaluated (e.g., for decommissioning)
    UnderReview,
    /// Scheduled for removal
    Deprecated,
    /// Removed but still tracked
    Decommissioned,
    /// No owner, stale, potentially forgotten
    Abandoned,
}

impl std::fmt::Display for AssetLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetLifecycle::Active => write!(f, "Active"),
            AssetLifecycle::UnderReview => write!(f, "Under Review"),
            AssetLifecycle::Deprecated => write!(f, "Deprecated"),
            AssetLifecycle::Decommissioned => write!(f, "Decommissioned"),
            AssetLifecycle::Abandoned => write!(f, "Abandoned"),
        }
    }
}

impl AssetLifecycle {
    /// Parse lifecycle state from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(AssetLifecycle::Active),
            "under_review" | "under-review" | "underreview" | "review" => {
                Some(AssetLifecycle::UnderReview)
            }
            "deprecated" => Some(AssetLifecycle::Deprecated),
            "decommissioned" => Some(AssetLifecycle::Decommissioned),
            "abandoned" => Some(AssetLifecycle::Abandoned),
            _ => None,
        }
    }

    /// Get color for this lifecycle state (for TUI display)
    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            AssetLifecycle::Active => Color::Green,
            AssetLifecycle::UnderReview => Color::Yellow,
            AssetLifecycle::Deprecated => Color::Magenta,
            AssetLifecycle::Decommissioned => Color::Gray,
            AssetLifecycle::Abandoned => Color::Red,
        }
    }

    /// Get icon for this lifecycle state
    pub fn icon(&self) -> &'static str {
        match self {
            AssetLifecycle::Active => "●",
            AssetLifecycle::UnderReview => "○",
            AssetLifecycle::Deprecated => "◐",
            AssetLifecycle::Decommissioned => "◌",
            AssetLifecycle::Abandoned => "✗",
        }
    }
}

/// Lifecycle transition event (append-only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub item_type: ItemType,
    pub id: String,
    pub asset_name: String,
    pub from_state: Option<AssetLifecycle>, // None = initial assignment
    pub to_state: AssetLifecycle,
    pub reason: Option<String>,
    pub changed_by: String, // "cli", "auto", "finding", "tui"
    pub created_at: DateTime<Utc>,
}

impl LifecycleEvent {
    /// Create a new lifecycle transition event
    pub fn new(
        asset_name: String,
        from_state: Option<AssetLifecycle>,
        to_state: AssetLifecycle,
        reason: Option<String>,
        changed_by: String,
    ) -> Self {
        Self {
            item_type: ItemType::LifecycleEvent,
            id: Uuid::new_v4().to_string(),
            asset_name,
            from_state,
            to_state,
            reason,
            changed_by,
            created_at: Utc::now(),
        }
    }

    /// Create an initial lifecycle assignment (no from_state)
    #[allow(dead_code)]
    pub fn initial(asset_name: String, state: AssetLifecycle, changed_by: String) -> Self {
        Self::new(asset_name, None, state, None, changed_by)
    }
}

/// Get current lifecycle state for an asset from its event history
pub fn get_current_lifecycle(
    events: &[LifecycleEvent],
    asset_name: &str,
) -> Option<AssetLifecycle> {
    events
        .iter()
        .filter(|e| e.asset_name == asset_name)
        .max_by_key(|e| e.created_at)
        .map(|e| e.to_state)
}

/// Get full lifecycle history for an asset
pub fn get_lifecycle_history(events: &[LifecycleEvent], asset_name: &str) -> Vec<LifecycleEvent> {
    let mut history: Vec<LifecycleEvent> = events
        .iter()
        .filter(|e| e.asset_name == asset_name)
        .cloned()
        .collect();
    history.sort_by_key(|e| e.created_at);
    history
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_lifecycle_display() {
        assert_eq!(AssetLifecycle::Active.to_string(), "Active");
        assert_eq!(AssetLifecycle::UnderReview.to_string(), "Under Review");
        assert_eq!(AssetLifecycle::Deprecated.to_string(), "Deprecated");
        assert_eq!(AssetLifecycle::Decommissioned.to_string(), "Decommissioned");
        assert_eq!(AssetLifecycle::Abandoned.to_string(), "Abandoned");
    }

    #[test]
    fn test_asset_lifecycle_from_str() {
        assert_eq!(
            AssetLifecycle::parse("active"),
            Some(AssetLifecycle::Active)
        );
        assert_eq!(
            AssetLifecycle::parse("under_review"),
            Some(AssetLifecycle::UnderReview)
        );
        assert_eq!(
            AssetLifecycle::parse("under-review"),
            Some(AssetLifecycle::UnderReview)
        );
        assert_eq!(
            AssetLifecycle::parse("deprecated"),
            Some(AssetLifecycle::Deprecated)
        );
        assert_eq!(
            AssetLifecycle::parse("decommissioned"),
            Some(AssetLifecycle::Decommissioned)
        );
        assert_eq!(
            AssetLifecycle::parse("abandoned"),
            Some(AssetLifecycle::Abandoned)
        );
        assert_eq!(AssetLifecycle::parse("invalid"), None);
    }

    #[test]
    fn test_lifecycle_event_creation() {
        let event = LifecycleEvent::new(
            "example.com".to_string(),
            Some(AssetLifecycle::Active),
            AssetLifecycle::Deprecated,
            Some("Scheduled for removal".to_string()),
            "cli".to_string(),
        );

        assert_eq!(event.asset_name, "example.com");
        assert_eq!(event.from_state, Some(AssetLifecycle::Active));
        assert_eq!(event.to_state, AssetLifecycle::Deprecated);
        assert_eq!(event.reason, Some("Scheduled for removal".to_string()));
        assert_eq!(event.changed_by, "cli");
        assert_eq!(event.item_type, ItemType::LifecycleEvent);
    }

    #[test]
    fn test_lifecycle_event_initial() {
        let event = LifecycleEvent::initial(
            "example.com".to_string(),
            AssetLifecycle::Active,
            "cli".to_string(),
        );

        assert_eq!(event.asset_name, "example.com");
        assert_eq!(event.from_state, None);
        assert_eq!(event.to_state, AssetLifecycle::Active);
        assert_eq!(event.changed_by, "cli");
    }

    #[test]
    fn test_get_current_lifecycle() {
        let mut events = vec![];

        // No events yet
        assert_eq!(get_current_lifecycle(&events, "example.com"), None);

        // Initial state
        events.push(LifecycleEvent::initial(
            "example.com".to_string(),
            AssetLifecycle::Active,
            "cli".to_string(),
        ));
        assert_eq!(
            get_current_lifecycle(&events, "example.com"),
            Some(AssetLifecycle::Active)
        );

        // Transition to deprecated
        events.push(LifecycleEvent::new(
            "example.com".to_string(),
            Some(AssetLifecycle::Active),
            AssetLifecycle::Deprecated,
            None,
            "cli".to_string(),
        ));
        assert_eq!(
            get_current_lifecycle(&events, "example.com"),
            Some(AssetLifecycle::Deprecated)
        );
    }

    #[test]
    fn test_get_lifecycle_history() {
        let events = vec![
            LifecycleEvent::initial(
                "example.com".to_string(),
                AssetLifecycle::Active,
                "cli".to_string(),
            ),
            LifecycleEvent::new(
                "example.com".to_string(),
                Some(AssetLifecycle::Active),
                AssetLifecycle::Deprecated,
                None,
                "cli".to_string(),
            ),
            // Different asset
            LifecycleEvent::initial(
                "other.com".to_string(),
                AssetLifecycle::Active,
                "cli".to_string(),
            ),
        ];

        let history = get_lifecycle_history(&events, "example.com");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].to_state, AssetLifecycle::Active);
        assert_eq!(history[1].to_state, AssetLifecycle::Deprecated);

        let other_history = get_lifecycle_history(&events, "other.com");
        assert_eq!(other_history.len(), 1);

        let empty_history = get_lifecycle_history(&events, "nonexistent.com");
        assert!(empty_history.is_empty());
    }
}
