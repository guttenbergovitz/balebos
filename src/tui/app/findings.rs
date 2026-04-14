//! Findings methods for TUI.

use crate::error::BalebosError;
use crate::storage::StorageEngineExt;
use crate::tui::app::App;

impl App {
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
            {
                let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
                status.message = "⟳ Running checks...".to_string();
                status.timestamp = chrono::Utc::now();
                status.steps.clear();
            }

            match run_checks_and_persist(&storage, &ignore_patterns) {
                Ok(findings) => {
                    let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
                    if findings.is_empty() {
                        status.message = "✓ No new findings".to_string();
                    } else {
                        status.message = format!("✓ Found {} new findings", findings.len());
                    }
                    status.timestamp = chrono::Utc::now();
                }
                Err(e) => {
                    let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
                    status.message = format!("❌ Error: {}", e);
                    status.timestamp = chrono::Utc::now();
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
