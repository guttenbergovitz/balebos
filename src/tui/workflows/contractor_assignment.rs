//! Contractor assignment workflow state management.

use crate::org::{CompanyId, ContractorRole, ProjectId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractorAssignStep {
    SelectCompany,
    SelectRole,
}

/// State for contractor assignment workflow
#[derive(Debug)]
pub struct ContractorAssignWorkflow {
    /// Project ID being assigned to
    pub project: Option<ProjectId>,

    /// Selected company ID (string representation)
    pub company: Option<String>,

    /// Selected role for assignment
    pub role: Option<ContractorRole>,

    /// Current step in the workflow
    pub step: ContractorAssignStep,
}

impl ContractorAssignWorkflow {
    /// Create a new contractor assignment workflow
    pub fn new() -> Self {
        Self {
            project: None,
            company: None,
            role: None,
            step: ContractorAssignStep::SelectCompany,
        }
    }

    /// Start workflow for a project
    pub fn start(&mut self, project: ProjectId) {
        self.project = Some(project);
        self.company = None;
        self.role = None;
        self.step = ContractorAssignStep::SelectCompany;
    }

    /// Move to next step
    pub fn next_step(&mut self) -> bool {
        match self.step {
            ContractorAssignStep::SelectCompany if self.company.is_some() => {
                self.step = ContractorAssignStep::SelectRole;
                true
            }
            _ => false,
        }
    }

    /// Move to previous step
    pub fn prev_step(&mut self) -> bool {
        match self.step {
            ContractorAssignStep::SelectRole => {
                self.step = ContractorAssignStep::SelectCompany;
                true
            }
            _ => false,
        }
    }

    /// Check if workflow is complete and ready to submit
    pub fn is_complete(&self) -> bool {
        self.project.is_some() && self.company.is_some() && self.role.is_some()
    }

    /// Get company ID from string
    pub fn company_id(&self) -> Option<CompanyId> {
        self.company
            .as_ref()
            .and_then(|s| s.parse::<CompanyId>().ok())
    }

    /// Reset workflow to initial state
    pub fn reset(&mut self) {
        self.project = None;
        self.company = None;
        self.role = None;
        self.step = ContractorAssignStep::SelectCompany;
    }
}

impl Default for ContractorAssignWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let workflow = ContractorAssignWorkflow::new();
        assert!(workflow.project.is_none());
        assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);
    }

    #[test]
    fn test_start() {
        let mut workflow = ContractorAssignWorkflow::new();
        let proj_id = ProjectId(12345);
        workflow.start(proj_id);
        assert_eq!(workflow.project, Some(proj_id));
        assert!(workflow.company.is_none());
    }

    #[test]
    fn test_next_step() {
        let mut workflow = ContractorAssignWorkflow::new();
        assert!(!workflow.next_step()); // Can't advance without company

        workflow.company = Some("company_123".to_string());
        assert!(workflow.next_step());
        assert_eq!(workflow.step, ContractorAssignStep::SelectRole);
    }

    #[test]
    fn test_prev_step() {
        let mut workflow = ContractorAssignWorkflow::new();
        workflow.step = ContractorAssignStep::SelectRole;
        assert!(workflow.prev_step());
        assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);
    }

    #[test]
    fn test_is_complete() {
        let mut workflow = ContractorAssignWorkflow::new();
        assert!(!workflow.is_complete());

        workflow.project = Some(ProjectId(123));
        assert!(!workflow.is_complete());

        workflow.company = Some("company_id".to_string());
        assert!(!workflow.is_complete());

        workflow.role = Some(ContractorRole::Developer);
        assert!(workflow.is_complete());
    }

    #[test]
    fn test_reset() {
        let mut workflow = ContractorAssignWorkflow::new();
        workflow.project = Some(ProjectId(123));
        workflow.company = Some("company".to_string());
        workflow.step = ContractorAssignStep::SelectRole;

        workflow.reset();
        assert!(workflow.project.is_none());
        assert!(workflow.company.is_none());
        assert_eq!(workflow.step, ContractorAssignStep::SelectCompany);
    }
}
