//! Business logic for organizational entity management.

use crate::error::Result;

use crate::org::{
    AssetAssignmentRecord, BrandId, BrandRecord, CompanyId, CompanyRecord, ContractorRole,
    CountryCode, ExternalContractorAssignment, PersonId, PersonRecord, ProjectId, ProjectRecord,
    Role, RoleAssignmentRecord,
};
use crate::storage::{StorageEngine, StorageEngineExt};

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a timestamp-based unique ID.
fn generate_id() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos()
}

// ---------------------------------------------------------------------------
// Person operations
// ---------------------------------------------------------------------------

pub fn add_person(
    storage: &Arc<dyn StorageEngine>,
    first_name: &str,
    last_name: &str,
    email: &str,
    phone: Option<String>,
) -> Result<PersonRecord> {
    let id = PersonId::new(generate_id());
    let record = PersonRecord::new(
        id,
        first_name.to_string(),
        last_name.to_string(),
        email.to_string(),
        phone,
    );
    storage.save(&record)?;
    Ok(record)
}

pub fn update_person(
    storage: &Arc<dyn StorageEngine>,
    id: PersonId,
    first_name: &str,
    last_name: &str,
    email: &str,
    phone: Option<String>,
) -> Result<PersonRecord> {
    let record = PersonRecord::new(
        id,
        first_name.to_string(),
        last_name.to_string(),
        email.to_string(),
        phone,
    );
    storage.save(&record)?;
    Ok(record)
}

pub fn list_persons(storage: &Arc<dyn StorageEngine>) -> Result<Vec<PersonRecord>> {
    let records = storage.read_all_records()?;
    Ok(records.persons)
}

// ---------------------------------------------------------------------------
// Company operations
// ---------------------------------------------------------------------------

pub fn add_company(
    storage: &Arc<dyn StorageEngine>,
    name: &str,
    country: CountryCode,
    brand: Option<BrandId>,
) -> Result<CompanyRecord> {
    let id = CompanyId::new(generate_id());
    let record = CompanyRecord::new(id, name.to_string(), country, brand);
    storage.save(&record)?;
    Ok(record)
}

pub fn update_company(
    storage: &Arc<dyn StorageEngine>,
    id: CompanyId,
    name: &str,
    country: CountryCode,
    brand: Option<BrandId>,
) -> Result<CompanyRecord> {
    let record = CompanyRecord::new(id, name.to_string(), country, brand);
    storage.save(&record)?;
    Ok(record)
}

pub fn list_companies(storage: &Arc<dyn StorageEngine>) -> Result<Vec<CompanyRecord>> {
    let records = storage.read_all_records()?;
    Ok(records.companies)
}

// ---------------------------------------------------------------------------
// Brand operations
// ---------------------------------------------------------------------------

pub fn add_brand(storage: &Arc<dyn StorageEngine>, name: &str) -> Result<BrandRecord> {
    let id = BrandId::new(generate_id());
    let record = BrandRecord::new(id, name.to_string());
    storage.save(&record)?;
    Ok(record)
}

pub fn update_brand(
    storage: &Arc<dyn StorageEngine>,
    id: BrandId,
    name: &str,
) -> Result<BrandRecord> {
    let record = BrandRecord::new(id, name.to_string());
    storage.save(&record)?;
    Ok(record)
}

pub fn list_brands(storage: &Arc<dyn StorageEngine>) -> Result<Vec<BrandRecord>> {
    let records = storage.read_all_records()?;
    Ok(records.brands)
}

// ---------------------------------------------------------------------------
// Project operations
// ---------------------------------------------------------------------------

pub fn add_project(
    storage: &Arc<dyn StorageEngine>,
    name: &str,
    company_id: CompanyId,
) -> Result<ProjectRecord> {
    let id = ProjectId::new(generate_id());
    let record = ProjectRecord::new(id, name.to_string(), company_id);
    storage.save(&record)?;
    Ok(record)
}

pub fn add_project_with_importance(
    storage: &Arc<dyn StorageEngine>,
    name: &str,
    company_id: CompanyId,
    importance: Option<u8>,
) -> Result<ProjectRecord> {
    let id = ProjectId::new(generate_id());
    let record = ProjectRecord::new(id, name.to_string(), company_id).with_importance(importance);
    storage.save(&record)?;
    Ok(record)
}

pub fn set_project_importance(
    storage: &Arc<dyn StorageEngine>,
    id: ProjectId,
    name: &str,
    company_id: CompanyId,
    importance: Option<u8>,
) -> Result<ProjectRecord> {
    let record = ProjectRecord::new(id, name.to_string(), company_id).with_importance(importance);
    storage.save(&record)?;
    Ok(record)
}

pub fn list_projects(storage: &Arc<dyn StorageEngine>) -> Result<Vec<ProjectRecord>> {
    let records = storage.read_all_records()?;
    Ok(records.projects)
}

// ---------------------------------------------------------------------------
// Asset assignment operations
// ---------------------------------------------------------------------------

pub fn assign_asset(
    storage: &Arc<dyn StorageEngine>,
    host: &str,
    project_id: ProjectId,
) -> Result<AssetAssignmentRecord> {
    let record = AssetAssignmentRecord::new(host.to_string(), project_id);
    storage.save(&record)?;
    Ok(record)
}

pub fn unassign_asset(storage: &Arc<dyn StorageEngine>, host: &str) -> Result<bool> {
    let records = storage.read_all_records()?;
    let latest = records
        .asset_assignments
        .iter()
        .filter(|a| a.host == host)
        .max_by_key(|a| a.created_at);

    match latest {
        Some(a) if !a.unassigned => {
            let tombstone = AssetAssignmentRecord::unassigned(host.to_string());
            storage.save(&tombstone)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ---------------------------------------------------------------------------
// Role assignment operations
// ---------------------------------------------------------------------------

pub fn assign_person_to_project(
    storage: &Arc<dyn StorageEngine>,
    person_id: PersonId,
    project_id: ProjectId,
    role: Role,
) -> Result<RoleAssignmentRecord> {
    let record = RoleAssignmentRecord::new_person(project_id, person_id, role);
    storage.save(&record)?;
    Ok(record)
}

pub fn assign_company_to_project(
    storage: &Arc<dyn StorageEngine>,
    company_id: CompanyId,
    project_id: ProjectId,
    role: Role,
) -> Result<RoleAssignmentRecord> {
    let record = RoleAssignmentRecord::new_company(project_id, company_id, role);
    storage.save(&record)?;
    Ok(record)
}

// ---------------------------------------------------------------------------
// External contractor operations
// ---------------------------------------------------------------------------

pub fn assign_contractor_to_project(
    storage: &Arc<dyn StorageEngine>,
    company_id: CompanyId,
    project_id: ProjectId,
    role: ContractorRole,
) -> Result<ExternalContractorAssignment> {
    let record = ExternalContractorAssignment::new(project_id, company_id, role);
    storage.save(&record)?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_generate_id_is_unique() {
        let id1 = generate_id();
        // Small sleep to ensure time advances (nanosecond precision)
        thread::sleep(Duration::from_nanos(100));
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }
}
