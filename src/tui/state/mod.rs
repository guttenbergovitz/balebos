//! State management for TUI tabs.
//!
//! This module provides independent state components for different TUI tabs,
//! each managing its own navigation, table state, and filtering.

pub mod inventory;
pub mod organization;

pub use inventory::{InventoryState, VisibleItem};
pub use organization::{OrgPane, OrgState};
