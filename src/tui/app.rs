//! TUI application state and logic.

use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex};
use ratatui::widgets::TableState;

use crate::org::{BrandId, CompanyId, ProjectId};
use crate::storage::JsonlStorage;
use crate::tui::model::{load_tui_model, OrgModel, TuiModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Inventory,
    Organization,
    Projects,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Inventory, Tab::Organization, Tab::Projects]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Inventory => "Inventory",
            Tab::Organization => "Organization",
            Tab::Projects => "Projects",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrgPane {
    #[default]
    Persons,
    Companies,
    Brands,
}

impl OrgPane {
    pub fn all() -> &'static [OrgPane] {
        &[OrgPane::Persons, OrgPane::Companies, OrgPane::Brands]
    }

    pub fn title(&self) -> &'static str {
        match self {
            OrgPane::Persons => "Persons",
            OrgPane::Companies => "Companies",
            OrgPane::Brands => "Brands",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            OrgPane::Persons => OrgPane::Companies,
            OrgPane::Companies => OrgPane::Brands,
            OrgPane::Brands => OrgPane::Persons,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            OrgPane::Persons => OrgPane::Brands,
            OrgPane::Companies => OrgPane::Persons,
            OrgPane::Brands => OrgPane::Companies,
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
    OrgAdding(OrgAddType),
    OrgEditing(OrgAddType, usize), // (type, selected_index)
    Assigning,
    RoleAssigning(RoleAssignStep),
    ContractorAssigning(ContractorAssignStep),
    AssetDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleAssignStep {
    SelectTarget, // Person
    SelectRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractorAssignStep {
    SelectCompany,
    SelectRole,
}

/// Visible items in Inventory table (tree structure)
pub enum VisibleItem {
    Root(String),
    Subdomain(String, String),
}

pub struct App {
    pub model: TuiModel,
    pub org_model: OrgModel,
    pub storage: JsonlStorage,
    pub should_quit: bool,

    // Tab navigation
    pub active_tab: Tab,
    pub org_pane: OrgPane,

    // Inventory tab state
    pub table_state: TableState,
    pub expanded_roots: HashSet<String>,
    pub visible_items: Vec<VisibleItem>,
    pub hide_external_redirects: bool,
    pub marked_assets: HashSet<String>,

    // Organization tab state
    pub org_table_state: TableState,

    // Projects tab state
    pub projects_table_state: TableState,
    pub expanded_projects: HashSet<String>,

    // Input handling
    pub input: String,
    pub filter: String,
    pub input_mode: InputMode,

    // Form state for OrgAdding mode
    pub form_fields: Vec<String>,
    pub form_field_index: usize,
    pub org_select_index: Option<usize>,
    pub org_select_matches: Vec<(String, String)>, // (Name, ID)

    // Role assignment state
    pub role_assign_project: Option<crate::org::ProjectId>,
    pub role_assign_type: Option<bool>, // true=Person, false=Company
    pub role_assign_target: Option<String>, // ID string
    pub role_assign_role: Option<crate::org::Role>,

    // Contractor assignment state
    pub contractor_assign_project: Option<crate::org::ProjectId>,
    pub contractor_assign_company: Option<String>, // CompanyId string
    pub contractor_assign_role: Option<crate::org::ContractorRole>,

    pub asset_details_scroll: u16,

    // Scanning state
    pub is_scanning: Arc<Mutex<bool>>,
    pub scan_status: Arc<Mutex<String>>,
    pub ignore_patterns: Vec<String>,
}

impl App {
    pub fn new(storage: JsonlStorage) -> Result<Self, io::Error> {
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
            org_pane: OrgPane::default(),

            // Inventory tab state
            table_state: TableState::default(),
            expanded_roots: HashSet::new(),
            visible_items: Vec::new(),
            hide_external_redirects: false,
            marked_assets: HashSet::new(),

            // Organization tab state
            org_table_state: TableState::default(),

            // Projects tab state
            projects_table_state: TableState::default(),
            expanded_projects: HashSet::new(),

            // Input handling
            input: String::new(),
            filter: String::new(),
            input_mode: InputMode::default(),

            // Form state
            form_fields: Vec::new(),
            form_field_index: 0,
            org_select_index: None,
            org_select_matches: Vec::new(),

            role_assign_project: None,
            role_assign_type: None,
            role_assign_target: None,
            role_assign_role: None,

            contractor_assign_project: None,
            contractor_assign_company: None,
            contractor_assign_role: None,

            asset_details_scroll: 0,

            // Scanning state
            is_scanning: Arc::new(Mutex::new(false)),
            scan_status: Arc::new(Mutex::new(String::new())),
            ignore_patterns,
        };

        app.update_visible_items();
        app.update_org_selection();

        if !app.visible_items.is_empty() {
            app.table_state.select(Some(0));
        }

        Ok(app)
    }

    pub fn refresh(&mut self) -> io::Result<()> {
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
        let current_idx = tabs.iter().position(|t| *t == self.active_tab).unwrap();
        let next_idx = (current_idx + 1) % tabs.len();
        self.set_tab(tabs[next_idx]);
    }

    pub fn prev_tab(&mut self) {
        let tabs = Tab::all();
        let current_idx = tabs.iter().position(|t| *t == self.active_tab).unwrap();
        let prev_idx = if current_idx == 0 {
            tabs.len() - 1
        } else {
            current_idx - 1
        };
        self.set_tab(tabs[prev_idx]);
    }

    pub fn next_org_pane(&mut self) {
        self.org_pane = self.org_pane.next();
        self.update_org_selection();
    }

    pub fn prev_org_pane(&mut self) {
        self.org_pane = self.org_pane.prev();
        self.update_org_selection();
    }

    pub fn toggle_project_expand(&mut self) {
        if let Some(idx) = self.projects_table_state.selected() {
            let mut current_idx = 0;
            let mut target_project_id = None;

            for project in &self.org_model.project_views {
                if current_idx == idx {
                    target_project_id = Some(project.id.to_string());
                    break;
                }
                current_idx += 1;
                if self.expanded_projects.contains(&project.id.to_string()) {
                    if let Some(assets) = self.org_model.assets_by_project.get(&project.id) {
                        current_idx += assets.len();
                    } else {
                        current_idx += 1;
                    }
                }
            }

            if let Some(id_str) = target_project_id {
                if self.expanded_projects.contains(&id_str) {
                    self.expanded_projects.remove(&id_str);
                } else {
                    self.expanded_projects.insert(id_str);
                }
            }
        }
    }

    pub fn projects_next(&mut self) {
        let total_rows = self.projects_total_rows();
        if total_rows == 0 { return; }
        let i = match self.projects_table_state.selected() {
            Some(i) => if i >= total_rows - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.projects_table_state.select(Some(i));
    }

    pub fn projects_previous(&mut self) {
        let total_rows = self.projects_total_rows();
        if total_rows == 0 { return; }
        let i = match self.projects_table_state.selected() {
            Some(i) => if i == 0 { total_rows - 1 } else { i - 1 },
            None => 0,
        };
        self.projects_table_state.select(Some(i));
    }

    fn projects_total_rows(&self) -> usize {
        let mut count = 0;
        for project in &self.org_model.project_views {
            count += 1;
            if self.expanded_projects.contains(&project.id.to_string()) {
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
            if count > 0 {
                if self.org_table_state.selected().is_none() {
                    self.org_table_state.select(Some(0));
                } else if self.org_table_state.selected().unwrap() >= count {
                    self.org_table_state.select(Some(count - 1));
                }
            } else {
                self.org_table_state.select(None);
            }
        }
    }

    pub fn org_item_count(&self) -> usize {
        match self.org_pane {
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
        self.form_fields = vec![String::new(); field_count];
        self.form_field_index = 0;
        self.org_select_index = None;
        self.org_select_matches = Vec::new();
        self.input_mode = InputMode::OrgAdding(add_type);
        self.update_select_matches();
    }

    pub fn start_org_edit(&mut self, add_type: OrgAddType, index: usize) {
        match add_type {
            OrgAddType::Person => {
                if let Some(p) = self.org_model.persons.get(index) {
                    self.form_fields = vec![
                        p.first_name.clone(),
                        p.last_name.clone(),
                        p.email.clone(),
                        p.phone.clone().unwrap_or_default(),
                    ];
                }
            }
            OrgAddType::Company => {
                if let Some(c) = self.org_model.companies.get(index) {
                    self.form_fields = vec![
                        c.name.clone(),
                        c.country.alpha2().to_string(),
                        c.brand.map(|id: BrandId| id.to_string()).unwrap_or_default(),
                    ];
                }
            }
            OrgAddType::Brand => {
                if let Some(b) = self.org_model.brands.get(index) {
                    self.form_fields = vec![b.name.clone()];
                }
            }
            OrgAddType::Project => {
                if let Some(p) = self.org_model.projects.get(index) {
                    self.form_fields = vec![
                        p.name.clone(),
                        p.company.to_string(),
                        p.importance.map(|i: u8| i.to_string()).unwrap_or_default(),
                    ];
                }
            }
        }
        self.form_field_index = 0;
        self.org_select_index = None;
        self.org_select_matches = Vec::new();
        self.input_mode = InputMode::OrgEditing(add_type, index);
        self.update_select_matches();
    }

    pub fn form_next_field(&mut self) {
        if let InputMode::OrgAdding(add_type) | InputMode::OrgEditing(add_type, _) = &self.input_mode {
            let max = add_type.fields().len();
            if self.form_field_index < max - 1 {
                self.form_field_index += 1;
                self.org_select_index = None;
                self.update_select_matches();
            }
        }
    }

    pub fn form_prev_field(&mut self) {
        if self.form_field_index > 0 {
            self.form_field_index -= 1;
            self.org_select_index = None;
            self.update_select_matches();
        }
    }

    pub fn org_select_next(&mut self) {
        if !self.org_select_matches.is_empty() {
            let i = self.org_select_index.map(|i| i + 1).unwrap_or(0);
            if i >= self.org_select_matches.len() {
                self.org_select_index = Some(0);
            } else {
                self.org_select_index = Some(i);
            }
        }
    }

    pub fn org_select_prev(&mut self) {
        if !self.org_select_matches.is_empty() {
            let i = self.org_select_index.map(|i| {
                if i == 0 {
                    self.org_select_matches.len() - 1
                } else {
                    i - 1
                }
            }).unwrap_or(self.org_select_matches.len() - 1);
            self.org_select_index = Some(i);
        }
    }

    pub fn update_select_matches(&mut self) {
        let mode = match &self.input_mode {
            InputMode::OrgAdding(t) => Some(*t),
            InputMode::OrgEditing(t, _) => Some(*t),
            InputMode::Assigning => {
                let current_input = self.input.to_lowercase();
                self.org_select_matches = self.org_model.projects.iter()
                    .map(|p| {
                        let brand_name = self.org_model.companies_by_id.get(&p.company)
                            .and_then(|c| c.brand)
                            .and_then(|b_id| self.org_model.brands_by_id.get(&b_id))
                            .map(|b| b.name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());
                        (format!("{} ({})", p.name, brand_name), p.id.to_string())
                    })
                    .filter(|(display, _id): &(String, String)| display.to_lowercase().contains(&current_input))
                    .collect();
                
                if self.org_select_matches.is_empty() {
                    self.org_select_index = None;
                } else if let Some(i) = self.org_select_index {
                    if i >= self.org_select_matches.len() {
                        self.org_select_index = Some(0);
                    }
                }
                return;
            }
            _ => None,
        };

        if let Some(add_type) = mode {
            if add_type.is_id_field(self.form_field_index) {
                let current_input = self.form_fields.get(self.form_field_index)
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                
                let candidates = add_type.get_candidates(self.form_field_index, self);
                self.org_select_matches = candidates.into_iter()
                    .filter(|(name, _id)| name.to_lowercase().contains(&current_input))
                    .collect();
                
                // Keep selection in bounds or reset if empty
                if self.org_select_matches.is_empty() {
                    self.org_select_index = None;
                } else if let Some(i) = self.org_select_index {
                    if i >= self.org_select_matches.len() {
                        self.org_select_index = Some(0);
                    }
                }
            } else {
                self.org_select_matches = Vec::new();
                self.org_select_index = None;
            }
        }
    }

    #[allow(dead_code)]
    pub fn form_current_value(&self) -> &str {
        self.form_fields
            .get(self.form_field_index)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn form_push_char(&mut self, c: char) {
        if let Some(field) = self.form_fields.get_mut(self.form_field_index) {
            field.push(c);
            self.update_select_matches();
        }
    }

    pub fn form_pop_char(&mut self) {
        if let Some(field) = self.form_fields.get_mut(self.form_field_index) {
            field.pop();
            self.update_select_matches();
        }
    }

    pub fn cancel_form(&mut self) {
        self.input_mode = InputMode::Normal;
        self.form_fields.clear();
        self.form_field_index = 0;
        self.org_select_index = None;
        self.org_select_matches.clear();
    }

    /// Submit the org form and create the entity.
    /// Returns Ok(true) if successful, Ok(false) if validation failed.
    pub fn submit_org_form(&mut self) -> io::Result<bool> {
        // If we have a selection, pick it instead of submitting the form
        if let Some(i) = self.org_select_index {
            if let Some((_name, id)) = self.org_select_matches.get(i) {
                let id = id.clone();
                if let Some(field) = self.form_fields.get_mut(self.form_field_index) {
                    *field = id;
                }
                self.org_select_index = None;
                self.update_select_matches();
                // Auto-advance to next field if possible
                self.form_next_field();
                return Ok(true);
            }
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
                    } else { false }
                } else {
                    self.submit_person_form()?
                }
            }
            OrgAddType::Company => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.companies.get(idx) {
                        self.submit_company_edit(existing.id)?
                    } else { false }
                } else {
                    self.submit_company_form()?
                }
            }
            OrgAddType::Brand => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.brands.get(idx) {
                        self.submit_brand_edit(existing.id)?
                    } else { false }
                } else {
                    self.submit_brand_form()?
                }
            }
            OrgAddType::Project => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.projects.get(idx) {
                        self.submit_project_edit(existing.id)?
                    } else { false }
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

    fn submit_person_form(&mut self) -> io::Result<bool> {
        let first_name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let last_name = self.form_fields.get(1).map(|s| s.trim()).unwrap_or("");
        let email = self.form_fields.get(2).map(|s| s.trim()).unwrap_or("");
        let phone_str = self.form_fields.get(3).map(|s| s.trim()).unwrap_or("");
        let phone = if phone_str.is_empty() { None } else { Some(phone_str.to_string()) };

        if first_name.is_empty() || last_name.is_empty() || email.is_empty() {
            return Ok(false);
        }

        crate::app::org::add_person(&self.storage, first_name, last_name, email, phone)?;
        Ok(true)
    }

    fn submit_person_edit(&mut self, id: crate::org::PersonId) -> io::Result<bool> {
        let first_name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let last_name = self.form_fields.get(1).map(|s| s.trim()).unwrap_or("");
        let email = self.form_fields.get(2).map(|s| s.trim()).unwrap_or("");
        let phone_str = self.form_fields.get(3).map(|s| s.trim()).unwrap_or("");
        let phone = if phone_str.is_empty() { None } else { Some(phone_str.to_string()) };

        if first_name.is_empty() || last_name.is_empty() || email.is_empty() {
            return Ok(false);
        }

        crate::app::org::update_person(&self.storage, id, first_name, last_name, email, phone)?;
        Ok(true)
    }

    fn submit_company_form(&mut self) -> io::Result<bool> {
        use crate::org::{BrandId, CountryCode};

        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let country_str = self.form_fields.get(1).map(|s| s.trim()).unwrap_or("");
        let brand_str = self.form_fields.get(2).map(|s| s.trim()).unwrap_or("");

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

    fn submit_company_edit(&mut self, id: crate::org::CompanyId) -> io::Result<bool> {
        use crate::org::{BrandId, CountryCode};

        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let country_str = self.form_fields.get(1).map(|s| s.trim()).unwrap_or("");
        let brand_str = self.form_fields.get(2).map(|s| s.trim()).unwrap_or("");

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

    fn submit_brand_form(&mut self) -> io::Result<bool> {
        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");

        // Validation
        if name.is_empty() {
            return Ok(false);
        }

        crate::app::org::add_brand(&self.storage, name)?;
        Ok(true)
    }

    fn submit_brand_edit(&mut self, id: crate::org::BrandId) -> io::Result<bool> {
        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");

        if name.is_empty() {
            return Ok(false);
        }

        crate::app::org::update_brand(&self.storage, id, name)?;
        Ok(true)
    }

    fn parse_importance(&self) -> Option<u8> {
        let imp_str = self.form_fields.get(2).map(|s| s.trim()).unwrap_or("");
        if imp_str.is_empty() {
            return None;
        }
        match imp_str.parse::<u8>() {
            Ok(v) if (1..=5).contains(&v) => Some(v),
            _ => None,
        }
    }

    fn submit_project_form(&mut self) -> io::Result<bool> {
        use crate::org::CompanyId;

        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let company_id_str = self.form_fields.get(1).map(|s| s.trim()).unwrap_or("");
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

    fn submit_project_edit(&mut self, id: crate::org::ProjectId) -> io::Result<bool> {
        use crate::org::CompanyId;

        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let company_id_str = self.form_fields.get(1).map(|s| s.trim()).unwrap_or("");
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
        let mut visible = Vec::new();
        let filter = self.filter.to_lowercase();

        for group in &self.model.groups {
            let root_name = &group.root.name;
            
            // Apply filtering
            let root_matches = root_name.to_lowercase().contains(&filter);
            
            // Check if any subdomains match the filter
            let any_subdomain_matches = group.subdomains.iter().any(|s| {
                s.name.to_lowercase().contains(&filter)
            });

            if !root_matches && !any_subdomain_matches && !filter.is_empty() {
                continue;
            }

            // External redirects filter
            if self.hide_external_redirects && group.root.external_redirect().is_some() {
                continue;
            }

            visible.push(VisibleItem::Root(root_name.clone()));

            if self.expanded_roots.contains(root_name) {
                for sub in &group.subdomains {
                    if filter.is_empty() || sub.name.to_lowercase().contains(&filter) {
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

    pub fn toggle_expand(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if let Some(VisibleItem::Root(name)) = self.visible_items.get(selected) {
                if self.expanded_roots.contains(name) {
                    self.expanded_roots.remove(name);
                } else {
                    self.expanded_roots.insert(name.clone());
                }
                self.update_visible_items();
            }
        }
    }

    pub fn toggle_external_filter(&mut self) {
        self.hide_external_redirects = !self.hide_external_redirects;
        self.update_visible_items();
    }

    pub fn selected_asset(&self) -> Option<&AssetState> {
        let idx = self.table_state.selected()?;
        let item = self.visible_items.get(idx)?;
        
        let target_name = match item {
            VisibleItem::Root(name) => name,
            VisibleItem::Subdomain(_, name) => name,
        };

        for group in &self.model.groups {
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

    // -----------------------------------------------------------------------
    // Asset assignment to Project
    // -----------------------------------------------------------------------

    pub fn start_assign_asset(&mut self) {
        if self.selected_asset().is_some() {
            self.input_mode = InputMode::Assigning;
            self.input.clear();
            self.org_select_index = None;
            self.org_select_matches = Vec::new();
            self.update_select_matches();
        }
    }

    pub fn submit_asset_assignment(&mut self) -> io::Result<bool> {
        // Handle selection from list
        let project_id = if let Some(i) = self.org_select_index {
            if let Some((_display, id_str)) = self.org_select_matches.get(i) {
                match id_str.parse::<crate::org::ProjectId>() {
                    Ok(id) => Some(id),
                    Err(_) => return Ok(false),
                }
            } else {
                None
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
        let targets: Vec<String> = if !self.marked_assets.is_empty() {
            self.marked_assets.iter().cloned().collect()
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
        self.org_select_index = None;
        self.org_select_matches.clear();
        self.marked_assets.clear();
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
            let name = asset.name.clone();
            if self.marked_assets.contains(&name) {
                self.marked_assets.remove(&name);
            } else {
                self.marked_assets.insert(name);
            }
        }
    }

    pub fn clear_marks(&mut self) {
        self.marked_assets.clear();
    }

    // -----------------------------------------------------------------------
    // Role assignment
    // -----------------------------------------------------------------------

    pub fn start_role_assign(&mut self) {
        if let Some(idx) = self.projects_table_state.selected() {
            // Resolve actual project from flat rows
            let mut current_idx = 0;
            let mut selected_project = None;
            for project in &self.org_model.project_views {
                if current_idx == idx {
                    selected_project = Some(project);
                    break;
                }
                current_idx += 1;
                if self.expanded_projects.contains(&project.id.to_string()) {
                    if let Some(assets) = self.org_model.assets_by_project.get(&project.id) {
                        current_idx += assets.len();
                    } else {
                        current_idx += 1;
                    }
                }
            }

            if let Some(project) = selected_project {
                self.role_assign_project = Some(project.id);
                self.role_assign_type = Some(true); // Always Person
                self.role_assign_target = None;
                self.role_assign_role = None;
                self.input.clear();
                self.org_select_index = None;
                self.update_role_assign_matches();
                self.input_mode = InputMode::RoleAssigning(RoleAssignStep::SelectTarget);
            }
        }
    }

    pub fn update_role_assign_matches(&mut self) {
        let filter = self.input.to_lowercase();

        self.org_select_matches = self.org_model.persons.iter()
            .map(|p| (format!("{} {}", p.first_name, p.last_name), p.id.to_string()))
            .filter(|(name, _): &(String, String)| name.to_lowercase().contains(&filter))
            .collect();

        if self.org_select_matches.is_empty() {
            self.org_select_index = None;
        } else if let Some(i) = self.org_select_index {
            if i >= self.org_select_matches.len() {
                self.org_select_index = Some(0);
            }
        }
    }

    pub fn role_assign_pick_target(&mut self) -> bool {
        if let Some(i) = self.org_select_index {
            if let Some((_, id)) = self.org_select_matches.get(i) {
                self.role_assign_target = Some(id.clone());
                self.org_select_index = Some(0); // For role selection
                self.input_mode = InputMode::RoleAssigning(RoleAssignStep::SelectRole);
                return true;
            }
        }
        false
    }

    pub fn role_assign_submit(&mut self, role: crate::org::Role) -> io::Result<()> {
        let project_id = self.role_assign_project.ok_or_else(|| io::Error::other("No project selected"))?;
        let target_id_str = self.role_assign_target.as_ref().ok_or_else(|| io::Error::other("No target selected"))?;

        let person_id = target_id_str.parse::<crate::org::PersonId>()
            .map_err(|_| io::Error::other("Invalid person ID"))?;
        crate::app::org::assign_person_to_project(&self.storage, person_id, project_id, role)?;

        self.input_mode = InputMode::Normal;
        self.input.clear();
        self.refresh()?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Contractor assignment
    // -----------------------------------------------------------------------

    pub fn start_contractor_assign(&mut self) {
        if let Some(idx) = self.projects_table_state.selected() {
            let mut current_idx = 0;
            let mut selected_project = None;
            for project in &self.org_model.project_views {
                if current_idx == idx {
                    selected_project = Some(project);
                    break;
                }
                current_idx += 1;
                if self.expanded_projects.contains(&project.id.to_string()) {
                    if let Some(assets) = self.org_model.assets_by_project.get(&project.id) {
                        current_idx += assets.len();
                    } else {
                        current_idx += 1;
                    }
                }
            }

            if let Some(project) = selected_project {
                self.contractor_assign_project = Some(project.id);
                self.contractor_assign_company = None;
                self.contractor_assign_role = None;
                self.input.clear();
                self.org_select_index = None;
                self.update_contractor_assign_matches();
                self.input_mode = InputMode::ContractorAssigning(ContractorAssignStep::SelectCompany);
            }
        }
    }

    pub fn update_contractor_assign_matches(&mut self) {
        let filter = self.input.to_lowercase();

        self.org_select_matches = self.org_model.companies.iter()
            .filter(|c: &&crate::org::CompanyRecord| c.is_external())
            .map(|c| (c.name.clone(), c.id.to_string()))
            .filter(|(name, _): &(String, String)| name.to_lowercase().contains(&filter))
            .collect();

        if self.org_select_matches.is_empty() {
            self.org_select_index = None;
        } else if let Some(i) = self.org_select_index {
            if i >= self.org_select_matches.len() {
                self.org_select_index = Some(0);
            }
        }
    }

    pub fn contractor_assign_pick_company(&mut self) -> bool {
        if let Some(i) = self.org_select_index {
            if let Some((_, id)) = self.org_select_matches.get(i) {
                self.contractor_assign_company = Some(id.clone());
                self.org_select_index = Some(0);
                self.input_mode = InputMode::ContractorAssigning(ContractorAssignStep::SelectRole);
                return true;
            }
        }
        false
    }

    pub fn contractor_assign_submit(&mut self, role: crate::org::ContractorRole) -> io::Result<()> {
        let project_id = self.contractor_assign_project.ok_or_else(|| io::Error::other("No project selected"))?;
        let company_id_str = self.contractor_assign_company.as_ref().ok_or_else(|| io::Error::other("No company selected"))?;

        let company_id = company_id_str.parse::<crate::org::CompanyId>()
            .map_err(|_| io::Error::other("Invalid company ID"))?;
        crate::app::org::assign_contractor_to_project(&self.storage, company_id, project_id, role)?;

        self.input_mode = InputMode::Normal;
        self.input.clear();
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
        let ignore_patterns = self.ignore_patterns.clone();

        std::thread::spawn(move || {
            {
                let mut s = is_scanning.lock().unwrap();
                *s = true;
            }
            
            let res = crate::app::run_single_recon(&target, &storage, &ignore_patterns);
            
            {
                let mut s = is_scanning.lock().unwrap();
                *s = false;
            }

            if let Err(e) = res {
                eprintln!("Scan error: {}", e);
            }
        });
    }

    pub fn start_rescan(&mut self) {
        if let Some(asset) = self.selected_asset() {
            self.input = asset.name.clone();
            self.start_scan();
        }
    }
}

use crate::tui::model::AssetState;
