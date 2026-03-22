//! Business logic for organizational entity management.

#![allow(dead_code)]

use crate::org::{
    AssetAssignmentRecord, BrandId, BrandRecord, CompanyId, CompanyRecord, CountryCode, PersonId,
    PersonRecord, ProjectId, ProjectRecord, Role, RoleAssignmentRecord,
};
use crate::storage::{read_records, JsonlStorage};
use std::io;
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
    storage: &JsonlStorage,
    first_name: &str,
    last_name: &str,
    email: &str,
) -> io::Result<PersonRecord> {
    let id = PersonId::new(generate_id());
    let record = PersonRecord::new(
        id,
        first_name.to_string(),
        last_name.to_string(),
        email.to_string(),
        None,
    );
    storage.save(&record)?;
    Ok(record)
}

pub fn list_persons(storage: &JsonlStorage) -> io::Result<Vec<PersonRecord>> {
    let records = read_records(storage)?;
    Ok(records.persons)
}

// ---------------------------------------------------------------------------
// Company operations
// ---------------------------------------------------------------------------

pub fn add_company(
    storage: &JsonlStorage,
    name: &str,
    country: CountryCode,
    brand: Option<BrandId>,
) -> io::Result<CompanyRecord> {
    let id = CompanyId::new(generate_id());
    let record = CompanyRecord::new(id, name.to_string(), country, brand);
    storage.save(&record)?;
    Ok(record)
}

pub fn list_companies(storage: &JsonlStorage) -> io::Result<Vec<CompanyRecord>> {
    let records = read_records(storage)?;
    Ok(records.companies)
}

// ---------------------------------------------------------------------------
// Brand operations
// ---------------------------------------------------------------------------

pub fn add_brand(storage: &JsonlStorage, name: &str) -> io::Result<BrandRecord> {
    let id = BrandId::new(generate_id());
    let record = BrandRecord::new(id, name.to_string());
    storage.save(&record)?;
    Ok(record)
}

pub fn list_brands(storage: &JsonlStorage) -> io::Result<Vec<BrandRecord>> {
    let records = read_records(storage)?;
    Ok(records.brands)
}

// ---------------------------------------------------------------------------
// Project operations
// ---------------------------------------------------------------------------

pub fn add_project(
    storage: &JsonlStorage,
    name: &str,
    company_id: CompanyId,
) -> io::Result<ProjectRecord> {
    let id = ProjectId::new(generate_id());
    let record = ProjectRecord::new(id, name.to_string(), company_id);
    storage.save(&record)?;
    Ok(record)
}

pub fn list_projects(storage: &JsonlStorage) -> io::Result<Vec<ProjectRecord>> {
    let records = read_records(storage)?;
    Ok(records.projects)
}

// ---------------------------------------------------------------------------
// Asset assignment operations
// ---------------------------------------------------------------------------

pub fn assign_asset(
    storage: &JsonlStorage,
    host: &str,
    project_id: ProjectId,
) -> io::Result<AssetAssignmentRecord> {
    let record = AssetAssignmentRecord::new(host.to_string(), project_id);
    storage.save(&record)?;
    Ok(record)
}

pub fn unassign_asset(storage: &JsonlStorage, host: &str) -> io::Result<bool> {
    let records = read_records(storage)?;
    // Check if the asset is assigned
    let is_assigned = records.asset_assignments.iter().any(|a| a.host == host);
    if !is_assigned {
        return Ok(false);
    }
    // For now, we don't actually remove from JSONL (append-only log).
    // In a real implementation, we would add a "tombstone" record or rewrite the file.
    // For now, we just return true to indicate the asset was found.
    // TODO: Implement proper unassignment with tombstone records
    Ok(true)
}

pub fn list_asset_assignments(storage: &JsonlStorage) -> io::Result<Vec<AssetAssignmentRecord>> {
    let records = read_records(storage)?;
    Ok(records.asset_assignments)
}

// ---------------------------------------------------------------------------
// Role assignment operations
// ---------------------------------------------------------------------------

pub fn assign_person_to_project(
    storage: &JsonlStorage,
    person_id: PersonId,
    project_id: ProjectId,
    role: Role,
) -> io::Result<RoleAssignmentRecord> {
    let record = RoleAssignmentRecord::new_person(project_id, person_id, role);
    storage.save(&record)?;
    Ok(record)
}

pub fn assign_company_to_project(
    storage: &JsonlStorage,
    company_id: CompanyId,
    project_id: ProjectId,
    role: Role,
) -> io::Result<RoleAssignmentRecord> {
    let record = RoleAssignmentRecord::new_company(project_id, company_id, role);
    storage.save(&record)?;
    Ok(record)
}

pub fn list_role_assignments(storage: &JsonlStorage) -> io::Result<Vec<RoleAssignmentRecord>> {
    let records = read_records(storage)?;
    Ok(records.role_assignments)
}

// ---------------------------------------------------------------------------
// Lookup helpers
// ---------------------------------------------------------------------------

pub fn find_person_by_id(storage: &JsonlStorage, id: PersonId) -> io::Result<Option<PersonRecord>> {
    let records = read_records(storage)?;
    Ok(records.persons.into_iter().find(|p| p.id == id))
}

pub fn find_company_by_id(
    storage: &JsonlStorage,
    id: CompanyId,
) -> io::Result<Option<CompanyRecord>> {
    let records = read_records(storage)?;
    Ok(records.companies.into_iter().find(|c| c.id == id))
}

pub fn find_brand_by_id(storage: &JsonlStorage, id: BrandId) -> io::Result<Option<BrandRecord>> {
    let records = read_records(storage)?;
    Ok(records.brands.into_iter().find(|b| b.id == id))
}

pub fn find_project_by_id(
    storage: &JsonlStorage,
    id: ProjectId,
) -> io::Result<Option<ProjectRecord>> {
    let records = read_records(storage)?;
    Ok(records.projects.into_iter().find(|p| p.id == id))
}

/// Find the project assignment for a given host.
pub fn find_asset_assignment(
    storage: &JsonlStorage,
    host: &str,
) -> io::Result<Option<AssetAssignmentRecord>> {
    let records = read_records(storage)?;
    // Return the most recent assignment for this host
    Ok(records
        .asset_assignments
        .into_iter()
        .filter(|a| a.host == host)
        .max_by_key(|a| a.created_at))
}

/// Find all role assignments for a given project.
pub fn find_role_assignments_for_project(
    storage: &JsonlStorage,
    project_id: ProjectId,
) -> io::Result<Vec<RoleAssignmentRecord>> {
    let records = read_records(storage)?;
    Ok(records
        .role_assignments
        .into_iter()
        .filter(|a| a.project == project_id)
        .collect())
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
