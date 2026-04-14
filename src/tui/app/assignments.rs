//! Assignment and project methods for TUI.

use crate::error::BalebosError;
use crate::tui::app::{App, ContractorAssignStep, InputMode, RoleAssignStep};
use crate::tui::state::OrgPane;

impl App {
    pub fn next_org_pane(&mut self) {
        self.organization.next_pane();
        self.update_org_selection();
    }

    pub fn prev_org_pane(&mut self) {
        self.organization.prev_pane();
        self.update_org_selection();
    }

    pub fn toggle_project_expand(&mut self) {
        if let Some(idx) = self.projects.table_state.selected() {
            let mut current_idx = 0;
            let mut target_project_id = None;

            for project in &self.org_model.project_views {
                if current_idx == idx {
                    target_project_id = Some(project.id.to_string());
                    break;
                }
                current_idx += 1;
                if self.projects.is_expanded(&project.id.to_string()) {
                    if let Some(assets) = self.org_model.assets_by_project.get(&project.id) {
                        current_idx += assets.len();
                    } else {
                        current_idx += 1;
                    }
                }
            }

            if let Some(id_str) = target_project_id {
                if self.projects.is_expanded(&id_str) {
                    self.projects.collapse(&id_str);
                } else {
                    self.projects.expand(id_str);
                }
            }
        }
    }

    pub fn projects_next(&mut self) {
        let total_rows = self.projects_total_rows();
        self.projects.next(total_rows);
    }

    pub fn projects_previous(&mut self) {
        let total_rows = self.projects_total_rows();
        self.projects.previous(total_rows);
    }

    fn projects_total_rows(&self) -> usize {
        let mut count = 0;
        for project in &self.org_model.project_views {
            count += 1;
            if self.projects.is_expanded(&project.id.to_string()) {
                if let Some(assets) = self.org_model.assets_by_project.get(&project.id) {
                    count += assets.len();
                } else {
                    count += 1;
                }
            }
        }
        count
    }

    pub fn update_org_selection(&mut self) {
        if self.active_tab == super::Tab::Organization {
            let count = self.org_item_count();
            self.organization.update_selection(count);
        }
    }

    pub fn org_item_count(&self) -> usize {
        match self.organization.pane {
            OrgPane::Persons => self.org_model.persons.len(),
            OrgPane::Companies => self.org_model.companies.len(),
            OrgPane::Brands => self.org_model.brands.len(),
        }
    }

    pub fn start_assign_asset(&mut self) {
        if self.selected_asset().is_some() {
            self.input_mode = InputMode::Assigning;
            self.input.clear();
            self.form.reset_select();
            self.update_select_matches();
        }
    }

    pub fn submit_asset_assignment(&mut self) -> Result<bool, BalebosError> {
        // Handle selection from list
        let project_id = if let Some((_, id_str)) = self.form.selected_match() {
            match id_str.parse::<crate::org::ProjectId>() {
                Ok(id) => Some(id),
                Err(_) => return Ok(false),
            }
        } else {
            let project_id_str = self.input.trim();
            if project_id_str.is_empty() {
                return Ok(false);
            }
            match project_id_str.parse::<crate::org::ProjectId>() {
                Ok(id) => Some(id),
                Err(_) => return Ok(false),
            }
        };

        let project_id = match project_id {
            Some(id) => id,
            None => return Ok(false),
        };

        // Batch assign marked assets if any, otherwise assign selected asset
        let targets: Vec<String> = if !self.inventory.marked_assets.is_empty() {
            self.inventory.marked_assets.iter().cloned().collect()
        } else if let Some(asset) = self.selected_asset() {
            vec![asset.name.clone()]
        } else {
            Vec::new()
        };

        if targets.is_empty() {
            return Ok(false);
        }

        for host in targets {
            crate::app::org::assign_asset(&self.storage, &host, project_id)?;
        }

        self.input_mode = InputMode::Normal;
        self.input.clear();
        self.form.reset_select();
        self.inventory.clear_marks();
        self.refresh()?;
        Ok(true)
    }

    pub fn assign_push_char(&mut self, c: char) {
        self.input.push(c);
        self.update_select_matches();
    }

    pub fn assign_pop_char(&mut self) {
        self.input.pop();
        self.update_select_matches();
    }

    // -----------------------------------------------------------------------
    // Role assignment
    // -----------------------------------------------------------------------

