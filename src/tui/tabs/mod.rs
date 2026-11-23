//! Tab-specific rendering modules.
//!
//! Each tab in the TUI has its own module for rendering its content.

pub mod drift;
pub mod findings;
pub mod inventory;

pub use drift::draw_drift;
pub use findings::draw_findings;
pub use inventory::draw_inventory;
