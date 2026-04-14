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
}
