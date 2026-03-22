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
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: CompanyId,
    pub name: String,
    pub country: CountryCode,
    pub is_external: bool,
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
// Project — smallest unit of responsibility
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub brand: BrandId,
    pub company: CompanyId,
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
    fn test_company_with_country() {
        let company = Company {
            id: CompanyId::new(1),
            name: "Acme GmbH".to_string(),
            country: CountryCode::DEU,
            is_external: false,
        };
        assert_eq!(company.country.name(), "Germany");
        assert!(!company.is_external);
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
    fn test_project_links_brand_and_company() {
        let project = Project {
            id: ProjectId::new(1),
            name: "Website Relaunch".to_string(),
            brand: BrandId::new(100),
            company: CompanyId::new(200),
        };
        assert_eq!(project.brand, BrandId::new(100));
        assert_eq!(project.company, CompanyId::new(200));
    }

    #[test]
    fn test_role_display() {
        assert_eq!(Role::Owner.to_string(), "Owner");
        assert_eq!(Role::Manager.to_string(), "Manager");
        assert_eq!(Role::Technician.to_string(), "Technician");
    }
}
