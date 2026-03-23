//! Tab-specific rendering modules.
//!
//! Each tab in the TUI has its own module for rendering its content.

pub mod inventory;
pub mod organization;
pub mod projects;

pub use inventory::{build_details_text, draw_inventory};
pub use organization::draw_organization;
pub use projects::draw_projects;
