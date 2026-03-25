//! Inventory tab state management.
//!
//! Manages the state of the inventory table including navigation,
//! expand/collapse operations, filtering, and asset marking.

use crate::tui::model::{AssetState, TuiModel};
use ratatui::widgets::TableState;
use std::collections::HashSet;

/// Visible items in Inventory table (tree structure)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibleItem {
    Root(String),
    Subdomain(String, String),
}

/// Manages the state of the inventory tab.
///
/// Tracks table navigation, expansion state of items, filtering,
/// and marked assets for bulk operations.
#[derive(Debug, Clone)]
pub struct InventoryState {
    pub table_state: TableState,
    pub expanded_roots: HashSet<String>,
    pub visible_items: Vec<VisibleItem>,
    pub hide_external_redirects: bool,
    pub marked_assets: HashSet<String>,
    pub filter_tag: Option<String>,
}

impl InventoryState {
    /// Create a new inventory state with empty tables and no filtering.
    pub fn new() -> Self {
        Self {
            table_state: TableState::default(),
            expanded_roots: HashSet::new(),
            visible_items: Vec::new(),
            hide_external_redirects: false,
            marked_assets: HashSet::new(),
            filter_tag: None,
        }
    }

    /// Move selection to the next visible item.
    pub fn next(&mut self) {
        if self.visible_items.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.visible_items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    /// Move selection to the previous visible item.
    pub fn previous(&mut self) {
        if self.visible_items.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.visible_items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    /// Toggle expansion of a root domain (show/hide subdomains).
    pub fn toggle_expand(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if let Some(VisibleItem::Root(name)) = self.visible_items.get(selected) {
                if self.expanded_roots.contains(name) {
                    self.expanded_roots.remove(name);
                } else {
                    self.expanded_roots.insert(name.clone());
                }
            }
        }
    }

    /// Toggle filtering for external redirects.
    pub fn toggle_external_filter(&mut self) {
        self.hide_external_redirects = !self.hide_external_redirects;
    }

    /// Get the currently selected row index.
    /// Get the currently selected asset from the model.
    ///
    /// Returns None if no item is selected or the item cannot be found.
    pub fn selected_asset<'a>(&self, model: &'a TuiModel) -> Option<&'a AssetState> {
        let idx = self.table_state.selected()?;
        let item = self.visible_items.get(idx)?;

        let target_name = match item {
            VisibleItem::Root(name) => name,
            VisibleItem::Subdomain(_, name) => name,
        };

        for group in &model.groups {
            if &group.root.name == target_name {
                return Some(&group.root);
            }
            for sub in &group.subdomains {
                if &sub.name == target_name {
                    return Some(sub);
                }
            }
        }
        None
    }

    /// Toggle the mark state of an asset.
    pub fn toggle_mark(&mut self, asset_name: String) {
        if self.marked_assets.contains(&asset_name) {
            self.marked_assets.remove(&asset_name);
        } else {
            self.marked_assets.insert(asset_name);
        }
    }

    /// Clear all marked assets.
    pub fn clear_marks(&mut self) {
        self.marked_assets.clear();
    }

    /// Update visible items list based on filter and expansion state.
    ///
    /// This method recalculates which items should be visible based on:
    /// - Current filter string
    /// - Whether to hide external redirects
    /// - Which roots are expanded
    ///
    /// It also ensures the selection stays in bounds.
    pub fn update_visible_items(&mut self, model: &TuiModel, filter: &str) {
        let mut visible = Vec::new();
        let filter_lower = filter.to_lowercase();

        for group in &model.groups {
            let root_name = &group.root.name;

            // Check tag filter first
            if let Some(ref tag) = self.filter_tag {
                if !model
                    .asset_tags
                    .get(root_name)
                    .map(|tags| tags.contains(tag))
                    .unwrap_or(false)
                {
                    continue;
                }
            }

            // Apply filtering
            let root_matches = root_name.to_lowercase().contains(&filter_lower);

            // Check if any subdomains match the filter
            let any_subdomain_matches = group
                .subdomains
                .iter()
                .any(|s| s.name.to_lowercase().contains(&filter_lower));

            if !root_matches && !any_subdomain_matches && !filter_lower.is_empty() {
                continue;
            }

            // External redirects filter
            if self.hide_external_redirects && group.root.external_redirect().is_some() {
                continue;
            }

            visible.push(VisibleItem::Root(root_name.clone()));

            if self.expanded_roots.contains(root_name) {
                for sub in &group.subdomains {
                    if filter_lower.is_empty() || sub.name.to_lowercase().contains(&filter_lower) {
                        visible.push(VisibleItem::Subdomain(root_name.clone(), sub.name.clone()));
                    }
                }
            }
        }

        self.visible_items = visible;

        // Keep selection in bounds
        if let Some(selected) = self.table_state.selected() {
            if selected >= self.visible_items.len() {
                if !self.visible_items.is_empty() {
                    self.table_state.select(Some(self.visible_items.len() - 1));
                } else {
                    self.table_state.select(None);
                }
            }
        }
    }
}

impl Default for InventoryState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_new() {
        let state = InventoryState::new();
        assert!(state.visible_items.is_empty());
        assert!(state.expanded_roots.is_empty());
        assert!(state.marked_assets.is_empty());
        assert!(!state.hide_external_redirects);
    }

    #[test]
    fn test_next_navigation() {
        let mut state = InventoryState::new();
        state.visible_items = vec![
            VisibleItem::Root("example.com".to_string()),
            VisibleItem::Root("test.com".to_string()),
        ];

        state.next();
        assert_eq!(state.table_state.selected(), Some(0));

        state.next();
        assert_eq!(state.table_state.selected(), Some(1));

        // Wrap around
        state.next();
        assert_eq!(state.table_state.selected(), Some(0));
    }

    #[test]
    fn test_previous_navigation() {
        let mut state = InventoryState::new();
        state.visible_items = vec![
            VisibleItem::Root("example.com".to_string()),
            VisibleItem::Root("test.com".to_string()),
        ];

        state.previous();
        assert_eq!(state.table_state.selected(), Some(0));

        state.table_state.select(Some(1));
        state.previous();
        assert_eq!(state.table_state.selected(), Some(0));

        state.previous();
        assert_eq!(state.table_state.selected(), Some(1));
    }

    #[test]
    fn test_toggle_expand() {
        let mut state = InventoryState::new();
        state.visible_items = vec![VisibleItem::Root("example.com".to_string())];

        assert!(!state.expanded_roots.contains("example.com"));
        state.toggle_expand();
        assert!(state.expanded_roots.contains("example.com"));
        state.toggle_expand();
        assert!(!state.expanded_roots.contains("example.com"));
    }

    #[test]
    fn test_toggle_external_filter() {
        let mut state = InventoryState::new();
        assert!(!state.hide_external_redirects);
        state.toggle_external_filter();
        assert!(state.hide_external_redirects);
        state.toggle_external_filter();
        assert!(!state.hide_external_redirects);
    }

    #[test]
    fn test_toggle_mark() {
        let mut state = InventoryState::new();
        state.toggle_mark("example.com".to_string());
        assert!(state.marked_assets.contains("example.com"));

        state.toggle_mark("example.com".to_string());
        assert!(!state.marked_assets.contains("example.com"));
    }

    #[test]
    fn test_clear_marks() {
        let mut state = InventoryState::new();
        state.toggle_mark("example.com".to_string());
        state.toggle_mark("test.com".to_string());
        assert_eq!(state.marked_assets.len(), 2);

        state.clear_marks();
        assert!(state.marked_assets.is_empty());
    }
}
