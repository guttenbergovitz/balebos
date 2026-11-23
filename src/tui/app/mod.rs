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
    Runs,
    Inventory,
    Drift,
    Findings,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Overview,
            Tab::Runs,
            Tab::Inventory,
            Tab::Drift,
            Tab::Findings,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Runs => "Runs",
            Tab::Inventory => "Explore",
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
    Help,             // Contextual help modal (?)
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

/// Detailed scan state for TUI status bar and Runs view.
#[derive(Clone)]
pub struct ScanState {
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub steps: Vec<crate::app::batch::ReconStepResult>,
    pub run_id: Option<String>,
    pub target: Option<String>,
    pub current_stage: Option<String>,
    pub total_targets: Option<usize>,
    pub stage_processed: usize,
    pub processed_targets: usize,
    pub active_targets: Vec<String>,
    pub new_discoveries: usize,
    pub services_discovered: usize,
    pub error_count: usize,
    pub live_discoveries: Vec<String>,
    pub event_log: Vec<crate::domain::scan_event::ScanEvent>,
}

impl Default for ScanState {
    fn default() -> Self {
        Self {
            message: String::new(),
            timestamp: chrono::Utc::now(),
            steps: Vec::new(),
            run_id: None,
            target: None,
            current_stage: None,
            total_targets: None,
            stage_processed: 0,
            processed_targets: 0,
            active_targets: Vec::new(),
            new_discoveries: 0,
            services_discovered: 0,
            error_count: 0,
            live_discoveries: Vec::new(),
            event_log: Vec::new(),
        }
    }
}

impl ScanState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a typed [`crate::domain::scan_event::ScanEvent`] to the live run state.
    pub fn apply_event(&mut self, ev: crate::domain::scan_event::ScanEvent) {
        use crate::domain::scan_event::ScanEvent::*;
        match &ev {
            RunStarted {
                run_id,
                target,
                started_at,
            } => {
                self.run_id = Some(run_id.clone());
                self.target = Some(target.clone());
                self.current_stage = Some("starting".to_string());
                self.total_targets = None;
                self.stage_processed = 0;
                self.processed_targets = 0;
                self.active_targets.clear();
                self.new_discoveries = 0;
                self.services_discovered = 0;
                self.error_count = 0;
                self.live_discoveries.clear();
                self.timestamp = *started_at;
            }
            StageStarted {
                stage,
                total_targets,
                at,
                ..
            } => {
                self.current_stage = Some(stage.clone());
                self.total_targets = *total_targets;
                self.stage_processed = 0;
                self.message = stage.clone();
                self.timestamp = *at;
            }
            TargetStarted {
                stage, target, at, ..
            } => {
                self.current_stage = Some(stage.clone());
                if !self.active_targets.contains(target) {
                    self.active_targets.push(target.clone());
                    if self.active_targets.len() > 10 {
                        self.active_targets.remove(0);
                    }
                }
                self.message = format!("{}: {}", stage, target);
                self.timestamp = *at;
            }
            TargetCompleted {
                stage,
                target,
                ok,
                detail,
                at,
                ..
            } => {
                self.active_targets.retain(|t| t != target);
                self.processed_targets += 1;
                self.stage_processed += 1;
                if !*ok {
                    self.error_count += 1;
                }
                let summary = format!(
                    "{} [{}] {}{}",
                    if *ok { "✓" } else { "✗" },
                    stage,
                    target,
                    detail
                        .as_deref()
                        .map(|d| format!(" — {}", d))
                        .unwrap_or_default()
                );
                self.live_discoveries.push(summary);
                if self.live_discoveries.len() > 50 {
                    self.live_discoveries.remove(0);
                }
                self.timestamp = *at;
            }
            ObservationSaved {
                kind, target, at, ..
            } => {
                if kind == "service" {
                    self.services_discovered += 1;
                }
                self.live_discoveries
                    .push(format!("+ saved {} ({})", kind, target));
                if self.live_discoveries.len() > 50 {
                    self.live_discoveries.remove(0);
                }
                self.timestamp = *at;
            }
            CandidateDiscovered {
                kind,
                entity,
                source,
                at,
                ..
            } => {
                self.new_discoveries += 1;
                self.live_discoveries
                    .push(format!("★ {} {} (via {})", kind, entity, source));
                if self.live_discoveries.len() > 50 {
                    self.live_discoveries.remove(0);
                }
                self.timestamp = *at;
            }
            FindingRaised {
                title,
                severity,
                at,
                ..
            } => {
                self.live_discoveries
                    .push(format!("! [{}] {}", severity, title));
                self.timestamp = *at;
            }
            StageFailed {
                stage, error, at, ..
            } => {
                self.error_count += 1;
                self.live_discoveries
                    .push(format!("✗ stage {} failed: {}", stage, error));
                self.timestamp = *at;
            }
            RunCompleted {
                finished_at,
                errors,
                ..
            } => {
                self.current_stage = Some(if *errors == 0 {
                    "completed".to_string()
                } else {
                    "completed_with_errors".to_string()
                });
                self.timestamp = *finished_at;
            }
            RunCancelled { finished_at, .. } => {
                self.current_stage = Some("cancelled".to_string());
                self.timestamp = *finished_at;
            }
        }
        self.event_log.push(ev);
        if self.event_log.len() > 200 {
            self.event_log.remove(0);
        }
    }
}

