//! Organization tab state management.
//!
//! Manages navigation between different organization panes (Persons, Companies, Brands)
//! and table state for the organization tab.

use ratatui::widgets::TableState;

/// Organization pane selection (Persons, Companies, or Brands).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrgPane {
    #[default]
    Persons,
    Companies,
    Brands,
}

impl OrgPane {
    /// Get all available panes.
    pub fn all() -> &'static [OrgPane] {
        &[OrgPane::Persons, OrgPane::Companies, OrgPane::Brands]
    }

    /// Get the display title for this pane.
    pub fn title(&self) -> &'static str {
        match self {
            OrgPane::Persons => "Persons",
            OrgPane::Companies => "Companies",
            OrgPane::Brands => "Brands",
        }
    }

    /// Move to the next pane (wraps around).
    pub fn next(&self) -> Self {
        match self {
            OrgPane::Persons => OrgPane::Companies,
            OrgPane::Companies => OrgPane::Brands,
            OrgPane::Brands => OrgPane::Persons,
        }
    }

    /// Move to the previous pane (wraps around).
    pub fn prev(&self) -> Self {
        match self {
            OrgPane::Persons => OrgPane::Brands,
            OrgPane::Companies => OrgPane::Persons,
            OrgPane::Brands => OrgPane::Companies,
        }
    }
}

/// Manages the state of the organization tab.
///
/// Tracks which pane is currently active and the table selection.
#[derive(Debug, Clone)]
pub struct OrgState {
    pub pane: OrgPane,
    pub table_state: TableState,
}

impl OrgState {
    /// Create a new organization state.
    pub fn new() -> Self {
        Self {
            pane: OrgPane::default(),
            table_state: TableState::default(),
        }
    }

    /// Move to the next pane.
    pub fn next_pane(&mut self) {
        self.pane = self.pane.next();
    }

    /// Move to the previous pane.
    pub fn prev_pane(&mut self) {
        self.pane = self.pane.prev();
    }

    /// Get the current pane.
    pub fn current_pane(&self) -> OrgPane {
        self.pane
    }

    /// Update the table selection based on current item count.
    ///
    /// Ensures the selection stays within bounds when the number of items changes.
    pub fn update_selection(&mut self, item_count: usize) {
        if item_count > 0 {
            if let Some(selected) = self.table_state.selected() {
                if selected >= item_count {
                    self.table_state.select(Some(item_count - 1));
                }
            } else {
                self.table_state.select(Some(0));
            }
        } else {
            self.table_state.select(None);
        }
    }

    /// Get the currently selected row index.
    pub fn selected(&self) -> Option<usize> {
        self.table_state.selected()
    }

    /// Set the selected row index.
    pub fn select(&mut self, index: Option<usize>) {
        self.table_state.select(index);
    }
}

impl Default for OrgState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_pane_navigation() {
        let pane = OrgPane::Persons;
        assert_eq!(pane.next(), OrgPane::Companies);
        assert_eq!(pane.next().next(), OrgPane::Brands);
        assert_eq!(pane.next().next().next(), OrgPane::Persons);
    }

    #[test]
    fn test_org_pane_reverse_navigation() {
        let pane = OrgPane::Persons;
        assert_eq!(pane.prev(), OrgPane::Brands);
        assert_eq!(pane.prev().prev(), OrgPane::Companies);
        assert_eq!(pane.prev().prev().prev(), OrgPane::Persons);
    }

    #[test]
    fn test_org_pane_titles() {
        assert_eq!(OrgPane::Persons.title(), "Persons");
        assert_eq!(OrgPane::Companies.title(), "Companies");
        assert_eq!(OrgPane::Brands.title(), "Brands");
    }

    #[test]
    fn test_org_state_new() {
        let state = OrgState::new();
        assert_eq!(state.pane, OrgPane::Persons);
        assert_eq!(state.selected(), None);
    }

    #[test]
    fn test_org_state_pane_navigation() {
        let mut state = OrgState::new();
        state.next_pane();
        assert_eq!(state.pane, OrgPane::Companies);
        state.prev_pane();
        assert_eq!(state.pane, OrgPane::Persons);
    }

    #[test]
    fn test_org_state_selection_update() {
        let mut state = OrgState::new();
        state.update_selection(5);
        assert_eq!(state.selected(), Some(0));

        state.select(Some(3));
        state.update_selection(2);
        assert_eq!(state.selected(), Some(1));

        state.update_selection(0);
        assert_eq!(state.selected(), None);
    }
}
