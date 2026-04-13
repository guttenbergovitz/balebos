//! TUI application state and logic.

use std::sync::{Arc, Mutex};

use crate::error::BalebosError;

use crate::org::BrandId;
use crate::storage::{StorageEngine, StorageEngineExt};
use crate::tui::model::{load_tui_model, OrgModel, TuiModel};
use crate::tui::state::{
    FindingsState, FormState, InventoryState, OrgPane, OrgState, ProjectsState,
};
use crate::tui::workflows::{ContractorAssignWorkflow, RoleAssignWorkflow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Inventory,
    Organization,
    Projects,
    Findings,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Inventory,
            Tab::Organization,
            Tab::Projects,
            Tab::Findings,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Inventory => "Inventory",
            Tab::Organization => "Organization",
            Tab::Projects => "Projects",
            Tab::Findings => "Findings",
        }
    }
}

/// Type of entity being added in OrgAdding mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrgAddType {
    Person,
    Company,
    Brand,
    Project,
}

impl OrgAddType {
    pub fn title(&self) -> &'static str {
        match self {
            OrgAddType::Person => "Add Person",
            OrgAddType::Company => "Add Company",
            OrgAddType::Brand => "Add Brand",
            OrgAddType::Project => "Add Project",
        }
    }

    pub fn fields(&self) -> &'static [&'static str] {
        match self {
            OrgAddType::Person => &["First Name", "Last Name", "Email", "Phone (optional)"],
            OrgAddType::Company => &["Name", "Country Code", "Brand ID (empty=external)"],
            OrgAddType::Brand => &["Name"],
            OrgAddType::Project => &["Name", "Company ID", "Importance (1-5, optional)"],
        }
    }

    pub fn is_id_field(&self, index: usize) -> bool {
        match self {
            OrgAddType::Company => index == 2,
            OrgAddType::Project => index == 1,
            _ => false,
        }
    }

    pub fn get_candidates(&self, index: usize, app: &App) -> Vec<(String, String)> {
        match self {
            OrgAddType::Company if index == 2 => app
                .org_model
                .brands
                .iter()
                .map(|b| (b.name.clone(), b.id.to_string()))
                .collect(),
            OrgAddType::Project if index == 1 => app
                .org_model
                .companies
                .iter()
                .map(|c| (c.name.clone(), c.id.to_string()))
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Normal,
    Editing,
    Filtering,
    TagAdding,
    TagSelecting,         // Selecting existing tag to add to asset
    TagFilterSelecting,   // Selecting tag for filtering view
    TagRemoving,          // Manual tag removal (typing)
    TagRemovingSelecting, // NEW: Selecting tag to remove from asset
    OrgAdding(OrgAddType),
    OrgEditing(OrgAddType, usize), // (type, selected_index)
    Assigning,
    RoleAssigning(RoleAssignStep),
    ContractorAssigning(ContractorAssignStep),
    AssetDetails,
    LifecycleEditing,     // Editing asset lifecycle state
}

// Re-export workflow step types for convenience
pub use crate::tui::workflows::{ContractorAssignStep, RoleAssignStep};

/// Visible items in Inventory table (tree structure)
pub struct App {
    pub model: TuiModel,
    pub org_model: OrgModel,
    pub storage: Arc<dyn StorageEngine>,
    pub should_quit: bool,

    // Tab navigation
    pub active_tab: Tab,

    // Composed state components
    pub inventory: InventoryState,
    pub organization: OrgState,
    pub projects: ProjectsState,
    pub findings: FindingsState,

    // Input handling
    pub input: String,
    pub filter: String,
    pub input_mode: InputMode,

    // Form state for OrgAdding mode
    pub form: FormState,

    // Workflow states for complex multi-step operations
    pub role_assign_workflow: RoleAssignWorkflow,
    pub contractor_assign_workflow: ContractorAssignWorkflow,

    pub asset_details_scroll: u16,

    // Scanning state
    pub is_scanning: Arc<Mutex<bool>>,
    pub scan_status: Arc<Mutex<(String, chrono::DateTime<chrono::Utc>)>>,
    pub ignore_patterns: Vec<String>,

    // Tag selection state
    pub tag_select_index: usize,
    pub available_tags: Vec<String>,

    // Status message (for toasts/notifications)
    pub status_message: Option<(String, chrono::DateTime<chrono::Utc>)>,
}

impl App {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Result<Self, BalebosError> {
        let model = load_tui_model(&storage)?;
        let org_model = OrgModel::load(&storage)?;
        let ignore_patterns = crate::ignore::load_ignore_patterns("balebos.ignore");

        let mut app = Self {
            model,
            org_model,
            storage,
            should_quit: false,

            // Tab navigation
            active_tab: Tab::default(),

            // State components
            inventory: InventoryState::new(),
            organization: OrgState::new(),
            projects: ProjectsState::new(),
            findings: FindingsState::new(),

            // Input handling
            input: String::new(),
            filter: String::new(),
            input_mode: InputMode::default(),

            // Form state
            form: FormState::new(0),

            // Workflow states
            role_assign_workflow: RoleAssignWorkflow::new(),
            contractor_assign_workflow: ContractorAssignWorkflow::new(),

            asset_details_scroll: 0,

            // Scanning state
            is_scanning: Arc::new(Mutex::new(false)),
            scan_status: Arc::new(Mutex::new((String::new(), chrono::Utc::now()))),
            ignore_patterns,

            // Tag selection state
            tag_select_index: 0,
            available_tags: vec![],

            // Status message
            status_message: None,
        };

        app.update_visible_items();
        app.update_org_selection();

        if !app.inventory.visible_items.is_empty() {
            app.inventory.table_state.select(Some(0));
        }

        Ok(app)
    }

    pub fn refresh(&mut self) -> Result<(), BalebosError> {
        self.model = load_tui_model(&self.storage)?;
        self.org_model = OrgModel::load(&self.storage)?;
        self.update_visible_items();
        self.update_org_selection();
        Ok(())
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.active_tab = tab;
        self.update_org_selection();
    }

    pub fn next_tab(&mut self) {
        let tabs = Tab::all();
        let current_idx = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);
        let next_idx = (current_idx + 1) % tabs.len();
        self.set_tab(tabs[next_idx]);
    }

    pub fn prev_tab(&mut self) {
        let tabs = Tab::all();
        let current_idx = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            tabs.len() - 1
        } else {
            current_idx - 1
        };
        self.set_tab(tabs[prev_idx]);
    }

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
        if self.active_tab == Tab::Organization {
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

    // -----------------------------------------------------------------------
    // Form handling for OrgAdding mode
    // -----------------------------------------------------------------------

    pub fn start_org_add(&mut self, add_type: OrgAddType) {
        let field_count = add_type.fields().len();
        self.form = FormState::new(field_count);
        self.input_mode = InputMode::OrgAdding(add_type);
        self.update_select_matches();
    }

    pub fn start_org_edit(&mut self, add_type: OrgAddType, index: usize) {
        let mut form = FormState::new(add_type.fields().len());
        match add_type {
            OrgAddType::Person => {
                if let Some(p) = self.org_model.persons.get(index) {
                    form.set_field_value(0, p.first_name.clone());
                    form.set_field_value(1, p.last_name.clone());
                    form.set_field_value(2, p.email.clone());
                    form.set_field_value(3, p.phone.clone().unwrap_or_default());
                }
            }
            OrgAddType::Company => {
                if let Some(c) = self.org_model.companies.get(index) {
                    form.set_field_value(0, c.name.clone());
                    form.set_field_value(1, c.country.alpha2().to_string());
                    form.set_field_value(
                        2,
                        c.brand
                            .map(|id: BrandId| id.to_string())
                            .unwrap_or_default(),
                    );
                }
            }
            OrgAddType::Brand => {
                if let Some(b) = self.org_model.brands.get(index) {
                    form.set_field_value(0, b.name.clone());
                }
            }
            OrgAddType::Project => {
                if let Some(p) = self.org_model.projects.get(index) {
                    form.set_field_value(0, p.name.clone());
                    form.set_field_value(1, p.company.to_string());
                    form.set_field_value(
                        2,
                        p.importance.map(|i: u8| i.to_string()).unwrap_or_default(),
                    );
                }
            }
        }
        self.form = form;
        self.input_mode = InputMode::OrgEditing(add_type, index);
        self.update_select_matches();
    }

    pub fn form_next_field(&mut self) {
        if let InputMode::OrgAdding(add_type) | InputMode::OrgEditing(add_type, _) =
            &self.input_mode
        {
            let max = add_type.fields().len();
            if self.form.next_field(max) {
                self.update_select_matches();
            }
        }
    }

    pub fn form_prev_field(&mut self) {
        if self.form.previous_field() {
            self.update_select_matches();
        }
    }

    pub fn org_select_next(&mut self) {
        self.form.select_next();
    }

    pub fn org_select_prev(&mut self) {
        self.form.select_previous();
    }

    pub fn update_select_matches(&mut self) {
        let mode = match &self.input_mode {
            InputMode::OrgAdding(t) => Some(*t),
            InputMode::OrgEditing(t, _) => Some(*t),
            InputMode::Assigning => {
                let current_input = self.input.to_lowercase();
                let matches = self
                    .org_model
                    .projects
                    .iter()
                    .map(|p| {
                        let brand_name = self
                            .org_model
                            .companies_by_id
                            .get(&p.company)
                            .and_then(|c| c.brand)
                            .and_then(|b_id| self.org_model.brands_by_id.get(&b_id))
                            .map(|b| b.name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());
                        (format!("{} ({})", p.name, brand_name), p.id.to_string())
                    })
                    .filter(|(display, _id): &(String, String)| {
                        display.to_lowercase().contains(&current_input)
                    })
                    .collect();

                self.form.set_select_matches(matches);
                return;
            }
            _ => None,
        };

        if let Some(add_type) = mode {
            if add_type.is_id_field(self.form.field_index) {
                let current_input = self
                    .form
                    .get_field_value(self.form.field_index)
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();

                let candidates = add_type.get_candidates(self.form.field_index, self);
                let matches = candidates
                    .into_iter()
                    .filter(|(name, _id)| name.to_lowercase().contains(&current_input))
                    .collect();

                self.form.set_select_matches(matches);
            } else {
                self.form.reset_select();
            }
        }
    }

    #[allow(dead_code)]
    pub fn form_current_value(&self) -> &str {
        self.form.current_value()
    }

    pub fn form_push_char(&mut self, c: char) {
        self.form.push_char(c);
        self.update_select_matches();
    }

    pub fn form_pop_char(&mut self) {
        self.form.pop_char();
        self.update_select_matches();
    }

    pub fn cancel_form(&mut self) {
        self.input_mode = InputMode::Normal;
        self.form.reset();
    }

    /// Submit the org form and create the entity.
    /// Returns Ok(true) if successful, Ok(false) if validation failed.
    pub fn submit_org_form(&mut self) -> Result<bool, BalebosError> {
        // If we have a selection, pick it instead of submitting the form
        if let Some((_, id)) = self.form.selected_match() {
            let id = id.clone();
            self.form.set_field_value(self.form.field_index, id);
            self.form.reset_select();
            // Auto-advance to next field if possible
            self.form_next_field();
            return Ok(true);
        }

        let mode = self.input_mode.clone();
        let (add_type, edit_index) = match &mode {
            InputMode::OrgAdding(t) => (*t, None),
            InputMode::OrgEditing(t, idx) => (*t, Some(*idx)),
            _ => return Ok(false),
        };

        let success = match add_type {
            OrgAddType::Person => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.persons.get(idx) {
                        self.submit_person_edit(existing.id)?
                    } else {
                        false
                    }
                } else {
                    self.submit_person_form()?
                }
            }
            OrgAddType::Company => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.companies.get(idx) {
                        self.submit_company_edit(existing.id)?
                    } else {
                        false
                    }
                } else {
                    self.submit_company_form()?
                }
            }
            OrgAddType::Brand => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.brands.get(idx) {
                        self.submit_brand_edit(existing.id)?
                    } else {
                        false
                    }
                } else {
                    self.submit_brand_form()?
                }
            }
            OrgAddType::Project => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.projects.get(idx) {
                        self.submit_project_edit(existing.id)?
                    } else {
                        false
                    }
                } else {
                    self.submit_project_form()?
                }
            }
        };

        if success {
            self.input_mode = InputMode::Normal;
            self.refresh()?;
        }
        Ok(success)
    }

    fn submit_person_form(&mut self) -> Result<bool, BalebosError> {
        let first_name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let last_name = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let email = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");
        let phone_str = self.form.get_field_value(3).map(|s| s.trim()).unwrap_or("");
        let phone = if phone_str.is_empty() {
            None
        } else {
            Some(phone_str.to_string())
        };

        if first_name.is_empty() || last_name.is_empty() || email.is_empty() {
            return Ok(false);
        }

        crate::app::org::add_person(&self.storage, first_name, last_name, email, phone)?;
        Ok(true)
    }

    fn submit_person_edit(&mut self, id: crate::org::PersonId) -> Result<bool, BalebosError> {
        let first_name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let last_name = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let email = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");
        let phone_str = self.form.get_field_value(3).map(|s| s.trim()).unwrap_or("");
        let phone = if phone_str.is_empty() {
            None
        } else {
            Some(phone_str.to_string())
        };

        if first_name.is_empty() || last_name.is_empty() || email.is_empty() {
            return Ok(false);
        }

        crate::app::org::update_person(&self.storage, id, first_name, last_name, email, phone)?;
        Ok(true)
    }

    fn submit_company_form(&mut self) -> Result<bool, BalebosError> {
        use crate::org::{BrandId, CountryCode};

        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let country_str = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let brand_str = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");

        if name.is_empty() || country_str.is_empty() {
            return Ok(false);
        }

        let country = match CountryCode::for_alpha2(country_str) {
            Ok(c) => c,
            Err(_) => return Ok(false), // Invalid country code
        };

        // Brand is optional - empty means external contractor
        let brand: Option<BrandId> = if brand_str.is_empty() {
            None
        } else {
            match brand_str.parse() {
                Ok(id) => Some(id),
                Err(_) => return Ok(false), // Invalid brand ID
            }
        };

        crate::app::org::add_company(&self.storage, name, country, brand)?;
        Ok(true)
    }

    fn submit_company_edit(&mut self, id: crate::org::CompanyId) -> Result<bool, BalebosError> {
        use crate::org::{BrandId, CountryCode};

        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let country_str = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let brand_str = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");

        if name.is_empty() || country_str.is_empty() {
            return Ok(false);
        }

        let country = match CountryCode::for_alpha2(country_str) {
            Ok(c) => c,
            Err(_) => return Ok(false), // Invalid country code
        };

        let brand: Option<BrandId> = if brand_str.is_empty() {
            None
        } else {
            match brand_str.parse() {
                Ok(id) => Some(id),
                Err(_) => return Ok(false), // Invalid brand ID
            }
        };

        crate::app::org::update_company(&self.storage, id, name, country, brand)?;
        Ok(true)
    }

    fn submit_brand_form(&mut self) -> Result<bool, BalebosError> {
        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");

        // Validation
        if name.is_empty() {
            return Ok(false);
        }

        crate::app::org::add_brand(&self.storage, name)?;
        Ok(true)
    }

    fn submit_brand_edit(&mut self, id: crate::org::BrandId) -> Result<bool, BalebosError> {
        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");

        if name.is_empty() {
            return Ok(false);
        }

        crate::app::org::update_brand(&self.storage, id, name)?;
        Ok(true)
    }

    fn parse_importance(&self) -> Option<u8> {
        let imp_str = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");
        if imp_str.is_empty() {
            return None;
        }
        match imp_str.parse::<u8>() {
            Ok(v) if (1..=5).contains(&v) => Some(v),
            _ => None,
        }
    }

    fn submit_project_form(&mut self) -> Result<bool, BalebosError> {
        use crate::org::CompanyId;

        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let company_id_str = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let importance = self.parse_importance();

        if name.is_empty() || company_id_str.is_empty() {
            return Ok(false);
        }

        let company_id = match company_id_str.parse::<CompanyId>() {
            Ok(id) => id,
            Err(_) => return Ok(false),
        };

        crate::app::org::add_project_with_importance(&self.storage, name, company_id, importance)?;
        Ok(true)
    }

    fn submit_project_edit(&mut self, id: crate::org::ProjectId) -> Result<bool, BalebosError> {
        use crate::org::CompanyId;

        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let company_id_str = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let importance = self.parse_importance();

        if name.is_empty() || company_id_str.is_empty() {
            return Ok(false);
        }

        let company_id = match company_id_str.parse::<CompanyId>() {
            Ok(id) => id,
            Err(_) => return Ok(false),
        };

        crate::app::org::set_project_importance(&self.storage, id, name, company_id, importance)?;
        Ok(true)
    }

    // -----------------------------------------------------------------------
    // Inventory table expansion and selection
    // -----------------------------------------------------------------------

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

    pub fn set_status(&mut self, message: String) {
        self.status_message = Some((message, chrono::Utc::now()));
    }

    pub fn clear_old_status(&mut self) {
        if let Some((_, timestamp)) = &self.status_message {
            let elapsed = chrono::Utc::now().signed_duration_since(*timestamp);
            if elapsed.num_seconds() > 3 {
                self.status_message = None;
            }
        }
    }

    pub fn selected_asset(&self) -> Option<&AssetState> {
        self.inventory.selected_asset(&self.model)
    }

    // -----------------------------------------------------------------------
    // Asset assignment to Project
    // -----------------------------------------------------------------------

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

    pub fn toggle_mark(&mut self) {
        if let Some(asset) = self.selected_asset() {
            self.inventory.toggle_mark(asset.name.clone());
        }
    }

    pub fn clear_marks(&mut self) {
        self.inventory.clear_marks();
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

    // -----------------------------------------------------------------------
    // Scanning
    // -----------------------------------------------------------------------

    pub fn start_scan(&mut self) {
        if self.input.is_empty() {
            return;
        }

        let target = self.input.clone();
        self.input.clear();
        self.input_mode = InputMode::Normal;

        let storage = self.storage.clone();
        let is_scanning = self.is_scanning.clone();
        let scan_status = self.scan_status.clone();
        let ignore_patterns = self.ignore_patterns.clone();

        std::thread::spawn(move || {
            {
                let mut s = is_scanning.lock().unwrap_or_else(|p| p.into_inner());
                *s = true;
            }

            let res = crate::app::run_single_recon(&target, &storage, &ignore_patterns);

            {
                let mut s = is_scanning.lock().unwrap_or_else(|p| p.into_inner());
                *s = false;
            }

            if let Err(e) = res {
                let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
                *status = (format!("❌ Scan error: {}", e), chrono::Utc::now());
            } else {
                let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
                *status = ("✓ Scan completed".to_string(), chrono::Utc::now());
            }
        });
    }

    pub fn start_rescan(&mut self) {
        if let Some(asset) = self.selected_asset() {
            self.input = asset.name.clone();
            self.start_scan();
        }
    }

    // ---------------------------------------------------------------------------
    // Findings tab methods
    // ---------------------------------------------------------------------------

    pub fn load_findings(&mut self) -> Result<(), BalebosError> {
        // Read all records from storage
        let records = self.storage.read_all_records()?;

        // Extract findings and events
        self.findings.all_findings = records.findings;
        self.findings.all_events = records.finding_events;
        self.findings.apply_filter();

        if !self.findings.visible_findings.is_empty() {
            self.findings.table_state.select(Some(0));
        }

        Ok(())
    }

    pub fn findings_next(&mut self) {
        self.findings.next();
    }

    pub fn findings_previous(&mut self) {
        self.findings.previous();
    }

    pub fn acknowledge_finding(&mut self) {
        if let Some(idx) = self.findings.table_state.selected() {
            if let Some(finding) = self.findings.visible_findings.get(idx) {
                let event = crate::app::finding::FindingEvent::acknowledge(&finding.id);
                if let Err(e) = self.storage.save(&event) {
                    self.set_status(format!("Error acknowledging: {}", e));
                } else {
                    self.set_status(format!("Acknowledged: {}", finding.check_name));
                    // Reload findings to reflect the change
                    let _ = self.load_findings();
                }
            }
        }
    }

    pub fn resolve_finding(&mut self) {
        if let Some(idx) = self.findings.table_state.selected() {
            if let Some(finding) = self.findings.visible_findings.get(idx) {
                let event = crate::app::finding::FindingEvent::resolve(&finding.id);
                if let Err(e) = self.storage.save(&event) {
                    self.set_status(format!("Error resolving: {}", e));
                } else {
                    self.set_status(format!("Resolved: {}", finding.check_name));
                    // Reload findings to reflect the change
                    let _ = self.load_findings();
                }
            }
        }
    }

    pub fn run_findings_scan(&mut self) {
        use crate::app::check::run_checks_and_persist;

        self.set_status("Running findings scan...".to_string());

        // Run checks in background thread
        let storage = self.storage.clone();
        let ignore_patterns = self.ignore_patterns.clone();
        let scan_status = self.scan_status.clone();

        std::thread::spawn(move || {
            let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
            *status = ("⟳ Running checks...".to_string(), chrono::Utc::now());
            drop(status);

            match run_checks_and_persist(&storage, &ignore_patterns) {
                Ok(findings) => {
                    let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
                    if findings.is_empty() {
                        *status = ("✓ No new findings".to_string(), chrono::Utc::now());
                    } else {
                        *status = (
                            format!("✓ Found {} new findings", findings.len()),
                            chrono::Utc::now(),
                        );
                    }
                }
                Err(e) => {
                    let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
                    *status = (format!("❌ Error: {}", e), chrono::Utc::now());
                }
            }
        });
    }

    pub fn toggle_findings_filter(&mut self) {
        self.findings.cycle_filter();
    }

    pub fn toggle_finding_expand(&mut self) {
        self.findings.toggle_expand();
    }
}

use crate::tui::model::AssetState;
