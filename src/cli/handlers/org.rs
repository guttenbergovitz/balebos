use std::process;
use std::sync::Arc;

use crate::app::org::*;
use crate::cli::OrgCommand;
use crate::exit_codes::*;
use crate::org::{BrandId, CompanyId, CountryCode, PersonId, ProjectId, Role};
use crate::storage::StorageEngine;

#[allow(dead_code)]
pub(crate) fn cmd_org(org_cmd: OrgCommand, storage: &Arc<dyn StorageEngine>) {
    match org_cmd {
        OrgCommand::PersonAdd {
            first_name,
            last_name,
            email,
        } => match add_person(storage, &first_name, &last_name, &email, None) {
            Ok(rec) => {
                println!(
                    "Person added: {} {} <{}>",
                    rec.first_name, rec.last_name, rec.email
                );
                println!("ID: {}", rec.id);
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error adding person: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        OrgCommand::PersonList => match list_persons(storage) {
            Ok(persons) => {
                if persons.is_empty() {
                    println!("No persons found.");
                } else {
                    println!("Persons ({}):", persons.len());
                    for p in persons {
                        println!(
                            "  {} - {} {} <{}>",
                            p.id, p.first_name, p.last_name, p.email
                        );
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error listing persons: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        OrgCommand::CompanyAdd {
            name,
            country,
            brand_id,
        } => {
            let country_code = match CountryCode::for_alpha2(&country) {
                Ok(code) => code,
                Err(_) => {
                    eprintln!(
                        "Invalid country code: {}. Use ISO 3166-1 alpha-2 (e.g., DE, PL, US)",
                        country
                    );
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let brand = match brand_id {
                Some(id_str) => match id_str.parse::<BrandId>() {
                    Ok(id) => Some(id),
                    Err(_) => {
                        eprintln!("Invalid brand ID: {}", id_str);
                        process::exit(EXIT_INVALID_ARGUMENT);
                    }
                },
                None => None,
            };
            match add_company(storage, &name, country_code, brand) {
                Ok(rec) => {
                    println!("Company added: {} ({})", rec.name, rec.country.name());
                    println!("ID: {}", rec.id);
                    if rec.is_external() {
                        println!("Type: External contractor");
                    } else if let Some(brand_id) = rec.brand {
                        println!("Brand ID: {}", brand_id);
                    }
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error adding company: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::CompanyList => match list_companies(storage) {
            Ok(companies) => {
                if companies.is_empty() {
                    println!("No companies found.");
                } else {
                    println!("Companies ({}):", companies.len());
                    for c in companies {
                        let brand_info = if c.is_external() {
                            " [external]".to_string()
                        } else if let Some(brand_id) = c.brand {
                            format!(" [brand: {}]", brand_id)
                        } else {
                            String::new()
                        };
                        println!(
                            "  {} - {} ({}){}",
                            c.id,
                            c.name,
                            c.country.name(),
                            brand_info
                        );
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error listing companies: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        OrgCommand::BrandAdd { name } => match add_brand(storage, &name) {
            Ok(rec) => {
                println!("Brand added: {}", rec.name);
                println!("ID: {}", rec.id);
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error adding brand: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
        OrgCommand::BrandList => match list_brands(storage) {
            Ok(brands) => {
                if brands.is_empty() {
                    println!("No brands found.");
                } else {
                    println!("Brands ({}):", brands.len());
                    for b in brands {
                        println!("  {} - {}", b.id, b.name);
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error listing brands: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        OrgCommand::ProjectAdd { name, company_id } => {
            let company = match company_id.parse::<CompanyId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid company ID: {}", company_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match add_project(storage, &name, company) {
                Ok(rec) => {
                    println!("Project added: {}", rec.name);
                    println!("ID: {}", rec.id);
                    println!("Company ID: {}", rec.company);
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error adding project: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::ProjectList => match list_projects(storage) {
            Ok(projects) => {
                if projects.is_empty() {
                    println!("No projects found.");
                } else {
                    println!("Projects ({}):", projects.len());
                    for p in projects {
                        println!("  {} - {} (company: {})", p.id, p.name, p.company);
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error listing projects: {}", e);
                process::exit(EXIT_STORAGE_ERROR);
            }
        },
        OrgCommand::AssignAsset { host, project_id } => {
            let project = match project_id.parse::<ProjectId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid project ID: {}", project_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match assign_asset(storage, &host, project) {
                Ok(rec) => {
                    println!("Asset assigned: {} -> project {}", rec.host, rec.project);
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error assigning asset: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::AssignPerson {
            person_id,
            project_id,
            role,
        } => {
            let person = match person_id.parse::<PersonId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid person ID: {}", person_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let project = match project_id.parse::<ProjectId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid project ID: {}", project_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let role_enum = match role.to_lowercase().as_str() {
                "owner" => Role::Owner,
                "manager" => Role::Manager,
                "technician" => Role::Technician,
                _ => {
                    eprintln!(
                        "Invalid role: {}. Valid roles: owner, manager, technician",
                        role
                    );
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match assign_person_to_project(storage, person, project, role_enum) {
                Ok(rec) => {
                    println!(
                        "Person {} assigned to project {} as {}",
                        person, rec.project, rec.role
                    );
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error assigning person: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::AssignCompany {
            company_id,
            project_id,
            role,
        } => {
            let company = match company_id.parse::<CompanyId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid company ID: {}", company_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let project = match project_id.parse::<ProjectId>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid project ID: {}", project_id);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            let role_enum = match role.to_lowercase().as_str() {
                "owner" => Role::Owner,
                "manager" => Role::Manager,
                "technician" => Role::Technician,
                _ => {
                    eprintln!(
                        "Invalid role: {}. Valid roles: owner, manager, technician",
                        role
                    );
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match assign_company_to_project(storage, company, project, role_enum) {
                Ok(rec) => {
                    println!(
                        "Company {} assigned to project {} as {}",
                        company, rec.project, rec.role
                    );
                    process::exit(EXIT_SUCCESS);
                }
                Err(e) => {
                    eprintln!("Error assigning company: {}", e);
                    process::exit(EXIT_GENERAL_ERROR);
                }
            }
        }
        OrgCommand::Unassign { host } => match unassign_asset(storage, &host) {
            Ok(found) => {
                if found {
                    println!("Asset {} unassigned.", host);
                    println!("Note: The unassignment is recorded but the original assignment remains in the log.");
                } else {
                    println!("Asset {} was not assigned to any project.", host);
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error unassigning asset: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        },
    }
}
