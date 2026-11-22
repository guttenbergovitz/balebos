//! Integration tests for TUI workflow state machines
//!
//! These tests verify that workflow transitions work correctly in various scenarios,
//! ensuring proper state management during user interactions.

use balebos::org::{ContractorRole, ProjectId, Role};
use balebos::tui::workflows::{
    ContractorAssignStep, ContractorAssignWorkflow, RoleAssignStep, RoleAssignWorkflow,
};

// ==================== Role Assignment Workflow Tests ====================

#[test]
fn role_assign_complete_workflow_sequence() {
    let mut workflow = RoleAssignWorkflow::new();

    // Initial state
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);
    assert!(!workflow.is_complete());

    // Start workflow
    let project_id = ProjectId::new(100);
    workflow.start(project_id);
    assert_eq!(workflow.project, Some(project_id));
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);

    // Try to advance without selecting target (should fail)
    assert!(!workflow.next_step());
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);

    // Select target
    workflow.target = Some("person_42".to_string());
    assert!(!workflow.is_complete()); // Still need role

    // Advance to role selection
    assert!(workflow.next_step());
    assert_eq!(workflow.step, RoleAssignStep::SelectRole);

    // Select role
    workflow.role = Some(Role::Manager);
    assert!(workflow.is_complete()); // Now complete

    // Reset workflow
    workflow.reset();
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);
    assert!(!workflow.is_complete());
}

#[test]
fn role_assign_back_forward_navigation() {
    let mut workflow = RoleAssignWorkflow::new();

    workflow.target = Some("person".to_string());
    workflow.next_step();
    assert_eq!(workflow.step, RoleAssignStep::SelectRole);

    // Go back
    assert!(workflow.prev_step());
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);

    // Go forward again
    assert!(workflow.next_step());
    assert_eq!(workflow.step, RoleAssignStep::SelectRole);
}

#[test]
fn role_assign_cannot_go_back_from_first_step() {
    let mut workflow = RoleAssignWorkflow::new();
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);

    // Can't go back from first step
    assert!(!workflow.prev_step());
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);
}

#[test]
fn role_assign_cannot_go_forward_without_target() {
    let mut workflow = RoleAssignWorkflow::new();
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);

    // Can't advance without target
    assert!(!workflow.next_step());
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);

    // Add target and try again
    workflow.target = Some("person_id".to_string());
    assert!(workflow.next_step());
}

#[test]
fn role_assign_multiple_workflow_instances_independent() {
    let mut workflow1 = RoleAssignWorkflow::new();
    let mut workflow2 = RoleAssignWorkflow::new();

    workflow1.start(ProjectId::new(1));
    workflow2.start(ProjectId::new(2));

    workflow1.target = Some("person_1".to_string());
    workflow1.next_step();

    // workflow2 should be unaffected
    assert_eq!(workflow2.step, RoleAssignStep::SelectTarget);
    assert_eq!(workflow1.step, RoleAssignStep::SelectRole);
}

// ==================== Contractor Assignment Workflow Tests ====================

#[test]
fn contractor_assign_complete_workflow_sequence() {
    let mut workflow = ContractorAssignWorkflow::new();

    // Initial state
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);
    assert!(!workflow.is_complete());

    // Start workflow
    let project_id = ProjectId::new(200);
    workflow.start(project_id);
    assert_eq!(workflow.project, Some(project_id));
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);

    // Try to advance without selecting company (should fail)
    assert!(!workflow.next_step());
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);

    // Select company
    workflow.company = Some("123".to_string()); // Valid company ID as string
    assert!(!workflow.is_complete()); // Still need role

    // Advance to role selection
    assert!(workflow.next_step());
    assert_eq!(workflow.step, ContractorAssignStep::SelectRole);

    // Select role
    workflow.role = Some(ContractorRole::Development);
    assert!(workflow.is_complete()); // Now complete

    // Reset workflow
    workflow.reset();
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);
    assert!(!workflow.is_complete());
}

#[test]
fn contractor_assign_back_forward_navigation() {
    let mut workflow = ContractorAssignWorkflow::new();

    workflow.company = Some("company_id".to_string());
    workflow.next_step();
    assert_eq!(workflow.step, ContractorAssignStep::SelectRole);

    // Go back
    assert!(workflow.prev_step());
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);

    // Go forward again
    assert!(workflow.next_step());
    assert_eq!(workflow.step, ContractorAssignStep::SelectRole);
}

