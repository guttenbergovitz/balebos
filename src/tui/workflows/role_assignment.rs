//! Role assignment workflow state management.

use crate::org::{PersonId, ProjectId, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleAssignStep {
    SelectTarget, // Person
    SelectRole,
}

/// State for role assignment workflow
#[derive(Debug)]
pub struct RoleAssignWorkflow {
    /// Project ID being assigned to
    pub project: Option<ProjectId>,

    /// Whether we're assigning a person (true) or company (false)
    pub assign_type: Option<bool>,

    /// Selected person or company ID (string representation)
    pub target: Option<String>,

    /// Selected role for assignment
    pub role: Option<Role>,

    /// Current step in the workflow
    pub step: RoleAssignStep,
}

impl RoleAssignWorkflow {
    /// Create a new role assignment workflow
    pub fn new() -> Self {
        Self {
            project: None,
            assign_type: None,
            target: None,
            role: None,
            step: RoleAssignStep::SelectTarget,
        }
    }

    /// Start workflow for a project
    pub fn start(&mut self, project: ProjectId) {
        self.project = Some(project);
        self.assign_type = Some(true); // Always Person
        self.target = None;
        self.role = None;
        self.step = RoleAssignStep::SelectTarget;
    }

    /// Move to next step
    pub fn next_step(&mut self) -> bool {
        match self.step {
            RoleAssignStep::SelectTarget if self.target.is_some() => {
                self.step = RoleAssignStep::SelectRole;
                true
            }
            _ => false,
        }
    }

    /// Move to previous step
    pub fn prev_step(&mut self) -> bool {
        match self.step {
            RoleAssignStep::SelectRole => {
                self.step = RoleAssignStep::SelectTarget;
                true
            }
            _ => false,
        }
    }

    /// Check if workflow is complete and ready to submit
    pub fn is_complete(&self) -> bool {
        self.project.is_some() && self.target.is_some() && self.role.is_some()
    }

    /// Reset workflow to initial state
    pub fn reset(&mut self) {
        self.project = None;
        self.assign_type = None;
        self.target = None;
        self.role = None;
        self.step = RoleAssignStep::SelectTarget;
    }
}

impl Default for RoleAssignWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let workflow = RoleAssignWorkflow::new();
        assert!(workflow.project.is_none());
        assert_eq!(workflow.step, RoleAssignStep::SelectTarget);
    }

    #[test]
    fn test_start() {
        let mut workflow = RoleAssignWorkflow::new();
        let proj_id = ProjectId(12345);
        workflow.start(proj_id);
        assert_eq!(workflow.project, Some(proj_id));
        assert_eq!(workflow.assign_type, Some(true));
    }

    #[test]
    fn test_next_step() {
        let mut workflow = RoleAssignWorkflow::new();
        assert!(!workflow.next_step()); // Can't advance without target

        workflow.target = Some("person_123".to_string());
        assert!(workflow.next_step());
        assert_eq!(workflow.step, RoleAssignStep::SelectRole);
    }

    #[test]
    fn test_prev_step() {
        let mut workflow = RoleAssignWorkflow::new();
        workflow.step = RoleAssignStep::SelectRole;
        assert!(workflow.prev_step());
        assert_eq!(workflow.step, RoleAssignStep::SelectTarget);
    }

    #[test]
    fn test_is_complete() {
        let mut workflow = RoleAssignWorkflow::new();
        assert!(!workflow.is_complete());

        workflow.project = Some(ProjectId(123));
        assert!(!workflow.is_complete());

        workflow.target = Some("person_id".to_string());
        assert!(!workflow.is_complete());

        workflow.role = Some(Role::Developer);
        assert!(workflow.is_complete());
    }

    #[test]
    fn test_reset() {
        let mut workflow = RoleAssignWorkflow::new();
        workflow.project = Some(ProjectId(123));
        workflow.target = Some("person".to_string());
        workflow.step = RoleAssignStep::SelectRole;

        workflow.reset();
        assert!(workflow.project.is_none());
        assert!(workflow.target.is_none());
        assert_eq!(workflow.step, RoleAssignStep::SelectTarget);
    }
}
