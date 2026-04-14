//! State management for Findings tab.

use ratatui::widgets::TableState;
use std::collections::HashSet;

use crate::app::{Finding, FindingEvent};

/// State for the Findings tab.
#[derive(Debug, Default)]
pub struct FindingsState {
    /// Table state for navigation
    pub table_state: TableState,

    /// Filter for findings (by status, severity, etc.)
    pub filter: Option<FindingFilter>,

    /// Expanded findings (showing details)
    pub expanded_findings: HashSet<String>,

    /// Visible findings after filtering
    pub visible_findings: Vec<Finding>,

    /// All findings loaded from storage
    pub all_findings: Vec<Finding>,

    /// All finding events loaded from storage
    pub all_events: Vec<FindingEvent>,
}

/// Filter options for findings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingFilter {
    Open,
    Acknowledged,
    Resolved,
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// Status of a finding based on events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingStatus {
    Open,
    Acknowledged,
    Resolved,
}

impl FindingsState {
    pub fn new() -> Self {
        Self {
            table_state: TableState::default(),
            filter: None,
            expanded_findings: HashSet::new(),
            visible_findings: Vec::new(),
            all_findings: Vec::new(),
            all_events: Vec::new(),
        }
    }

    /// Apply current filter to all findings
    pub fn apply_filter(&mut self) {
        self.visible_findings = self
            .all_findings
            .iter()
            .filter(|f| self.matches_filter(f))
            .cloned()
            .collect();
    }

    /// Get status of a finding based on events
    pub fn get_finding_status(&self, finding_id: &str) -> FindingStatus {
        let has_resolved = self.all_events.iter().any(|e| {
            e.finding_id == finding_id && e.event_type == crate::app::FindingEventType::Resolved
        });

        if has_resolved {
            return FindingStatus::Resolved;
        }

        let has_acknowledged = self.all_events.iter().any(|e| {
            e.finding_id == finding_id && e.event_type == crate::app::FindingEventType::Acknowledged
        });

        if has_acknowledged {
            return FindingStatus::Acknowledged;
        }

        FindingStatus::Open
    }

    /// Check if a finding matches the current filter
    fn matches_filter(&self, finding: &Finding) -> bool {
        match self.filter {
            Some(FindingFilter::Critical) => {
                finding.severity == crate::app::FindingSeverity::Critical
            }
            Some(FindingFilter::High) => finding.severity == crate::app::FindingSeverity::High,
            Some(FindingFilter::Medium) => finding.severity == crate::app::FindingSeverity::Medium,
            Some(FindingFilter::Low) => finding.severity == crate::app::FindingSeverity::Low,
            Some(FindingFilter::Info) => finding.severity == crate::app::FindingSeverity::Info,
            Some(FindingFilter::Open) => {
                self.get_finding_status(&finding.id) == FindingStatus::Open
            }
            Some(FindingFilter::Acknowledged) => {
                self.get_finding_status(&finding.id) == FindingStatus::Acknowledged
            }
            Some(FindingFilter::Resolved) => {
                self.get_finding_status(&finding.id) == FindingStatus::Resolved
            }
            None => true,
        }
    }