    pub fn start_role_assign(&mut self) {
        if let Some(idx) = self.projects.table_state.selected() {
            // Resolve actual project from flat rows
            let mut current_idx = 0;
            let mut selected_project = None;
            for project in &self.org_model.project_views {
                if current_idx == idx {
                    selected_project = Some(project);
                    break;
                }
                current_idx += 1;
                if self.projects.is_expanded(&project.id.to_string()) {
                    if let Some(assets) = self.org_model.assets_by_project.get(&project.id) {
                        current_idx += assets.len();
                    } else {
                        current_idx += 1;
                    }
                }
            }

            if let Some(project) = selected_project {
                self.role_assign_workflow.start(project.id);
                self.input.clear();
                self.form.select_index = None;
                self.update_role_assign_matches();
                self.input_mode = InputMode::RoleAssigning(RoleAssignStep::SelectTarget);
            }
        }
    }

    pub fn update_role_assign_matches(&mut self) {
        let filter = self.input.to_lowercase();

        let matches = self
            .org_model
            .persons
            .iter()
            .map(|p| {
                (
                    format!("{} {}", p.first_name, p.last_name),
                    p.id.to_string(),
                )
            })
            .filter(|(name, _): &(String, String)| name.to_lowercase().contains(&filter))
            .collect();

        self.form.set_select_matches(matches);
    }

    pub fn role_assign_pick_target(&mut self) -> bool {
        if let Some((_, id)) = self.form.selected_match() {
            self.role_assign_workflow.target = Some(id.clone());
            self.form.select_index = Some(0); // For role selection
            self.input_mode = InputMode::RoleAssigning(RoleAssignStep::SelectRole);
            return true;
        }
        false
    }

    pub fn role_assign_submit(&mut self, role: crate::org::Role) -> Result<(), BalebosError> {
        let project_id = self
            .role_assign_workflow
            .project
            .ok_or(BalebosError::MissingField("project".to_string()))?;
        let target_id_str = self
            .role_assign_workflow
            .target
            .as_ref()
            .ok_or(BalebosError::MissingField("target".to_string()))?;

        let person_id = target_id_str
            .parse::<crate::org::PersonId>()
            .map_err(|_| BalebosError::InvalidId("person ID".to_string()))?;
        crate::app::org::assign_person_to_project(&self.storage, person_id, project_id, role)?;

        self.input_mode = InputMode::Normal;
        self.input.clear();
        self.role_assign_workflow.reset();
        self.refresh()?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Contractor assignment
    // -----------------------------------------------------------------------

    pub fn start_contractor_assign(&mut self) {
        if let Some(idx) = self.projects.table_state.selected() {
            let mut current_idx = 0;
            let mut selected_project = None;
            for project in &self.org_model.project_views {
                if current_idx == idx {
                    selected_project = Some(project);
                    break;
                }
                current_idx += 1;
                if self.projects.is_expanded(&project.id.to_string()) {
                    if let Some(assets) = self.org_model.assets_by_project.get(&project.id) {
                        current_idx += assets.len();
                    } else {
                        current_idx += 1;
                    }
                }
            }

            if let Some(project) = selected_project {
                self.contractor_assign_workflow.start(project.id);
                self.input.clear();
                self.form.select_index = None;
                self.update_contractor_assign_matches();
                self.input_mode =
                    InputMode::ContractorAssigning(ContractorAssignStep::SelectCompany);
            }
        }
    }

    pub fn update_contractor_assign_matches(&mut self) {
        let filter = self.input.to_lowercase();

        let matches = self
            .org_model
            .companies
            .iter()
            .filter(|c: &&crate::org::CompanyRecord| c.is_external())
            .map(|c| (c.name.clone(), c.id.to_string()))
            .filter(|(name, _): &(String, String)| name.to_lowercase().contains(&filter))
            .collect();

        self.form.set_select_matches(matches);
    }

    pub fn contractor_assign_pick_company(&mut self) -> bool {
        if let Some((_, id)) = self.form.selected_match() {
            self.contractor_assign_workflow.company = Some(id.clone());
            self.form.select_index = Some(0);
            self.input_mode = InputMode::ContractorAssigning(ContractorAssignStep::SelectRole);
            return true;
        }
        false
    }

    pub fn contractor_assign_submit(
        &mut self,
        role: crate::org::ContractorRole,
    ) -> Result<(), BalebosError> {
        let project_id = self
            .contractor_assign_workflow
            .project
            .ok_or(BalebosError::MissingField("project".to_string()))?;
        let company_id_str = self
            .contractor_assign_workflow
            .company
            .as_ref()
            .ok_or(BalebosError::MissingField("company".to_string()))?;

        let company_id = company_id_str
            .parse::<crate::org::CompanyId>()
            .map_err(|_| BalebosError::InvalidId("company ID".to_string()))?;
        crate::app::org::assign_contractor_to_project(&self.storage, company_id, project_id, role)?;

        self.input_mode = InputMode::Normal;
        self.input.clear();
        self.contractor_assign_workflow.reset();
        self.refresh()?;
        Ok(())
    }
}
