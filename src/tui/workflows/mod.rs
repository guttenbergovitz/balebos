//! Workflow state management for complex multi-step operations.
//!
//! This module provides independent state components for different workflows,
//! each managing its own navigation and data collection across multiple steps.

pub mod contractor_assignment;
pub mod role_assignment;

pub use contractor_assignment::ContractorAssignStep;
pub use contractor_assignment::ContractorAssignWorkflow;
pub use role_assignment::RoleAssignStep;
pub use role_assignment::RoleAssignWorkflow;