#[test]
fn contractor_assign_cannot_go_back_from_first_step() {
    let mut workflow = ContractorAssignWorkflow::new();
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);

    // Can't go back from first step
    assert!(!workflow.prev_step());
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);
}

#[test]
fn contractor_assign_cannot_go_forward_without_company() {
    let mut workflow = ContractorAssignWorkflow::new();
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);

    // Can't advance without company
    assert!(!workflow.next_step());
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);

    // Add company and try again
    workflow.company = Some("456".to_string());
    assert!(workflow.next_step());
}

#[test]
fn contractor_assign_multiple_workflow_instances_independent() {
    let mut workflow1 = ContractorAssignWorkflow::new();
    let mut workflow2 = ContractorAssignWorkflow::new();

    workflow1.start(ProjectId::new(10));
    workflow2.start(ProjectId::new(20));

    workflow1.company = Some("company_1".to_string());
    workflow1.next_step();

    // workflow2 should be unaffected
    assert_eq!(workflow2.step, ContractorAssignStep::SelectCompany);
    assert_eq!(workflow1.step, ContractorAssignStep::SelectRole);
}

// ==================== State Transition Edge Cases ====================

#[test]
fn role_assign_cannot_go_forward_from_last_step() {
    let mut workflow = RoleAssignWorkflow::new();
    workflow.target = Some("person".to_string());
    workflow.next_step(); // Move to SelectRole

    // Already at last step, can't go forward
    assert!(!workflow.next_step());
    assert_eq!(workflow.step, RoleAssignStep::SelectRole);
}

#[test]
fn contractor_assign_cannot_go_forward_from_last_step() {
    let mut workflow = ContractorAssignWorkflow::new();
    workflow.company = Some("company".to_string());
    workflow.next_step(); // Move to SelectRole

    // Already at last step, can't go forward
    assert!(!workflow.next_step());
    assert_eq!(workflow.step, ContractorAssignStep::SelectRole);
}

#[test]
fn role_assign_partial_completion() {
    let mut workflow = RoleAssignWorkflow::new();

    // Only project set
    workflow.project = Some(ProjectId::new(1));
    assert!(!workflow.is_complete());

    // Only project and target set
    workflow.target = Some("person".to_string());
    assert!(!workflow.is_complete());

    // All three required fields set
    workflow.role = Some(Role::Owner);
    assert!(workflow.is_complete());
}

#[test]
fn contractor_assign_partial_completion() {
    let mut workflow = ContractorAssignWorkflow::new();

    // Only project set
    workflow.project = Some(ProjectId::new(1));
    assert!(!workflow.is_complete());

    // Only project and company set
    workflow.company = Some("company_id".to_string());
    assert!(!workflow.is_complete());

    // All three required fields set
    workflow.role = Some(ContractorRole::Security);
    assert!(workflow.is_complete());
}

// ==================== Reset and State Cleanup ====================

#[test]
fn role_assign_reset_clears_all_state() {
    let mut workflow = RoleAssignWorkflow::new();

    // Set all fields
    workflow.start(ProjectId::new(1));
    workflow.target = Some("person".to_string());
    workflow.role = Some(Role::Technician);
    workflow.step = RoleAssignStep::SelectRole;

    // Verify all are set
    assert!(workflow.project.is_some());
    assert!(workflow.target.is_some());
    assert!(workflow.role.is_some());

    // Reset
    workflow.reset();

    // Verify all are cleared
    assert!(workflow.project.is_none());
    assert!(workflow.assign_type.is_none());
    assert!(workflow.target.is_none());
    assert!(workflow.role.is_none());
    assert_eq!(workflow.step, RoleAssignStep::SelectTarget);
}

#[test]
fn contractor_assign_reset_clears_all_state() {
    let mut workflow = ContractorAssignWorkflow::new();

    // Set all fields
    workflow.start(ProjectId::new(1));
    workflow.company = Some("company".to_string());
    workflow.role = Some(ContractorRole::Consultancy);
    workflow.step = ContractorAssignStep::SelectRole;

    // Verify all are set
    assert!(workflow.project.is_some());
    assert!(workflow.company.is_some());
    assert!(workflow.role.is_some());

    // Reset
    workflow.reset();

    // Verify all are cleared
    assert!(workflow.project.is_none());
    assert!(workflow.company.is_none());
    assert!(workflow.role.is_none());
    assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);
}