    /// Cycle through filter options
    pub fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            None => Some(FindingFilter::Critical),
            Some(FindingFilter::Critical) => Some(FindingFilter::High),
            Some(FindingFilter::High) => Some(FindingFilter::Medium),
            Some(FindingFilter::Medium) => Some(FindingFilter::Low),
            Some(FindingFilter::Low) => Some(FindingFilter::Info),
            Some(FindingFilter::Info) => Some(FindingFilter::Open),
            Some(FindingFilter::Open) => Some(FindingFilter::Acknowledged),
            Some(FindingFilter::Acknowledged) => Some(FindingFilter::Resolved),
            Some(FindingFilter::Resolved) => None,
        };
        self.apply_filter();
    }

    /// Clear current filter
    #[allow(dead_code)]
    pub fn clear_filter(&mut self) {
        self.filter = None;
        self.apply_filter();
    }

    /// Get filter description for display
    pub fn filter_description(&self) -> String {
        match self.filter {
            Some(FindingFilter::Open) => "Open".to_string(),
            Some(FindingFilter::Acknowledged) => "Acknowledged".to_string(),
            Some(FindingFilter::Resolved) => "Resolved".to_string(),
            Some(FindingFilter::Critical) => "Critical".to_string(),
            Some(FindingFilter::High) => "High".to_string(),
            Some(FindingFilter::Medium) => "Medium".to_string(),
            Some(FindingFilter::Low) => "Low".to_string(),
            Some(FindingFilter::Info) => "Info".to_string(),
            None => "All".to_string(),
        }
    }

    /// Toggle expansion for selected finding
    pub fn toggle_expand(&mut self) {
        if let Some(idx) = self.table_state.selected() {
            if let Some(finding) = self.visible_findings.get(idx) {
                let id = finding.id.clone();
                if self.expanded_findings.contains(&id) {
                    self.expanded_findings.remove(&id);
                } else {
                    self.expanded_findings.insert(id);
                }
            }
        }
    }

    /// Check if selected finding is expanded
    pub fn is_selected_expanded(&self) -> bool {
        if let Some(idx) = self.table_state.selected() {
            if let Some(finding) = self.visible_findings.get(idx) {
                return self.expanded_findings.contains(&finding.id);
            }
        }
        false
    }

    /// Move selection down
    pub fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.visible_findings.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    /// Move selection up
    pub fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.visible_findings.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Finding, FindingEvent, FindingEventType, FindingSeverity};
    use crate::app::finding::FindingCategory;

    fn create_test_finding(id: &str, severity: FindingSeverity) -> Finding {
        Finding::new(
            severity,
            FindingCategory::Security,
            "test_check",
            "example.com",
            &format!("Test finding {}", id),
        )
    }

    fn create_test_event(finding_id: &str, event_type: FindingEventType) -> FindingEvent {
        FindingEvent {
            item_type: crate::inventory::ItemType::FindingEvent,
            id: format!("event-{}", finding_id),
            finding_id: finding_id.to_string(),
            event_type,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_findings_state_new() {
        let state = FindingsState::new();
        assert!(state.filter.is_none());
        assert!(state.expanded_findings.is_empty());
        assert!(state.visible_findings.is_empty());
        assert!(state.all_findings.is_empty());
    }

    #[test]
    fn test_findings_state_filter() {
        let mut state = FindingsState::new();

        // Add test findings
        state.all_findings = vec![
            create_test_finding("1", FindingSeverity::Critical),
            create_test_finding("2", FindingSeverity::High),
            create_test_finding("3", FindingSeverity::Medium),
            create_test_finding("4", FindingSeverity::Low),
            create_test_finding("5", FindingSeverity::Info),
        ];

        // Test critical filter
        state.filter = Some(FindingFilter::Critical);
        state.apply_filter();
        assert_eq!(state.visible_findings.len(), 1);
        assert_eq!(
            state.visible_findings[0].severity,
            FindingSeverity::Critical
        );

        // Test high filter
        state.filter = Some(FindingFilter::High);
        state.apply_filter();
        assert_eq!(state.visible_findings.len(), 1);
        assert_eq!(state.visible_findings[0].severity, FindingSeverity::High);

        // Test no filter
        state.filter = None;
        state.apply_filter();
        assert_eq!(state.visible_findings.len(), 5);
    }

    #[test]
    fn test_get_finding_status() {
        let mut state = FindingsState::new();

        // Add test finding
        let finding = create_test_finding("1", FindingSeverity::Medium);
        let finding_id = finding.id.clone();
        state.all_findings = vec![finding];

        // Initially should be open
        assert_eq!(state.get_finding_status(&finding_id), FindingStatus::Open);

        // Add acknowledged event
        state.all_events = vec![create_test_event(
            &finding_id,
            FindingEventType::Acknowledged,
        )];
        assert_eq!(
            state.get_finding_status(&finding_id),
            FindingStatus::Acknowledged
        );

        // Add resolved event (should override acknowledged)
        state.all_events = vec![
            create_test_event(&finding_id, FindingEventType::Acknowledged),
            create_test_event(&finding_id, FindingEventType::Resolved),
        ];
        assert_eq!(
            state.get_finding_status(&finding_id),
            FindingStatus::Resolved
        );
    }

    #[test]
    fn test_status_filter() {
        let mut state = FindingsState::new();

        // Create findings with different statuses
        let finding1 = create_test_finding("1", FindingSeverity::Medium);
        let finding2 = create_test_finding("2", FindingSeverity::Medium);
        let finding3 = create_test_finding("3", FindingSeverity::Medium);

        state.all_findings = vec![finding1.clone(), finding2.clone(), finding3.clone()];

        // Add events to change statuses
        state.all_events = vec![
            create_test_event(&finding1.id, FindingEventType::Acknowledged),
            create_test_event(&finding2.id, FindingEventType::Resolved),
            // finding3 remains open
        ];

        // Test open filter
        state.filter = Some(FindingFilter::Open);
        state.apply_filter();
        assert_eq!(state.visible_findings.len(), 1);
        assert_eq!(state.visible_findings[0].id, finding3.id);

        // Test acknowledged filter
        state.filter = Some(FindingFilter::Acknowledged);
        state.apply_filter();
        assert_eq!(state.visible_findings.len(), 1);
        assert_eq!(state.visible_findings[0].id, finding1.id);

        // Test resolved filter
        state.filter = Some(FindingFilter::Resolved);
        state.apply_filter();
        assert_eq!(state.visible_findings.len(), 1);
        assert_eq!(state.visible_findings[0].id, finding2.id);
    }

    #[test]
    fn test_findings_state_cycle_filter() {
        let mut state = FindingsState::new();

        // Start with no filter
        assert!(state.filter.is_none());

        // Cycle through filters
        state.cycle_filter();
        assert_eq!(state.filter, Some(FindingFilter::Critical));

        state.cycle_filter();
        assert_eq!(state.filter, Some(FindingFilter::High));

        state.cycle_filter();
        assert_eq!(state.filter, Some(FindingFilter::Medium));

        state.cycle_filter();
        assert_eq!(state.filter, Some(FindingFilter::Low));

        state.cycle_filter();
        assert_eq!(state.filter, Some(FindingFilter::Info));

        state.cycle_filter();
        assert_eq!(state.filter, Some(FindingFilter::Open));

        state.cycle_filter();
        assert_eq!(state.filter, Some(FindingFilter::Acknowledged));

        state.cycle_filter();
        assert_eq!(state.filter, Some(FindingFilter::Resolved));

        state.cycle_filter();
        assert!(state.filter.is_none());
    }

    #[test]
    fn test_findings_state_navigation() {
        let mut state = FindingsState::new();

        // Add test findings
        state.all_findings = vec![
            create_test_finding("1", FindingSeverity::Medium),
            create_test_finding("2", FindingSeverity::Medium),
            create_test_finding("3", FindingSeverity::Medium),
        ];
        state.apply_filter();

        // Test next navigation
        state.next();
        assert_eq!(state.table_state.selected(), Some(0));

        state.next();
        assert_eq!(state.table_state.selected(), Some(1));

        state.next();
        assert_eq!(state.table_state.selected(), Some(2));

        state.next(); // Should wrap around
        assert_eq!(state.table_state.selected(), Some(0));

        // Test previous navigation
        state.previous();
        assert_eq!(state.table_state.selected(), Some(2));

        state.previous();
        assert_eq!(state.table_state.selected(), Some(1));

        state.previous();
        assert_eq!(state.table_state.selected(), Some(0));

        state.previous(); // Should wrap around
        assert_eq!(state.table_state.selected(), Some(2));
    }

    #[test]
    fn test_findings_state_expand() {
        let mut state = FindingsState::new();

        // Add test finding
        let finding = create_test_finding("1", FindingSeverity::Medium);
        let finding_id = finding.id.clone();
        state.all_findings = vec![finding];
        state.apply_filter();
        state.table_state.select(Some(0));

        // Test toggle expand
        assert!(!state.expanded_findings.contains(&finding_id));

        state.toggle_expand();
        assert!(state.expanded_findings.contains(&finding_id));
        assert!(state.is_selected_expanded());

        state.toggle_expand();
        assert!(!state.expanded_findings.contains(&finding_id));
        assert!(!state.is_selected_expanded());
    }

    #[test]
    fn test_filter_description() {
        let mut state = FindingsState::new();

        assert_eq!(state.filter_description(), "All");

        state.filter = Some(FindingFilter::Critical);
        assert_eq!(state.filter_description(), "Critical");

        state.filter = Some(FindingFilter::High);
        assert_eq!(state.filter_description(), "High");

        state.filter = Some(FindingFilter::Medium);
        assert_eq!(state.filter_description(), "Medium");

        state.filter = Some(FindingFilter::Low);
        assert_eq!(state.filter_description(), "Low");

        state.filter = Some(FindingFilter::Info);
        assert_eq!(state.filter_description(), "Info");

        state.filter = Some(FindingFilter::Open);
        assert_eq!(state.filter_description(), "Open");

        state.filter = Some(FindingFilter::Acknowledged);
        assert_eq!(state.filter_description(), "Acknowledged");

        state.filter = Some(FindingFilter::Resolved);
        assert_eq!(state.filter_description(), "Resolved");
    }
}
