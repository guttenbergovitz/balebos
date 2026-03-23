/// Organizational structure and ownership model.
///
/// Models responsibility and organizational context for infrastructure assets.
/// Project is the smallest unit of responsibility — assets belong to projects,
/// and people/companies are assigned to projects via roles.
///
/// Country codes use the `isocountry` crate (ISO 3166-1 alpha-3).
pub mod model;
pub mod storage;

pub use self::model::{
    BrandId, CompanyId, ContractorRole, CountryCode, DomainContact, DomainRegistration, PersonId,
    ProjectId, Role,
};
pub use self::storage::{
    AssetAssignmentRecord, AssetImportanceRecord, BrandRecord, CompanyRecord,
    DomainRegistrationRecord, ExternalContractorAssignment, PersonRecord, ProjectRecord,
    RoleAssignmentRecord,
};
