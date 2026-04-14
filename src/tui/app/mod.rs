//! TUI application state and logic.

mod assignments;
mod findings;
mod inventory;
mod org_forms;
mod scanning;
mod tags;

use std::sync::{Arc, Mutex};

use crate::error::BalebosError;

use crate::storage::StorageEngine;
use crate::tui::model::{load_tui_model, OrgModel, TuiModel};
use crate::tui::state::{
    FindingsState, FormState, InventoryState, OrgState, ProjectsState,
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

/// Detailed scan state for TUI status bar
#[derive(Clone)]
pub struct ScanState {
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub steps: Vec<crate::app::batch::ReconStepResult>,
}

impl ScanState {
    pub fn new() -> Self {
        Self {
            message: String::new(),
            timestamp: chrono::Utc::now(),
            steps: Vec::new(),
        }
    }
}

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
    pub scan_status: Arc<Mutex<ScanState>>,
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
            scan_status: Arc::new(Mutex::new(ScanState::new())),
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
}
