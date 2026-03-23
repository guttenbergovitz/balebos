//! Domain model types for organizational entities.
//!
//! These types define the business domain model. Currently we use the
//! storage Record types directly, but these will be used when we build
//! more business logic layers.

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use isocountry::CountryCode;

// ---------------------------------------------------------------------------
// Typed IDs
// ---------------------------------------------------------------------------

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(u128);

        impl $name {
            pub fn new(value: u128) -> Self {
                Self(value)
            }

            pub fn inner(&self) -> u128 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = std::num::ParseIntError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.parse::<u128>().map(Self)
            }
        }
    };
}

typed_id!(PersonId);
typed_id!(CompanyId);
typed_id!(BrandId);
typed_id!(ProjectId);

// ---------------------------------------------------------------------------
// Person
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: PersonId,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
}

impl Person {
    pub fn display_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}

// ---------------------------------------------------------------------------
// Company
// ---------------------------------------------------------------------------

/// A legal entity (company).
///
/// - Internal companies belong to a Brand (`brand: Some(...)`)
/// - External contractors have no brand (`brand: None`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: CompanyId,
    pub name: String,
    pub country: CountryCode,
    /// The brand this company belongs to. None for external contractors.
    pub brand: Option<BrandId>,
}

impl Company {
    /// Returns true if this is an external contractor (no brand).
    pub fn is_external(&self) -> bool {
        self.brand.is_none()
    }
}

// ---------------------------------------------------------------------------
// Brand
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brand {
    pub id: BrandId,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Project — smallest unit of responsibility (cost center / MPK)
// ---------------------------------------------------------------------------

/// A project belongs to a Company (which belongs to a Brand).
///
/// Projects are cost centers / budget positions within a company.
/// External contractors can be assigned to projects via RoleAssignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub company: CompanyId,
    /// Importance rating from 1 (low) to 5 (critical). None if not rated.
    pub importance: Option<u8>,
}

// ---------------------------------------------------------------------------
// Asset — a host bound to exactly one project
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub host: String,
    pub project: ProjectId,
}

// ---------------------------------------------------------------------------
// ContractorRole — role for external contractor assignments
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractorRole {
    Development,
    DevOps,
    Design,
    Consultancy,
    Security,
}

impl ContractorRole {
    /// Returns all contractor role variants.
    pub fn all() -> &'static [ContractorRole] {
        &[
            ContractorRole::Development,
            ContractorRole::DevOps,
            ContractorRole::Design,
            ContractorRole::Consultancy,
            ContractorRole::Security,
        ]
    }
}

impl std::fmt::Display for ContractorRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractorRole::Development => write!(f, "Development"),
            ContractorRole::DevOps => write!(f, "DevOps"),
            ContractorRole::Design => write!(f, "Design"),
            ContractorRole::Consultancy => write!(f, "Consultancy"),
            ContractorRole::Security => write!(f, "Security"),
        }
    }
}

// ---------------------------------------------------------------------------
// Role & Assignment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Owner,
    Manager,
    Technician,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Owner => write!(f, "Owner"),
            Role::Manager => write!(f, "Manager"),
            Role::Technician => write!(f, "Technician"),
        }
    }
}

/// Assigns responsibility for a project to a person and/or company.
///
/// At least one of `person` or `company` should be `Some`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub project: ProjectId,
    pub role: Role,
    pub person: Option<PersonId>,
    pub company: Option<CompanyId>,
}

// ---------------------------------------------------------------------------
// Observation (reference shape — actual impl lives in probe_http)
// ---------------------------------------------------------------------------

