//! Typed domain events for discovery and reconnaissance runs.
//!
//! Independent of TUI or CLI presentation so both interfaces can share the
//! same execution telemetry without fabricating progress percentages.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Typed event emitted during a discovery or reconnaissance run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanEvent {
    RunStarted {
        run_id: String,
        target: String,
        started_at: DateTime<Utc>,
    },
    StageStarted {
        run_id: String,
        stage: String,
        /// `None` when total target count is not yet known (e.g. dynamic enumeration).
        total_targets: Option<usize>,
        at: DateTime<Utc>,
    },
    TargetStarted {
        run_id: String,
        stage: String,
        target: String,
        at: DateTime<Utc>,
    },
    TargetCompleted {
        run_id: String,
        stage: String,
        target: String,
        ok: bool,
        detail: Option<String>,
        at: DateTime<Utc>,
    },
    ObservationSaved {
        run_id: String,
        kind: String,
        target: String,
        at: DateTime<Utc>,
    },
    CandidateDiscovered {
        run_id: String,
        kind: String,
        entity: String,
        source: String,
        at: DateTime<Utc>,
    },
    FindingRaised {
        run_id: String,
        title: String,
        severity: String,
        at: DateTime<Utc>,
    },
    StageFailed {
        run_id: String,
        stage: String,
        error: String,
        at: DateTime<Utc>,
    },
    RunCompleted {
        run_id: String,
        targets_processed: usize,
        new_discoveries: usize,
        errors: usize,
        finished_at: DateTime<Utc>,
    },
    RunCancelled {
        run_id: String,
        finished_at: DateTime<Utc>,
    },
}
