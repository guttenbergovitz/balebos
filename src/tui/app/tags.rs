//! Tag methods for TUI.

use crate::tui::app::{App, InputMode};

impl App {
    pub fn cycle_tag_filter(&mut self) {
        // Open tag filter selector instead of cycling
        self.start_tag_filter_selection();
    }

    pub fn start_tag_filter_selection(&mut self) {
        // Get all unique tags for selection
        self.available_tags = self
            .model
            .asset_tags
            .values()
            .flat_map(|tags| tags.iter())
            .map(|s| s.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        self.tag_select_index = 0;

        if self.available_tags.is_empty() {
            // No tags available
            self.inventory.filter_tag = None;
            self.update_visible_items();
        } else {
            self.input_mode = InputMode::TagFilterSelecting;
        }
    }

    pub fn tag_filter_select_confirm(&mut self) {
        if let Some(tag) = self.available_tags.get(self.tag_select_index) {
            self.inventory.filter_tag = Some(tag.clone());
            self.update_visible_items();
        }
        self.input_mode = InputMode::Normal;
    }

    pub fn tag_filter_clear(&mut self) {
        self.inventory.filter_tag = None;
        self.update_visible_items();
        self.input_mode = InputMode::Normal;
    }

    pub fn start_tag_selection(&mut self) {
        // Get all unique tags for selection
        self.available_tags = self
            .model
            .asset_tags
            .values()
            .flat_map(|tags| tags.iter())
            .map(|s| s.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        self.tag_select_index = 0;

        if self.available_tags.is_empty() {
            // No tags to select, go straight to manual entry
            self.input_mode = InputMode::TagAdding;
            self.input.clear();
        } else {
            self.input_mode = InputMode::TagSelecting;
        }
    }

    pub fn tag_select_next(&mut self) {
        if !self.available_tags.is_empty() {
            self.tag_select_index = (self.tag_select_index + 1) % self.available_tags.len();
        }
    }

    pub fn tag_select_prev(&mut self) {
        if !self.available_tags.is_empty() {
            if self.tag_select_index == 0 {
                self.tag_select_index = self.available_tags.len() - 1;
            } else {
                self.tag_select_index -= 1;
            }
        }
    }

    pub fn tag_select_confirm(&mut self) -> crate::error::Result<()> {
        if let Some(tag) = self.available_tags.get(self.tag_select_index) {
            let tag = tag.clone();

            // Check if we have marked assets for bulk operation
            let targets: Vec<String> = if !self.inventory.marked_assets.is_empty() {
                self.inventory.marked_assets.iter().cloned().collect()
            } else {
                // Single asset - currently selected
                match self.selected_asset() {
                    Some(asset) => vec![asset.name.clone()],
                    None => return Ok(()),
                }
            };

            let count = targets.len();
            for asset_name in targets {
                crate::app::tags::add_tag(&self.storage, &asset_name, &tag)?;
            }

            if count > 1 {
                self.set_status(format!("✓ Tagged {} assets with '{}'", count, tag));
                self.inventory.clear_marks();
            } else {
                self.set_status(format!("✓ Tagged with '{}'", tag));
            }

            self.refresh()?;
            self.input_mode = InputMode::Normal;
        }
        Ok(())
    }

    pub fn add_tag_to_selected(&mut self) -> crate::error::Result<()> {
        let tag = self.input.trim().to_string();
        if tag.is_empty() {
            return Ok(());
        }

        // Check if we have marked assets for bulk operation
        let targets: Vec<String> = if !self.inventory.marked_assets.is_empty() {
            self.inventory.marked_assets.iter().cloned().collect()
        } else {
            // Single asset - currently selected
            match self.selected_asset() {
                Some(asset) => vec![asset.name.clone()],
                None => return Ok(()),
            }
        };

        let count = targets.len();
        for asset_name in targets {
            crate::app::tags::add_tag(&self.storage, &asset_name, &tag)?;
        }

        if count > 1 {
            self.set_status(format!("✓ Tagged {} assets with '{}'", count, tag));
            self.inventory.clear_marks();
        } else {
            self.set_status(format!("✓ Tagged with '{}'", tag));
        }

        self.refresh()?;
        Ok(())
    }

    pub fn remove_tag_from_selected(&mut self) -> crate::error::Result<()> {
        let tag = self.input.trim().to_string();
        if tag.is_empty() {
            return Ok(());
        }

        // Check if we have marked assets for bulk operation
        let targets: Vec<String> = if !self.inventory.marked_assets.is_empty() {
            self.inventory.marked_assets.iter().cloned().collect()
        } else {
            // Single asset - currently selected
            match self.selected_asset() {
                Some(asset) => vec![asset.name.clone()],
                None => return Ok(()),
            }
        };

        let count = targets.len();
        for asset_name in targets {
            crate::app::tags::remove_tag(&self.storage, &asset_name, &tag)?;
        }

        if count > 1 {
            self.set_status(format!("✓ Removed '{}' from {} assets", tag, count));
            self.inventory.clear_marks();
        } else {
            self.set_status(format!("✓ Removed tag '{}'", tag));
        }

        self.refresh()?;
        Ok(())
    }

    pub fn start_tag_remove_selection(&mut self) {
        let asset_name = match self.selected_asset() {
            Some(asset) => asset.name.clone(),
            None => return,
        };

        // Get tags for this specific asset
        self.available_tags = self
            .model
            .asset_tags
            .get(&asset_name)
            .map(|tags| tags.iter().cloned().collect())
            .unwrap_or_default();

        self.tag_select_index = 0;

        if self.available_tags.is_empty() {
            // No tags to remove
            self.input_mode = InputMode::Normal;
        } else {
            self.input_mode = InputMode::TagRemovingSelecting;
        }
    }

    pub fn tag_remove_select_confirm(&mut self) -> crate::error::Result<()> {
        if let Some(tag) = self.available_tags.get(self.tag_select_index) {
            let tag = tag.clone();

            // Check if we have marked assets for bulk operation
            let targets: Vec<String> = if !self.inventory.marked_assets.is_empty() {
                self.inventory.marked_assets.iter().cloned().collect()
            } else {
                // Single asset - currently selected
                match self.selected_asset() {
                    Some(asset) => vec![asset.name.clone()],
                    None => return Ok(()),
                }
            };

            let count = targets.len();
            for asset_name in targets {
                crate::app::tags::remove_tag(&self.storage, &asset_name, &tag)?;
            }

            if count > 1 {
                self.set_status(format!("✓ Removed '{}' from {} assets", tag, count));
                self.inventory.clear_marks();
            } else {
                self.set_status(format!("✓ Removed tag '{}'", tag));
            }

            self.refresh()?;
            self.input_mode = InputMode::Normal;
        }
        Ok(())
    }
}
