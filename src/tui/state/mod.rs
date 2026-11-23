//! State management for TUI tabs.
//!
//! This module provides independent state components for different TUI tabs,
//! each managing its own navigation, table state, and filtering.

pub mod drift;
pub mod findings;
pub mod forms;
pub mod inventory;
pub mod organization;
pub mod projects;

pub use drift::DriftState;
pub use findings::FindingsState;
pub use forms::FormState;
pub use inventory::{InventoryState, VisibleItem};
pub use organization::{OrgPane, OrgState};
pub use projects::ProjectsState;
