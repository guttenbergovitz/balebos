//! Tab-specific rendering modules.
//!
//! Each tab in the TUI has its own module for rendering its content.

pub mod findings;
pub mod inventory;
pub mod organization;
pub mod projects;

pub use findings::draw_findings;
pub use inventory::draw_inventory;
pub use organization::draw_organization;
pub use projects::draw_projects;
