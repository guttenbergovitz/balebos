//! Inventory table methods for TUI.

use crate::tui::app::App;
use crate::tui::model::AssetState;

impl App {
    pub fn update_visible_items(&mut self) {
        self.inventory
            .update_visible_items(&self.model, &self.filter);
    }

    pub fn next(&mut self) {
        self.inventory.next();
    }

    pub fn previous(&mut self) {
        self.inventory.previous();
    }

    pub fn toggle_expand(&mut self) {
        self.inventory.toggle_expand();
        self.update_visible_items();
    }

    pub fn toggle_external_filter(&mut self) {
        self.inventory.toggle_external_filter();
        self.update_visible_items();
    }

    pub fn toggle_risk_sort(&mut self) {
        self.inventory.toggle_risk_sort();
        self.update_visible_items();
    }

    pub fn toggle_mark(&mut self) {
        if let Some(asset) = self.selected_asset() {
            self.inventory.toggle_mark(asset.name.clone());
        }
    }

    pub fn clear_marks(&mut self) {
        self.inventory.clear_marks();
    }

    pub fn selected_asset(&self) -> Option<&AssetState> {
        self.inventory.selected_asset(&self.model)
    }

    /// Select an asset by name in the inventory table (expanding its
    /// root domain if needed), returning `true` when found.
    pub fn select_asset_by_name(&mut self, name: &str) -> bool {
        use crate::tui::state::VisibleItem;

        for group in &self.model.groups {
            if group.subdomains.iter().any(|s| s.name == name) {
                self.inventory
                    .expanded_roots
                    .insert(group.root.name.clone());
                break;
            }
        }
        self.update_visible_items();

        let pos = self
            .inventory
            .visible_items
            .iter()
            .position(|item| match item {
                VisibleItem::Root(r) => r == name,
                VisibleItem::Subdomain(_, s) => s == name,
            });
        if let Some(idx) = pos {
            self.inventory.table_state.select(Some(idx));
            true
        } else {
            false
        }
    }

    pub fn toggle_junk_filter(&mut self) {
        self.inventory.hide_junk = !self.inventory.hide_junk;
        self.update_visible_items();
        let msg = if self.inventory.hide_junk {
            "Hiding junk assets".to_string()
        } else {
            "Showing junk assets".to_string()
        };
        self.set_status(msg);
    }

    /// Mark or unmark the selected asset as junk.
    pub fn toggle_junk_mark(&mut self) {
        let asset_name = match self.selected_asset() {
            Some(a) => a.name.clone(),
            None => return,
        };

        let is_junk = self
            .model
            .asset_tags
            .get(&asset_name)
            .map(|tags| tags.contains(crate::app::tags::JUNK_TAG))
            .unwrap_or(false);

        let result = if is_junk {
            crate::app::tags::remove_tag(&self.storage, &asset_name, crate::app::tags::JUNK_TAG)
        } else {
            crate::app::tags::add_tag(&self.storage, &asset_name, crate::app::tags::JUNK_TAG)
        };

        match result {
            Ok(()) => {
                let msg = if is_junk {
                    format!("✓ Unmarked '{}' as junk", asset_name)
                } else {
                    format!("🗑 Marked '{}' as junk", asset_name)
                };
                self.set_status(msg);
                // Refresh model to reflect tag changes
                if let Ok(new_model) = crate::tui::model::load_tui_model(&self.storage) {
                    self.model = new_model;
                    self.update_visible_items();
                }
            }
            Err(e) => {
                self.set_status(format!("Error: {}", e));
            }
        }
    }
}
