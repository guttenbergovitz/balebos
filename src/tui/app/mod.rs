//! TUI application state and logic.

mod assignments;
mod drift;
mod findings;
mod inventory;
mod org_forms;
mod scanning;
mod tags;

use std::sync::{Arc, Mutex};

use crate::error::BalebosError;

use crate::storage::StorageEngine;
use crate::tui::model::{load_tui_model_from_records, OrgModel, TuiModel};
use crate::tui::state::{
    DriftState, FindingsState, FormState, InventoryState, OrgState, ProjectsState,
};
use crate::tui::workflows::{ContractorAssignWorkflow, RoleAssignWorkflow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Overview,
    Inventory,
    Drift,
    Findings,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Overview, Tab::Inventory, Tab::Drift, Tab::Findings]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Inventory => "Assets",
            Tab::Drift => "Changes",
            Tab::Findings => "Findings",
        }
    }
}

/// Coverage of the most recent scan report, or None when no scan has
/// been recorded yet.
fn compute_coverage_from_records(records: &crate::storage::reader::Records) -> Option<bool> {
    if records.scan_reports.is_empty() {
        return None;
    }
    Some(crate::app::snapshot::latest_scan_is_complete(records))
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
    LifecycleEditing, // Editing asset lifecycle state
    ResetFilters,     // Confirmation modal for resetting all filters
    ExportData,       // Export data modal
    ImportData,       // Import data modal
}

/// Supported export formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
    Parquet,
}

impl ExportFormat {
    pub fn all() -> &'static [ExportFormat] {
        &[ExportFormat::Json, ExportFormat::Csv, ExportFormat::Parquet]
    }

    pub fn title(&self) -> &'static str {
        match self {
            ExportFormat::Json => "JSON",
            ExportFormat::Csv => "CSV",
            ExportFormat::Parquet => "Parquet",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Csv => "csv",
            ExportFormat::Parquet => "parquet",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "json" => Some(ExportFormat::Json),
            "csv" => Some(ExportFormat::Csv),
            "parquet" => Some(ExportFormat::Parquet),
            _ => None,
        }
    }
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

impl Default for ScanState {
    fn default() -> Self {
        Self {
            message: String::new(),
            timestamp: chrono::Utc::now(),
            steps: Vec::new(),
        }
    }
}

impl ScanState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Visible items in Inventory table (tree structure)
pub struct App {
    pub model: TuiModel,
    pub org_model: OrgModel,
    /// Latest scan coverage: None = no scan report yet, Some(false) =
    /// incomplete (absences are not verified change).
    pub coverage_complete: Option<bool>,
    pub storage: Arc<dyn StorageEngine>,
    pub should_quit: bool,

    // Tab navigation
    pub active_tab: Tab,

    // Composed state components
    pub inventory: InventoryState,
    pub organization: OrgState,
    pub projects: ProjectsState,
    pub findings: FindingsState,
    pub drift: DriftState,

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

    // Export/import state
    pub export_format: ExportFormat,
    pub export_path: String,
    pub import_path: String,
}

impl App {
    pub fn new(storage: Arc<dyn StorageEngine>) -> Result<Self, BalebosError> {
        let records = storage.read_all_records()?;
        let model = load_tui_model_from_records(&records);
        let org_model = OrgModel::load_from_records(&records);
        let ignore_patterns = crate::ignore::load_ignore_patterns("balebos.ignore");
        let coverage = compute_coverage_from_records(&records);

        let mut app = Self {
            model,
            org_model,
            coverage_complete: coverage,
            storage,
            should_quit: false,

            // Tab navigation
            active_tab: Tab::default(),

            // State components
            inventory: InventoryState::new(),
            organization: OrgState::new(),
            projects: ProjectsState::new(),
            findings: FindingsState::new(),
            drift: DriftState::default(),

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

            // Export/import state
            export_format: ExportFormat::Json,
            export_path: String::new(),
            import_path: String::new(),
        };

        app.load_findings_from_records(&records);
        app.load_drift_from_records(&records);
        app.update_visible_items();
        app.update_org_selection();

        if !app.inventory.visible_items.is_empty() {
            app.inventory.table_state.select(Some(0));
        }

        Ok(app)
    }

    pub fn refresh(&mut self) -> Result<(), BalebosError> {
        let records = self.storage.read_all_records()?;
        self.model = load_tui_model_from_records(&records);
        self.org_model = OrgModel::load_from_records(&records);
        self.coverage_complete = compute_coverage_from_records(&records);
        self.load_findings_from_records(&records);
        if self.drift.loaded {
            self.load_drift_from_records(&records);
        }
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
