//! Domain model types for organizational entities.

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
// Domain Registration — RDAP/Whois information
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainContact {
    pub name: Option<String>,
    pub organization: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainRegistration {
    pub domain: String,
    pub registrar: Option<String>,
    pub registered_at: Option<chrono::DateTime<chrono::Utc>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: Vec<String>,
    pub registrant: Option<DomainContact>,
    pub admin: Option<DomainContact>,
    pub tech: Option<DomainContact>,
    pub source: String, // rdap, whois
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
// Role
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
    fn test_typed_id_from_str() {
        let id: PersonId = "123".parse().unwrap();
        assert_eq!(id.to_string(), "123");
        assert!("not_a_number".parse::<PersonId>().is_err());
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
    fn test_role_display() {
        assert_eq!(Role::Owner.to_string(), "Owner");
        assert_eq!(Role::Manager.to_string(), "Manager");
        assert_eq!(Role::Technician.to_string(), "Technician");
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
