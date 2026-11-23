//! State management for the Drift tab.

use ratatui::widgets::TableState;

use crate::drift::DriftFinding;

/// Default comparison window in hours.
pub const DEFAULT_WINDOW_HOURS: u64 = 24;

/// State for the Drift tab: classified differences between a past
/// evidence window and the current view (ADR 0009), plus the coverage
/// flag that decides how absences must be read.
#[derive(Debug)]
pub struct DriftState {
    pub table_state: TableState,
    pub findings: Vec<DriftFinding>,
    /// True when the latest scan report proves full coverage.
    pub coverage_complete: bool,
    pub window_hours: u64,
    /// Whether a load has been attempted (empty findings after load is
    /// a real result, not a missing one).
    pub loaded: bool,
}

impl Default for DriftState {
    fn default() -> Self {
        Self {
            table_state: TableState::default(),
            findings: Vec::new(),
            coverage_complete: false,
            window_hours: DEFAULT_WINDOW_HOURS,
            loaded: false,
        }
    }
}

impl DriftState {
    pub fn next(&mut self) {
        let len = self.findings.len();
        if len == 0 {
            return;
        }
        let next = match self.table_state.selected() {
            Some(i) if i + 1 < len => i + 1,
            _ => 0,
        };
        self.table_state.select(Some(next));
    }

    pub fn previous(&mut self) {
        let len = self.findings.len();
        if len == 0 {
            return;
        }
        let prev = match self.table_state.selected() {
            Some(0) | None => len - 1,
            Some(i) => i - 1,
        };
        self.table_state.select(Some(prev));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::{DriftClass, DriftFinding};

    fn finding(asset: &str) -> DriftFinding {
        DriftFinding {
            asset: asset.to_string(),
            class: DriftClass::DnsChange,
            detail: "test".to_string(),
        }
    }

    #[test]
    fn navigation_wraps_and_handles_empty() {
        let mut state = DriftState::default();
        state.next();
        state.previous();

        state.findings = vec![finding("a"), finding("b")];
        state.table_state.select(Some(0));
        state.next();
        assert_eq!(state.table_state.selected(), Some(1));
        state.next();
        assert_eq!(state.table_state.selected(), Some(0));
        state.previous();
        assert_eq!(state.table_state.selected(), Some(1));
    }
}
