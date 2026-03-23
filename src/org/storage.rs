//! Storage records for organizational entities.
//!
//! These types are used for JSONL serialization/deserialization.

use chrono::{DateTime, Utc};
use isocountry::CountryCode;
use serde::{Deserialize, Serialize};

use super::{BrandId, CompanyId, ContractorRole, PersonId, ProjectId, Role};

// ---------------------------------------------------------------------------
// Person Record
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonRecord {
    pub item_type: String,
    pub id: PersonId,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl PersonRecord {
    pub fn new(
        id: PersonId,
        first_name: String,
        last_name: String,
        email: String,
        phone: Option<String>,
    ) -> Self {
        Self {
            item_type: "org_person".to_string(),
            id,
            first_name,
            last_name,
            email,
            phone,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Company Record
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyRecord {
    pub item_type: String,
    pub id: CompanyId,
    pub name: String,
    pub country: CountryCode,
    /// The brand this company belongs to. None for external contractors.
    pub brand: Option<BrandId>,
    pub created_at: DateTime<Utc>,
}

impl CompanyRecord {
    pub fn new(id: CompanyId, name: String, country: CountryCode, brand: Option<BrandId>) -> Self {
        Self {
            item_type: "org_company".to_string(),
            id,
            name,
            country,
            brand,
            created_at: Utc::now(),
        }
    }

    /// Returns true if this is an external contractor (no brand).
    pub fn is_external(&self) -> bool {
        self.brand.is_none()
    }
}

// ---------------------------------------------------------------------------
// Brand Record
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandRecord {
    pub item_type: String,
    pub id: BrandId,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl BrandRecord {
    pub fn new(id: BrandId, name: String) -> Self {
        Self {
            item_type: "org_brand".to_string(),
            id,
            name,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Project Record
// ---------------------------------------------------------------------------

/// A project (cost center / MPK) belongs to a company.
/// Brand is derived from the company's brand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub item_type: String,
    pub id: ProjectId,
    pub name: String,
    pub company: CompanyId,
    /// Importance rating from 1 (low) to 5 (critical). None if not rated.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub importance: Option<u8>,
    pub created_at: DateTime<Utc>,
}

impl ProjectRecord {
    pub fn new(id: ProjectId, name: String, company: CompanyId) -> Self {
        Self {
            item_type: "org_project".to_string(),
            id,
            name,
            company,
            importance: None,
            created_at: Utc::now(),
        }
    }

    /// Builder method to set the importance rating.
    pub fn with_importance(mut self, importance: Option<u8>) -> Self {
        self.importance = importance;
        self
    }
}

// ---------------------------------------------------------------------------
// Asset Assignment Record
// ---------------------------------------------------------------------------

/// Assigns a host (domain/subdomain) to a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAssignmentRecord {
    pub item_type: String,
    pub host: String,
    pub project: ProjectId,
    pub created_at: DateTime<Utc>,
}

impl AssetAssignmentRecord {
    pub fn new(host: String, project: ProjectId) -> Self {
        Self {
            item_type: "org_asset_assignment".to_string(),
            host,
            project,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Asset Importance Record
// ---------------------------------------------------------------------------

/// Sets the importance rating for a host (domain/subdomain).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetImportanceRecord {
    pub item_type: String,
    pub host: String,
    pub importance: u8,
    pub created_at: DateTime<Utc>,
}

impl AssetImportanceRecord {
    pub fn new(host: String, importance: u8) -> Self {
        Self {
            item_type: "org_asset_importance".to_string(),
            host,
            importance,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Role Assignment Record
// ---------------------------------------------------------------------------

/// Assigns a person or company to a project with a specific role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignmentRecord {
    pub item_type: String,
    pub project: ProjectId,
    pub role: Role,
    pub person: Option<PersonId>,
    pub company: Option<CompanyId>,
    pub created_at: DateTime<Utc>,
}

impl RoleAssignmentRecord {
    pub fn new_person(project: ProjectId, person: PersonId, role: Role) -> Self {
        Self {
            item_type: "org_role_assignment".to_string(),
            project,
            role,
            person: Some(person),
            company: None,
            created_at: Utc::now(),
        }
    }

    pub fn new_company(project: ProjectId, company: CompanyId, role: Role) -> Self {
        Self {
            item_type: "org_role_assignment".to_string(),
            project,
            role,
            person: None,
            company: Some(company),
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// External Contractor Assignment Record
// ---------------------------------------------------------------------------

/// Assigns an external company to a project with a contractor role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalContractorAssignment {
    pub item_type: String,
    pub project: ProjectId,
    pub company: CompanyId,
    pub role: ContractorRole,
    pub created_at: DateTime<Utc>,
}

impl ExternalContractorAssignment {
    pub fn new(project: ProjectId, company: CompanyId, role: ContractorRole) -> Self {
        Self {
            item_type: "org_contractor_assignment".to_string(),
            project,
            company,
            role,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person_record_serializes() {
        let rec = PersonRecord::new(
            PersonId::new(1),
            "Jan".to_string(),
            "Kowalski".to_string(),
            "jan@example.com".to_string(),
            None,
        );
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"item_type\":\"org_person\""));
        assert!(json.contains("\"first_name\":\"Jan\""));
    }

    #[test]
    fn test_company_record_with_brand_serializes() {
        let rec = CompanyRecord::new(
            CompanyId::new(1),
            "Bunga Deutschland".to_string(),
            CountryCode::DEU,
            Some(BrandId::new(100)),
        );
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"item_type\":\"org_company\""));
        assert!(json.contains("\"name\":\"Bunga Deutschland\""));
        assert!(json.contains("\"brand\":100"));
        assert!(!rec.is_external());
    }

    #[test]
    fn test_external_company_record_serializes() {
        let rec = CompanyRecord::new(
            CompanyId::new(2),
            "External Agency".to_string(),
            CountryCode::POL,
            None,
        );
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"brand\":null"));
        assert!(rec.is_external());
    }

    #[test]
    fn test_brand_record_serializes() {
        let rec = BrandRecord::new(BrandId::new(1), "SuperBrand".to_string());
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"item_type\":\"org_brand\""));
    }

    #[test]
    fn test_project_record_serializes() {
        let rec = ProjectRecord::new(
            ProjectId::new(1),
            "Website".to_string(),
            CompanyId::new(20),
        );
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"item_type\":\"org_project\""));
        assert!(json.contains("\"company\":20"));
    }

    #[test]
    fn test_asset_assignment_serializes() {
        let rec = AssetAssignmentRecord::new("example.com".to_string(), ProjectId::new(1));
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"item_type\":\"org_asset_assignment\""));
        assert!(json.contains("\"host\":\"example.com\""));
    }

    #[test]
    fn test_role_assignment_person_serializes() {
        let rec = RoleAssignmentRecord::new_person(ProjectId::new(1), PersonId::new(10), Role::Owner);
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"item_type\":\"org_role_assignment\""));
        assert!(json.contains("\"person\":10"));
        assert!(json.contains("\"company\":null"));
    }

    #[test]
    fn test_role_assignment_company_serializes() {
        let rec = RoleAssignmentRecord::new_company(ProjectId::new(1), CompanyId::new(20), Role::Technician);
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"item_type\":\"org_role_assignment\""));
        assert!(json.contains("\"company\":20"));
        assert!(json.contains("\"person\":null"));
    }

    #[test]
    fn test_project_record_importance_defaults_to_none() {
        let rec = ProjectRecord::new(
            ProjectId::new(1),
            "Website".to_string(),
            CompanyId::new(20),
        );
        assert_eq!(rec.importance, None);
        let json = serde_json::to_string(&rec).unwrap();
        assert!(!json.contains("importance"));
    }

    #[test]
    fn test_project_record_with_importance() {
        let rec = ProjectRecord::new(
            ProjectId::new(1),
            "Website".to_string(),
            CompanyId::new(20),
        )
        .with_importance(Some(3));
        assert_eq!(rec.importance, Some(3));
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"importance\":3"));
    }

    #[test]
    fn test_project_record_deserializes_without_importance() {
        let json = r#"{"item_type":"org_project","id":1,"name":"Test","company":20,"created_at":"2024-01-01T00:00:00Z"}"#;
        let rec: ProjectRecord = serde_json::from_str(json).unwrap();
        assert_eq!(rec.importance, None);
    }

    #[test]
    fn test_contractor_assignment_serializes() {
        let rec = ExternalContractorAssignment::new(
            ProjectId::new(1),
            CompanyId::new(30),
            ContractorRole::DevOps,
        );
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"item_type\":\"org_contractor_assignment\""));
        assert!(json.contains("\"project\":1"));
        assert!(json.contains("\"company\":30"));
        assert!(json.contains("\"role\":\"DevOps\""));
    }
}
