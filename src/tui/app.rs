use ratatui::widgets::TableState;
use crate::storage::JsonlStorage;
use crate::tui::model::{load_tui_model, TuiModel, AssetState, OrgModel};
use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex};
use std::thread;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Main tabs in the TUI
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

    pub fn next(&self) -> Tab {
        match self {
            Tab::Inventory => Tab::Organization,
            Tab::Organization => Tab::Projects,
            Tab::Projects => Tab::Inventory,
        }
    }

    pub fn prev(&self) -> Tab {
        match self {
            Tab::Inventory => Tab::Projects,
            Tab::Organization => Tab::Inventory,
            Tab::Projects => Tab::Organization,
        }
    }
}

/// Sub-panes within Organization tab
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

    pub fn next(&self) -> OrgPane {
        match self {
            OrgPane::Persons => OrgPane::Companies,
            OrgPane::Companies => OrgPane::Brands,
            OrgPane::Brands => OrgPane::Persons,
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
            OrgAddType::Person => &["First Name", "Last Name", "Email"],
            OrgAddType::Company => &["Name", "Country Code", "Brand ID (empty=external)"],
            OrgAddType::Brand => &["Name"],
            OrgAddType::Project => &["Name", "Company ID"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
    Filtering,
    OrgAdding(OrgAddType),
    Assigning, // Assigning asset to project
}

impl Default for InputMode {
    fn default() -> Self {
        InputMode::Normal
    }
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

    // Scanning state
    pub is_scanning: Arc<Mutex<bool>>,
    pub scan_status: Arc<Mutex<String>>,
}

impl App {
    pub fn new(storage: JsonlStorage) -> Result<Self, io::Error> {
        let model = load_tui_model(&storage)?;
        let org_model = OrgModel::load(&storage)?;

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

            // Scanning state
            is_scanning: Arc::new(Mutex::new(false)),
            scan_status: Arc::new(Mutex::new(String::new())),
        };

        app.update_visible_items();
        app.update_org_selection();

        if !app.visible_items.is_empty() {
            app.table_state.select(Some(0));
        }

        Ok(app)
    }

    pub fn refresh(&mut self) -> Result<(), io::Error> {
        self.model = load_tui_model(&self.storage)?;
        self.org_model = OrgModel::load(&self.storage)?;
        self.update_visible_items();
        self.update_org_selection();
        self.fix_selection();
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Tab navigation
    // -----------------------------------------------------------------------

    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
        self.update_current_selection();
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
        self.update_current_selection();
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.active_tab = tab;
        self.update_current_selection();
    }

    fn update_current_selection(&mut self) {
        match self.active_tab {
            Tab::Inventory => {
                if self.table_state.selected().is_none() && !self.visible_items.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Tab::Organization => {
                self.update_org_selection();
            }
            Tab::Projects => {
                if self.projects_table_state.selected().is_none()
                    && !self.org_model.project_views.is_empty()
                {
                    self.projects_table_state.select(Some(0));
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Organization pane navigation
    // -----------------------------------------------------------------------

    pub fn next_org_pane(&mut self) {
        self.org_pane = self.org_pane.next();
        self.update_org_selection();
    }

    fn update_org_selection(&mut self) {
        let count = match self.org_pane {
            OrgPane::Persons => self.org_model.persons.len(),
            OrgPane::Companies => self.org_model.companies.len(),
            OrgPane::Brands => self.org_model.brands.len(),
        };
        if self.org_table_state.selected().is_none() && count > 0 {
            self.org_table_state.select(Some(0));
        } else if let Some(selected) = self.org_table_state.selected() {
            if selected >= count && count > 0 {
                self.org_table_state.select(Some(count - 1));
            } else if count == 0 {
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
        self.input_mode = InputMode::OrgAdding(add_type);
    }

    pub fn form_next_field(&mut self) {
        if let InputMode::OrgAdding(add_type) = &self.input_mode {
            let max = add_type.fields().len();
            if self.form_field_index < max - 1 {
                self.form_field_index += 1;
            }
        }
    }

    pub fn form_prev_field(&mut self) {
        if self.form_field_index > 0 {
            self.form_field_index -= 1;
        }
    }

    pub fn form_current_value(&self) -> &str {
        self.form_fields
            .get(self.form_field_index)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn form_push_char(&mut self, c: char) {
        if let Some(field) = self.form_fields.get_mut(self.form_field_index) {
            field.push(c);
        }
    }

    pub fn form_pop_char(&mut self) {
        if let Some(field) = self.form_fields.get_mut(self.form_field_index) {
            field.pop();
        }
    }

    pub fn cancel_form(&mut self) {
        self.input_mode = InputMode::Normal;
        self.form_fields.clear();
        self.form_field_index = 0;
    }

    /// Submit the org form and create the entity.
    /// Returns Ok(true) if successful, Ok(false) if validation failed.
    pub fn submit_org_form(&mut self) -> io::Result<bool> {
        let add_type = match &self.input_mode {
            InputMode::OrgAdding(t) => *t,
            _ => return Ok(false),
        };

        let result = match add_type {
            OrgAddType::Person => self.submit_person_form(),
            OrgAddType::Company => self.submit_company_form(),
            OrgAddType::Brand => self.submit_brand_form(),
            OrgAddType::Project => self.submit_project_form(),
        };

        match result {
            Ok(true) => {
                // Refresh data and close form
                self.refresh()?;
                self.cancel_form();
                Ok(true)
            }
            Ok(false) => Ok(false), // Validation failed, keep form open
            Err(e) => Err(e),
        }
    }

    fn submit_person_form(&mut self) -> io::Result<bool> {
        let first_name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let last_name = self.form_fields.get(1).map(|s| s.trim()).unwrap_or("");
        let email = self.form_fields.get(2).map(|s| s.trim()).unwrap_or("");

        // Validation
        if first_name.is_empty() || last_name.is_empty() || email.is_empty() {
            return Ok(false);
        }

        crate::app::org::add_person(&self.storage, first_name, last_name, email)?;
        Ok(true)
    }

    fn submit_company_form(&mut self) -> io::Result<bool> {
        use crate::org::{BrandId, CountryCode};

        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let country_str = self.form_fields.get(1).map(|s| s.trim().to_uppercase()).unwrap_or_default();
        let brand_str = self.form_fields.get(2).map(|s| s.trim()).unwrap_or("");

        // Validation
        if name.is_empty() || country_str.is_empty() {
            return Ok(false);
        }

        let country = match CountryCode::for_alpha2(&country_str) {
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

    fn submit_brand_form(&mut self) -> io::Result<bool> {
        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");

        // Validation
        if name.is_empty() {
            return Ok(false);
        }

        crate::app::org::add_brand(&self.storage, name)?;
        Ok(true)
    }

    fn submit_project_form(&mut self) -> io::Result<bool> {
        use crate::org::CompanyId;

        let name = self.form_fields.get(0).map(|s| s.trim()).unwrap_or("");
        let company_id_str = self.form_fields.get(1).map(|s| s.trim()).unwrap_or("");

        // Validation
        if name.is_empty() || company_id_str.is_empty() {
            return Ok(false);
        }

        let company_id: CompanyId = match company_id_str.parse() {
            Ok(id) => id,
            Err(_) => return Ok(false),
        };

        crate::app::org::add_project(&self.storage, name, company_id)?;
        Ok(true)
    }

    // -----------------------------------------------------------------------
    // Asset assignment
    // -----------------------------------------------------------------------

    /// Start assigning selected asset to a project.
    pub fn start_assign_asset(&mut self) {
        if self.selected_asset().is_some() {
            self.input_mode = InputMode::Assigning;
            self.input.clear();
        }
    }

    /// Submit asset assignment to a project.
    pub fn submit_asset_assignment(&mut self) -> io::Result<bool> {
        use crate::org::ProjectId;

        let asset = match self.selected_asset() {
            Some(a) => a.name.clone(),
            None => return Ok(false),
        };

        let project_id_str = self.input.trim();
        if project_id_str.is_empty() {
            return Ok(false);
        }

        let project_id: ProjectId = match project_id_str.parse() {
            Ok(id) => id,
            Err(_) => return Ok(false),
        };

        crate::app::org::assign_asset(&self.storage, &asset, project_id)?;
        self.refresh()?;
        self.input_mode = InputMode::Normal;
        self.input.clear();
        Ok(true)
    }

    pub fn update_visible_items(&mut self) {
        let mut items = Vec::new();
        let filter_lower = self.filter.to_lowercase();
        
        for group in &self.model.groups {
            if self.hide_external_redirects && group.root.external_redirect().is_some() {
                continue;
            }

            if filter_lower.is_empty() || group.root.name.to_lowercase().contains(&filter_lower) {
                items.push(VisibleItem::Root(group.root.name.clone()));
                if self.expanded_roots.contains(&group.root.name) {
                    for sub in &group.subdomains {
                        items.push(VisibleItem::Subdomain(group.root.name.clone(), sub.name.clone()));
                    }
                }
            }
        }
        self.visible_items = items;
    }

    pub fn toggle_external_filter(&mut self) {
        self.hide_external_redirects = !self.hide_external_redirects;
        self.update_visible_items();
        self.fix_selection();
    }

    pub fn toggle_expand(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if let Some(item) = self.visible_items.get(selected) {
                match item {
                    VisibleItem::Root(name) => {
                        if self.expanded_roots.contains(name) {
                            self.expanded_roots.remove(name);
                        } else {
                            self.expanded_roots.insert(name.clone());
                        }
                        self.update_visible_items();
                        self.fix_selection();
                    }
                    VisibleItem::Subdomain(_, _) => {}
                }
            }
        }
    }

    fn fix_selection(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if selected >= self.visible_items.len() {
                if self.visible_items.is_empty() {
                    self.table_state.select(None);
                } else {
                    self.table_state.select(Some(self.visible_items.len() - 1));
                }
            }
        } else if !self.visible_items.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    pub fn next(&mut self) {
        if self.visible_items.is_empty() { return; }
        let i = match self.table_state.selected() {
            Some(i) => if i >= self.visible_items.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.visible_items.is_empty() { return; }
        let i = match self.table_state.selected() {
            Some(i) => if i == 0 { self.visible_items.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn selected_asset(&self) -> Option<&AssetState> {
        let index = self.table_state.selected()?;
        let item = self.visible_items.get(index)?;
        match item {
            VisibleItem::Root(name) => self.model.groups.iter().find(|g| g.root.name == *name).map(|g| &g.root),
            VisibleItem::Subdomain(root_name, sub_name) => {
                self.model.groups.iter()
                    .find(|g| g.root.name == *root_name)?
                    .subdomains.iter()
                    .find(|s| s.name == *sub_name)
            }
        }
    }

    pub fn start_scan(&mut self) {
        let domain = self.input.drain(..).collect::<String>();
        if domain.is_empty() { return; }
        
        self.input_mode = InputMode::Normal;
        
        let storage = self.storage.clone();
        let is_scanning = Arc::clone(&self.is_scanning);
        let scan_status = Arc::clone(&self.scan_status);
        
        {
            let mut scanning = is_scanning.lock().unwrap();
            *scanning = true;
        }

        thread::spawn(move || {
            let set_status = |msg: &str| {
                *scan_status.lock().unwrap() = msg.to_string();
            };

            // Full discovery for new root domain
            set_status(&format!("Scanning {}...", domain));
            let _ = crate::app::scan::scan_domain(&domain, &storage);

            set_status(&format!("Discovering subdomains for {}...", domain));
            let _ = crate::app::probe::probe_subdomains(&domain, &storage);
            
            // Collect all unique targets found
            if let Ok(records) = crate::storage::read_records(&storage) {
                let mut targets = std::collections::HashSet::new();
                for item in records.items {
                    if item.name == domain || item.name.ends_with(&format!(".{}", domain)) {
                        targets.insert(item.name);
                    }
                }
                let target_list: Vec<String> = targets.into_iter().collect();
                let total = target_list.len();

                set_status(&format!("Resolving DNS for {} targets...", total));
                let _ = crate::app::probe_dns::probe_dns_batch(&target_list, &storage);

                set_status(&format!("Probing HTTP 0/{} ...", total));
                let status_ref = Arc::clone(&scan_status);
                let progress_cb: crate::app::probe_http::ProgressCallback = Arc::new(move |done, tot, name| {
                    *status_ref.lock().unwrap() = format!("HTTP {}/{} — {}", done, tot, name);
                });
                let _ = crate::app::probe_http::probe_http_batch_with_progress(&target_list, &storage, Some(progress_cb));
            }

            set_status("");
            let mut scanning = is_scanning.lock().unwrap();
            *scanning = false;
        });
    }

    pub fn start_rescan(&mut self) {
        if let Some(asset) = self.selected_asset() {
            let target = asset.name.clone();
            
            let storage = self.storage.clone();
            let is_scanning = Arc::clone(&self.is_scanning);
            let scan_status = Arc::clone(&self.scan_status);
            
            {
                let mut scanning = is_scanning.lock().unwrap();
                *scanning = true;
            }

            thread::spawn(move || {
                *scan_status.lock().unwrap() = format!("Resolving DNS for {}...", target);
                let _ = crate::app::probe_dns::probe_dns(&target, &storage);

                *scan_status.lock().unwrap() = format!("Probing HTTP for {}...", target);
                let _ = crate::app::probe_http::http_check(&target, &storage);

                *scan_status.lock().unwrap() = String::new();
                let mut scanning = is_scanning.lock().unwrap();
                *scanning = false;
            });
        }
    }
}
