//! Drift methods for the TUI. Loads findings through the shared drift
//! service so the TUI and CLI agree by construction (ADR 0006).

use crate::error::BalebosError;
use crate::tui::app::App;

impl App {
    pub fn load_drift(&mut self) -> Result<(), BalebosError> {
        let records = self.storage.read_all_records()?;
        let window = self.drift.window_hours;
        let (findings, complete) = crate::app::drift::drift_findings(&records, window);

        self.drift.findings = findings;
        self.drift.coverage_complete = complete;
        self.drift.loaded = true;

        if !self.drift.findings.is_empty() {
            self.drift.table_state.select(Some(0));
        } else {
            self.drift.table_state.select(None);
        }

        Ok(())
    }

    pub fn drift_next(&mut self) {
        self.drift.next();
    }

    pub fn drift_previous(&mut self) {
        self.drift.previous();
    }
}
