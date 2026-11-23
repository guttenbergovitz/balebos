//! Scanning methods for TUI.

use crate::tui::app::{App, InputMode};

impl App {
    pub fn start_scan(&mut self) {
        if self.input.is_empty() {
            return;
        }

        let target = self.input.clone();
        self.input.clear();
        self.input_mode = InputMode::Normal;

        // A previous cancellation must not poison the next scan.
        crate::app::probe::process::reset_cancel();

        // Reset scan status before starting a new scan
        {
            let mut status = self.scan_status.lock().unwrap_or_else(|p| p.into_inner());
            status.message = "Starting scan...".to_string();
            status.steps.clear();
            status.timestamp = chrono::Utc::now();
        }

        let storage = self.storage.clone();
        let is_scanning = self.is_scanning.clone();
        let scan_status = self.scan_status.clone();
        let ignore_patterns = self.ignore_patterns.clone();

        std::thread::spawn(move || {
            {
                let mut s = is_scanning.lock().unwrap_or_else(|p| p.into_inner());
                *s = true;
            }

            let scan_status_for_step = scan_status.clone();
            let on_step = move |step: &str| {
                let mut status = scan_status_for_step
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                status.message = step.to_string();
                status.timestamp = chrono::Utc::now();
            };

            let scan_status_for_result = scan_status.clone();
            let on_result = move |result: &crate::app::batch::ReconStepResult| {
                let mut status = scan_status_for_result
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                status.steps.push(result.clone());
                status.timestamp = chrono::Utc::now();
            };

            let res = crate::app::batch::run_single_recon_with_progress(
                &target,
                &storage,
                &ignore_patterns,
                &on_step,
                &on_result,
            );

            {
                let mut s = is_scanning.lock().unwrap_or_else(|p| p.into_inner());
                *s = false;
            }

            let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
            let failures: Vec<_> = status.steps.iter().filter(|s| !s.success).collect();
            if let Err(e) = res {
                status.message = format!("❌ Scan error: {}", e);
            } else if !failures.is_empty() {
                let first = &failures[0];
                status.message = format!("❌ {} failed: {}", first.name, first.message);
            } else {
                status.message = "✓ Scan completed".to_string();
            }
            status.timestamp = chrono::Utc::now();
        });
    }

    pub fn start_rescan(&mut self) {
        if let Some(asset) = self.selected_asset() {
            self.input = asset.name.clone();
            self.start_scan();
        }
    }
}