/// Lightweight observation record linking a host to a point-in-time check.
/// Full HTTP/DNS observation details are in `app::probe_http::HttpObservation`
/// and `app::probe_dns::DnsObservation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub host: String,
    pub status: u16,
    pub title: Option<String>,
    pub observed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typed_ids_are_distinct_types() {
        let person = PersonId::new(1);
        let company = CompanyId::new(1);
        assert_eq!(person.to_string(), company.to_string());
        // Same inner value but different types — won't compile:
        // let _: PersonId = company;
    }

    #[test]
    fn test_typed_id_inner() {
        let id = PersonId::new(42);
        assert_eq!(id.inner(), 42);
    }

    #[test]
    fn test_typed_id_from_str() {
        let id: PersonId = "123".parse().unwrap();
        assert_eq!(id.inner(), 123);
        assert!("not_a_number".parse::<PersonId>().is_err());
    }

    #[test]
    fn test_person_display_name() {
        let p = Person {
            id: PersonId::new(1),
            first_name: "Jan".to_string(),
            last_name: "Kowalski".to_string(),
            email: "jan@example.com".to_string(),
            phone: None,
        };
        assert_eq!(p.display_name(), "Jan Kowalski");
    }

    #[test]
    fn test_country_code_from_alpha2() {
        let de = CountryCode::for_alpha2("DE").unwrap();
        assert_eq!(de, CountryCode::DEU);
        assert_eq!(de.name(), "Germany");

        let pl = CountryCode::for_alpha2("PL").unwrap();
        assert_eq!(pl, CountryCode::POL);

        let ua = CountryCode::for_alpha2("UA").unwrap();
        assert_eq!(ua, CountryCode::UKR);
    }

    #[test]
    fn test_country_code_invalid() {
        assert!(CountryCode::for_alpha2("XX").is_err());
    }

    #[test]
    fn test_company_with_brand() {
        let company = Company {
            id: CompanyId::new(1),
            name: "Bunga Deutschland".to_string(),
            country: CountryCode::DEU,
            brand: Some(BrandId::new(100)),
        };
        assert_eq!(company.country.name(), "Germany");
        assert!(!company.is_external());
    }

    #[test]
    fn test_external_company() {
        let company = Company {
            id: CompanyId::new(2),
            name: "External Agency".to_string(),
            country: CountryCode::POL,
            brand: None,
        };
        assert!(company.is_external());
    }

    #[test]
    fn test_asset_belongs_to_project() {
        let project_id = ProjectId::new(42);
        let asset = Asset {
            host: "api.example.com".to_string(),
            project: project_id,
        };
        assert_eq!(asset.project, project_id);
    }

    #[test]
    fn test_assignment_person_only() {
        let a = Assignment {
            project: ProjectId::new(1),
            role: Role::Owner,
            person: Some(PersonId::new(10)),
            company: None,
        };
        assert_eq!(a.role, Role::Owner);
        assert!(a.person.is_some());
        assert!(a.company.is_none());
    }

    #[test]
    fn test_assignment_company_only() {
        let a = Assignment {
            project: ProjectId::new(1),
            role: Role::Technician,
            person: None,
            company: Some(CompanyId::new(20)),
        };
        assert_eq!(a.role, Role::Technician);
        assert!(a.company.is_some());
    }

    #[test]
    fn test_project_belongs_to_company() {
        let project = Project {
            id: ProjectId::new(1),
            name: "Website Relaunch".to_string(),
            company: CompanyId::new(200),
            importance: None,
        };
        assert_eq!(project.company, CompanyId::new(200));
        assert_eq!(project.importance, None);
    }

    #[test]
    fn test_project_with_importance() {
        let project = Project {
            id: ProjectId::new(2),
            name: "Critical API".to_string(),
            company: CompanyId::new(100),
            importance: Some(5),
        };
        assert_eq!(project.importance, Some(5));
    }

    #[test]
    fn test_role_display() {
        assert_eq!(Role::Owner.to_string(), "Owner");
        assert_eq!(Role::Manager.to_string(), "Manager");
        assert_eq!(Role::Technician.to_string(), "Technician");
    }

    #[test]
    fn test_contractor_role_all() {
        let all = ContractorRole::all();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ContractorRole::Development);
        assert_eq!(all[1], ContractorRole::DevOps);
        assert_eq!(all[2], ContractorRole::Design);
        assert_eq!(all[3], ContractorRole::Consultancy);
        assert_eq!(all[4], ContractorRole::Security);
    }

    #[test]
    fn test_contractor_role_display() {
        assert_eq!(ContractorRole::Development.to_string(), "Development");
        assert_eq!(ContractorRole::DevOps.to_string(), "DevOps");
        assert_eq!(ContractorRole::Design.to_string(), "Design");
        assert_eq!(ContractorRole::Consultancy.to_string(), "Consultancy");
        assert_eq!(ContractorRole::Security.to_string(), "Security");
    }
}
