//! Projects tab state management.

use ratatui::widgets::TableState;
use std::collections::HashSet;

/// State for the Projects tab
#[derive(Debug)]
pub struct ProjectsState {
    /// Selected row in the projects table (includes expanded assets)
    pub table_state: TableState,

    /// Set of project IDs that are expanded to show their assets
    pub expanded_projects: HashSet<String>,
}

impl ProjectsState {
    /// Creates a new ProjectsState
    pub fn new() -> Self {
        Self {
            table_state: TableState::default(),
            expanded_projects: HashSet::new(),
        }
    }

    /// Select the next row in the table
    /// `total_rows` - total number of visible rows (including expanded assets)
    pub fn next(&mut self, total_rows: usize) {
        if total_rows == 0 {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= total_rows - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    /// Select the previous row in the table
    /// `total_rows` - total number of visible rows (including expanded assets)
    pub fn previous(&mut self, total_rows: usize) {
        if total_rows == 0 {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    total_rows - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    /// Toggle expansion of a project at the given row index
    /// Returns the project ID if a project row was toggled, None if an asset row was selected
    pub fn toggle_expand_at_index(
        &mut self,
        idx: usize,
        project_ids: &[String],
        get_asset_count: impl Fn(&str) -> usize,
    ) -> Option<String> {
        let mut current_idx = 0;
        let mut target_project_id = None;

        for project_id in project_ids {
            if current_idx == idx {
                target_project_id = Some(project_id.clone());
                break;
            }
            current_idx += 1;
            if self.expanded_projects.contains(project_id) {
                current_idx += get_asset_count(project_id);
            }
        }

        if let Some(id) = target_project_id {
            if self.expanded_projects.contains(&id) {
                self.expanded_projects.remove(&id);
            } else {
                self.expanded_projects.insert(id.clone());
            }
            Some(id)
        } else {
            None
        }
    }

    /// Check if a project is expanded
    pub fn is_expanded(&self, project_id: &str) -> bool {
        self.expanded_projects.contains(project_id)
    }

    /// Expand a project
    pub fn expand(&mut self, project_id: String) {
        self.expanded_projects.insert(project_id);
    }

    /// Collapse a project
    pub fn collapse(&mut self, project_id: &str) {
        self.expanded_projects.remove(project_id);
    }
}

impl Default for ProjectsState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let state = ProjectsState::new();
        assert_eq!(state.table_state.selected(), None);
        assert!(state.expanded_projects.is_empty());
    }

    #[test]
    fn test_next_empty() {
        let mut state = ProjectsState::new();
        state.next(0);
        assert_eq!(state.table_state.selected(), None);
    }

    #[test]
    fn test_next_single() {
        let mut state = ProjectsState::new();
        state.next(1);
        assert_eq!(state.table_state.selected(), Some(0));
    }

    #[test]
    fn test_next_wraps() {
        let mut state = ProjectsState::new();
        state.table_state.select(Some(2));
        state.next(3);
        assert_eq!(state.table_state.selected(), Some(0));
    }

    #[test]
    fn test_previous_wraps() {
        let mut state = ProjectsState::new();
        state.table_state.select(Some(0));
        state.previous(3);
        assert_eq!(state.table_state.selected(), Some(2));
    }

    #[test]
    fn test_expand_collapse() {
        let mut state = ProjectsState::new();
        state.expand("proj1".to_string());
        assert!(state.is_expanded("proj1"));
        state.collapse("proj1");
        assert!(!state.is_expanded("proj1"));
    }
}