/// Visible items in Inventory table (tree structure)
pub struct App {
    pub model: TuiModel,
    pub org_model: OrgModel,
    /// Latest scan coverage: None = no scan report yet, Some(false) =
    /// incomplete (absences are not verified change).
    pub coverage_complete: Option<bool>,
    pub relationships: Vec<crate::domain::relationship::ObservedRelationship>,
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
        let relationships = crate::app::relationships::project_relationships(&records);

        let mut app = Self {
            model,
            org_model,
            coverage_complete: coverage,
            relationships,
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
        self.relationships = crate::app::relationships::project_relationships(&records);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::scan_event::ScanEvent;

    #[test]
    fn scan_state_tracks_per_stage_progress_errors_and_cancellation() {
        let mut s = ScanState::new();
        let now = chrono::Utc::now();
        s.apply_event(ScanEvent::RunStarted {
            run_id: "r1".to_string(),
            target: "example.com".to_string(),
            started_at: now,
        });
        s.apply_event(ScanEvent::StageStarted {
            run_id: "r1".to_string(),
            stage: "dns".to_string(),
            total_targets: Some(2),
            at: now,
        });
        s.apply_event(ScanEvent::TargetCompleted {
            run_id: "r1".to_string(),
            stage: "dns".to_string(),
            target: "a.example.com".to_string(),
            ok: true,
            detail: None,
            at: now,
        });
        s.apply_event(ScanEvent::TargetCompleted {
            run_id: "r1".to_string(),
            stage: "dns".to_string(),
            target: "b.example.com".to_string(),
            ok: true,
            detail: None,
            at: now,
        });
        assert_eq!(s.stage_processed, 2);
        assert_eq!(s.processed_targets, 2);

        // Moving to HTTP stage resets `stage_processed` to 0 while keeping `processed_targets`.
        s.apply_event(ScanEvent::StageStarted {
            run_id: "r1".to_string(),
            stage: "http".to_string(),
            total_targets: Some(2),
            at: now,
        });
        assert_eq!(s.stage_processed, 0);
        assert_eq!(s.processed_targets, 2);

        s.apply_event(ScanEvent::StageFailed {
            run_id: "r1".to_string(),
            stage: "http".to_string(),
            error: "timeout".to_string(),
            at: now,
        });
        assert_eq!(s.error_count, 1);

        s.apply_event(ScanEvent::RunCompleted {
            run_id: "r1".to_string(),
            targets_processed: s.processed_targets,
            new_discoveries: 0,
            errors: 1,
            finished_at: now,
        });
        assert_eq!(s.current_stage.as_deref(), Some("completed_with_errors"));

        s.apply_event(ScanEvent::RunCancelled {
            run_id: "r1".to_string(),
            finished_at: now,
        });
        assert_eq!(s.current_stage.as_deref(), Some("cancelled"));
    }
}
