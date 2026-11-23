//! Scanning methods for TUI.

use crate::tui::app::{App, InputMode};

impl App {
    pub fn start_scan(&mut self) {
        if self.input.is_empty() {
            return;
        }

        // Atomic guard against launching concurrent scans
        let already_running = {
            let mut scanning = self.is_scanning.lock().unwrap_or_else(|p| p.into_inner());
            if *scanning {
                true
            } else {
                *scanning = true;
                false
            }
        };
        if already_running {
            self.set_status("A scan is already in progress".to_string());
            return;
        }

        let target = self.input.clone();
        self.input.clear();
        self.input_mode = InputMode::Normal;

        // A previous cancellation must not poison the next scan.
        crate::app::probe::process::reset_cancel();

        let run_id = format!("run-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));

        // Reset scan status before starting a new scan
        {
            let mut status = self.scan_status.lock().unwrap_or_else(|p| p.into_inner());
            status.message = "Starting scan...".to_string();
            status.steps.clear();
            status.apply_event(crate::domain::scan_event::ScanEvent::RunStarted {
                run_id: run_id.clone(),
                target: target.clone(),
                started_at: chrono::Utc::now(),
            });
        }

        let storage = self.storage.clone();
        let is_scanning = self.is_scanning.clone();
        let scan_status = self.scan_status.clone();
        let ignore_patterns = self.ignore_patterns.clone();

        std::thread::spawn(move || {
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

            let scan_status_for_event = scan_status.clone();
            let on_event = move |ev: crate::domain::scan_event::ScanEvent| {
                let mut status = scan_status_for_event
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                status.apply_event(ev);
            };

            let res = crate::app::batch::run_single_recon_with_events(
                &run_id,
                &target,
                &storage,
                &ignore_patterns,
                &on_step,
                &on_result,
                &on_event,
            );

            {
                let mut s = is_scanning.lock().unwrap_or_else(|p| p.into_inner());
                *s = false;
            }

            let cancelled = crate::app::probe::process::is_cancelled();
            let mut status = scan_status.lock().unwrap_or_else(|p| p.into_inner());
            let failures: Vec<_> = status.steps.iter().filter(|s| !s.success).collect();
            if cancelled {
                status.message = "⚠ Scan cancelled".to_string();
                status.apply_event(crate::domain::scan_event::ScanEvent::RunCancelled {
                    run_id,
                    finished_at: chrono::Utc::now(),
                });
                return;
            }
            let mut err_count = failures.len().max(status.error_count);
            if let Err(e) = res {
                err_count = err_count.max(1);
                status.message = format!("❌ Scan error: {}", e);
                status.apply_event(crate::domain::scan_event::ScanEvent::StageFailed {
                    run_id: run_id.clone(),
                    stage: "recon".to_string(),
                    error: e.to_string(),
                    at: chrono::Utc::now(),
                });
            } else if !failures.is_empty() {
                let first = &failures[0];
                status.message = format!("❌ {} failed: {}", first.name, first.message);
            } else {
                status.message = "✓ Scan completed".to_string();
            }
            let processed = status.processed_targets;
            let discoveries = status.new_discoveries;
            status.apply_event(crate::domain::scan_event::ScanEvent::RunCompleted {
                run_id,
                targets_processed: processed,
                new_discoveries: discoveries,
                errors: err_count,
                finished_at: chrono::Utc::now(),
            });
        });
    }

    pub fn start_rescan(&mut self) {
        if let Some(asset) = self.selected_asset() {
            self.input = asset.name.clone();
            self.start_scan();
        }
    }
}
