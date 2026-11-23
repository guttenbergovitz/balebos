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
            {
                let mut s = is_scanning.lock().unwrap_or_else(|p| p.into_inner());
                *s = true;
            }

            let scan_status_for_step = scan_status.clone();
            let run_id_for_step = run_id.clone();
            let on_step = move |step: &str| {
                let mut status = scan_status_for_step
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                status.apply_event(crate::domain::scan_event::ScanEvent::StageStarted {
                    run_id: run_id_for_step.clone(),
                    stage: step.to_string(),
                    total_targets: None,
                    at: chrono::Utc::now(),
                });
            };

            let scan_status_for_result = scan_status.clone();
            let run_id_for_result = run_id.clone();
            let target_for_result = target.clone();
            let on_result = move |result: &crate::app::batch::ReconStepResult| {
                let mut status = scan_status_for_result
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                status.steps.push(result.clone());
                status.apply_event(crate::domain::scan_event::ScanEvent::TargetCompleted {
                    run_id: run_id_for_result.clone(),
                    stage: result.name.to_string(),
                    target: target_for_result.clone(),
                    ok: result.success,
                    detail: Some(result.message.clone()),
                    at: chrono::Utc::now(),
                });
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
            let err_count = failures.len();
            if let Err(e) = res {
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
