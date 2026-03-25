mod app;
mod cli;
mod domain;
mod error;
mod exit_codes;
mod ignore;
mod inventory;
mod org;
mod storage;
mod tui;

use exit_codes::*;
use ignore::load_ignore_patterns;
use std::process;

use app::probe::HttpErrorKind;
use app::{
    batch_process, check_inventory, display_domain_status, display_domain_tree, export_status_csv,
    export_status_html, export_status_markdown, generate_report, get_domain_stats,
    get_inventory_diff, http_check, list_domains, probe_subdomains, scan_domain, Severity,
};
use cli::{parse_command, Command, OrgCommand, ProbeKind};
use crate::storage::{StorageEngine, JsonlStorage};
use std::sync::Arc;

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  balebos scan <domain>              - Scan a domain and add to inventory");
    eprintln!(
        "  balebos batch <file>               - Batch process domains from file (full recon)"
    );
    eprintln!("  balebos list                       - List all domains in inventory");
    eprintln!("  balebos list --unique              - List unique domains in inventory");
    eprintln!("  balebos tree                       - Show domain hierarchy with subdomains");
    eprintln!("  balebos status                     - Show domain live status with HTTP codes");
    eprintln!("  balebos stats                      - Show basic domain statistics");
    eprintln!("  balebos stats --by-domain          - Show detailed statistics per domain");
    eprintln!("  balebos diff                       - Show domain changes (last 7 days)");
    eprintln!("  balebos check                      - Check for inventory issues and anomalies");
    eprintln!("  balebos report                     - Generate consolidated status report");
    eprintln!("  balebos ui                         - Launch interactive TUI dashboard");
    eprintln!("  balebos probe subdomains <domain>  - Discover subdomains using subfinder");
    eprintln!("  balebos probe http <domain>        - Check HTTP status of a domain");
    eprintln!();
    eprintln!("Organizational management:");
    eprintln!("  balebos org person add <first> <last> <email>  - Add a person");
    eprintln!("  balebos org person list                        - List all persons");
    eprintln!("  balebos org company add <name> <country>       - Add a company (ISO 3166-1 alpha-2)");
    eprintln!("  balebos org company list                       - List all companies");
    eprintln!("  balebos org brand add <name>                   - Add a brand");
    eprintln!("  balebos org brand list                         - List all brands");
    eprintln!("  balebos org project add <name> --brand <id> --company <id>");
    eprintln!("                                                 - Add a project");
    eprintln!("  balebos org project list                       - List all projects");
    eprintln!("  balebos org assign asset <host> --project <id> - Assign asset to project");
    eprintln!("  balebos org assign person <id> --project <id> --role <role>");
    eprintln!("                                                 - Assign person to project");
    eprintln!("  balebos org assign company <id> --project <id> --role <role>");
    eprintln!("                                                 - Assign company to project");
    eprintln!("  balebos org unassign <host>                    - Unassign asset from project");
}

fn main() {
    let use_duckdb = std::env::var("BALEBOS_STORAGE").unwrap_or_default() == "duckdb";
    let storage: Arc<dyn StorageEngine> = if use_duckdb {
        Arc::new(crate::storage::duckdb::DuckDbStorage::new("data/inventory.duckdb").unwrap())
    } else {
        Arc::new(JsonlStorage::new("data/inventory.jsonl"))
    };
    let ignore_patterns = load_ignore_patterns("balebos.ignore");

    let command = match parse_command() {
        Some(cmd) => cmd,
        None => {
            print_usage();
            process::exit(EXIT_INVALID_ARGUMENT);
        }
    };

    match command {
        Command::Scan { domain } => cmd_scan(&domain, &storage),
        Command::Batch { file } => cmd_batch(&file, &storage, &ignore_patterns),
        Command::List { unique } => cmd_list(unique, &storage),
        Command::Tree => cmd_tree(&storage),
        Command::Status { export } => cmd_status(export, &storage),
        Command::Stats { by_domain } => cmd_stats(by_domain, &storage),
        Command::Diff => cmd_diff(&storage),
        Command::Check => cmd_check(&storage, &ignore_patterns),
        Command::Report => cmd_report(&storage, &ignore_patterns),
        Command::Probe { kind, target } => cmd_probe(kind, &target, &storage),
        Command::Ui => cmd_ui(storage),
        Command::Org(org_cmd) => cmd_org(org_cmd, &storage),
    }
}

fn cmd_ui(storage: Arc<dyn StorageEngine>) {
    if let Err(e) = tui::run(storage) {
        eprintln!("TUI error: {}", e);
        process::exit(EXIT_GENERAL_ERROR);
    }
    process::exit(EXIT_SUCCESS);
}

fn cmd_scan(domain_name: &str, storage: &Arc<dyn StorageEngine>) {
    match scan_domain(domain_name, storage) {
        Ok(domain) => {
            println!("Domain scanned successfully: {}", domain.name);
            println!("Domain ID: {:?}", domain.id);
            println!("Discovered at: {}", domain.discovered_at);

            if domain_name != domain.name {
                println!("Normalized from: {}", domain_name);
            }
            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error scanning domain: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    }
}

fn cmd_batch(file: &str, storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
    match batch_process(file, storage, ignore_patterns) {
        Ok(_) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error during batch processing: {}", e);
            process::exit(EXIT_GENERAL_ERROR);
        }
    }
}

fn cmd_list(unique: bool, storage: &Arc<dyn StorageEngine>) {
    match list_domains(storage, unique) {
        Ok(domains) => {
            if domains.is_empty() {
                println!("No domains found in inventory.");
            } else {
                let label = if unique { "Unique" } else { "All" };
                println!("{} domains in inventory ({}):", label, domains.len());
                for domain in domains {
                    println!("- {}", domain);
                }
            }
            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error listing domains: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_tree(storage: &Arc<dyn StorageEngine>) {
    match display_domain_tree(storage) {
        Ok(()) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error displaying domain tree: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_status(export: Option<cli::ExportArgs>, storage: &Arc<dyn StorageEngine>) {
    if let Some(export_args) = export {
        let output_file = export_args.output_file.unwrap_or_else(|| {
            let ext = match export_args.format.as_str() {
                "csv" => "csv",
                "markdown" | "md" => "md",
                "html" => "html",
                _ => {
                    eprintln!("Unknown format: {}", export_args.format);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            format!("domain-status-report.{}", ext)
        });

        let result = match export_args.format.as_str() {
            "csv" => export_status_csv(storage, &output_file),
            "markdown" | "md" => export_status_markdown(storage, &output_file),
            "html" => export_status_html(storage, &output_file),
            _ => {
                eprintln!("Unknown format: {}", export_args.format);
                process::exit(EXIT_INVALID_ARGUMENT);
            }
        };

        match result {
            Ok(()) => process::exit(EXIT_SUCCESS),
            Err(e) => {
                eprintln!("Error exporting status report: {}", e);
                process::exit(EXIT_GENERAL_ERROR);
            }
        }
    }

    match display_domain_status(storage) {
        Ok(()) => process::exit(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("Error displaying domain status: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_stats(by_domain: bool, storage: &Arc<dyn StorageEngine>) {
    match get_domain_stats(storage) {
        Ok(stats) => {
            if by_domain {
                println!("Domain Statistics By Domain");
                println!("---------------------------");
            } else {
                println!("Domain Statistics");
                println!("----------------");
            }

            println!("Total domain entries: {}", stats.total_entries);
            println!("Unique domains: {}", stats.unique_domains);

            if by_domain {
                if !stats.top_domains.is_empty() {
                    println!("\nTop Domains:");
                    for (domain, count) in stats.top_domains {
                        println!("- {} ({} entries)", domain, count);
                    }
                } else {
                    println!("\nNo domains found in inventory.");
                }
            } else if let Some(latest) = stats.latest_domain {
                println!("Latest domain: {}", latest);
                if let Some(timestamp) = stats.latest_timestamp {
                    println!("Added on: {}", timestamp);
                }
            } else {
                println!("No domains found in inventory.");
            }

            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error getting domain statistics: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_diff(storage: &Arc<dyn StorageEngine>) {
    match get_inventory_diff(storage, None) {
        Ok(diff) => {
            println!("Domain Changes (last 7 days)");
            println!("---------------------------");

            if !diff.new_domains.is_empty() {
                println!("\nNew domains:");
                for domain in &diff.new_domains {
                    println!("+ {}", domain);
                }
            }

            if !diff.missing_domains.is_empty() {
                println!("\nMissing domains (not seen recently):");
                for domain in &diff.missing_domains {
                    println!("- {}", domain);
                }
            }

            if diff.new_domains.is_empty() && diff.missing_domains.is_empty() {
                println!("No changes detected.");
            }

            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error getting inventory diff: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_check(storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
    match check_inventory(storage, ignore_patterns) {
        Ok(issues) => {
            if issues.is_empty() {
                println!("No issues found in inventory.");
                process::exit(EXIT_SUCCESS);
            }

            let critical_count = issues
                .iter()
                .filter(|i| i.severity == Severity::Critical)
                .count();
            let warning_count = issues
                .iter()
                .filter(|i| i.severity == Severity::Warning)
                .count();

            println!("Inventory Check Results");
            println!("----------------------");
            println!(
                "Found {} issues ({} critical, {} warnings)",
                issues.len(),
                critical_count,
                warning_count
            );

            if critical_count > 0 {
                println!("\nCRITICAL ISSUES:");
                for issue in issues.iter().filter(|i| i.severity == Severity::Critical) {
                    println!("! {}", issue.message);
                }
            }

            if warning_count > 0 {
                println!("\nWARNINGS:");
                for issue in issues.iter().filter(|i| i.severity == Severity::Warning) {
                    println!("- {}", issue.message);
                }
            }

            if critical_count > 0 {
                process::exit(EXIT_CRITICAL_ISSUES);
            } else if warning_count > 0 {
                process::exit(EXIT_WARNING_ISSUES);
            }
            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error checking inventory: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_report(storage: &Arc<dyn StorageEngine>, ignore_patterns: &[String]) {
    match generate_report(storage, ignore_patterns) {
        Ok(report) => {
            println!("Balebos Report (last 7 days)");
            println!("----------------------------");
            println!();

            println!("Inventory:");
            println!("- total domains: {}", report.total_domains);
            println!("- unique domains: {}", report.unique_domains);
            println!();

            println!("Changes:");
            print_count_line("+", "new domain", report.new_domains);
            print_count_line("-", "missing domain", report.missing_domains);

            if report.new_subdomains > 0 {
                print_count_line("+", "new subdomain", report.new_subdomains);
            }
            if report.missing_subdomains > 0 {
                print_count_line("-", "missing subdomain", report.missing_subdomains);
            }
            println!();

            println!("Issues:");
            let total_issues = report.critical_issues + report.warning_issues;

            if total_issues == 0 {
                println!("- no issues detected");
            } else {
                print_issue_line("stale domain", report.stale_domains, " (>14 days)");
                print_issue_line("low confidence domain", report.low_confidence_domains, "");
                print_issue_line("unknown source", report.unknown_sources, "");
                print_issue_line("normalization issue", report.invalid_data, "");
                print_issue_line(
                    "suspicious subdomain pattern",
                    report.suspicious_subdomains,
                    "",
                );
                print_issue_line("HTTP status error", report.http_errors, "");
                print_issue_line(
                    "domain without recent HTTP checks",
                    report.missing_http_checks,
                    "",
                );
            }
            println!();

            println!("Summary:");
            if report.needs_attention {
                println!("⚠️ attention required");
            } else {
                println!("✅ all systems normal");
            }
            println!();

            println!("Run specific commands for details:");
            println!("- 'balebos diff' for change details");
            println!("- 'balebos check' for issue details");

            if report.needs_attention {
                process::exit(EXIT_WARNING_ISSUES);
            }
            process::exit(EXIT_SUCCESS);
        }
        Err(e) => {
            eprintln!("Error generating report: {}", e);
            process::exit(EXIT_STORAGE_ERROR);
        }
    }
}

fn cmd_probe(kind: ProbeKind, target: &str, storage: &Arc<dyn StorageEngine>) {
    match kind {
        ProbeKind::Subdomains => match probe_subdomains(target, storage) {
            Ok(subdomains) => {
                if subdomains.is_empty() {
                    println!("No subdomains found for {}", target);
                } else {
                    println!("Found {} subdomains for {}:", subdomains.len(), target);
                    for subdomain in &subdomains {
                        println!("- {}", subdomain);
                    }
                    println!("\nAll subdomains added to inventory.");
                    println!(
                        "Run 'balebos diff' to see changes or 'balebos check' to detect anomalies."
                    );
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error probing subdomains: {}", e);
                eprintln!(
                    "\nMake sure subfinder is installed: https://github.com/projectdiscovery/subfinder"
                );
                process::exit(EXIT_EXTERNAL_TOOL_ERROR);
            }
        },
        ProbeKind::Http => match http_check(target, storage) {
            Ok(observation) => {
                println!("HTTP check completed for {}", target);

                if let Some(status) = observation.status {
                    println!("Status: {}", status);
                } else if let Some(error_kind) = observation.error_kind {
                    println!("Probe failed: {:?}", error_kind);
                    println!(
                        "Error: {}",
                        observation.error_message.as_deref().unwrap_or("unknown")
                    );
                }

                if !observation.redirect_chain.is_empty() {
                    println!("\nRedirect chain ({} hops):", observation.redirect_chain.len());
                    for (i, hop) in observation.redirect_chain.iter().enumerate() {
                        println!("  {}. {} ({}) -> {}", i + 1, hop.url, hop.status, hop.redirect_to);
                    }
                    if let Some(final_url) = &observation.final_url {
                        println!("  Final URL: {}", final_url);
                    }
                }

                println!("\nDetails:");
                if let Some(title) = &observation.title {
                    println!("  Title: {}", title);
                }
                if let Some(server) = &observation.webserver {
                    println!("  Server: {}", server);
                }
                if !observation.tech.is_empty() {
                    println!("  Tech: {}", observation.tech.join(", "));
                }

                println!("Response time: {}ms", observation.response_time_ms);

                println!("\nObservation added to inventory.");
                println!(
                    "Run 'balebos check' to detect anomalies or 'balebos report' for a status summary."
                );

                if let Some(error_kind) = observation.error_kind {
                    match error_kind {
                        HttpErrorKind::DnsNotFound => process::exit(EXIT_DOMAIN_NOT_FOUND),
                        _ => process::exit(EXIT_PROBE_ERROR),
                    }
                } else if let Some(status) = observation.status {
                    if status >= 500 {
                        process::exit(EXIT_HTTP_SERVER_ERROR);
                    } else if status >= 400 {
                        process::exit(EXIT_HTTP_CLIENT_ERROR);
                    }
                }
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error checking HTTP status: {}", e);
                process::exit(EXIT_HTTP_ERROR);
            }
        },
        ProbeKind::Dns => match crate::app::probe::probe_dns(target, storage) {
            Ok(obs) => {
                println!("DNS check completed for {}", obs.target);
                
                if !obs.a.is_empty() {
                    println!("A: {}", obs.a.join(", "));
                }
                if !obs.aaaa.is_empty() {
                    println!("AAAA: {}", obs.aaaa.join(", "));
                }
                if !obs.cname.is_empty() {
                    println!("CNAME: {}", obs.cname.join(", "));
                }
                
                if obs.a.is_empty() && obs.aaaa.is_empty() && obs.cname.is_empty() {
                    println!("No DNS records found.");
                }

                println!("\nObservation added to inventory.");
                process::exit(EXIT_SUCCESS);
            }
            Err(e) => {
                eprintln!("Error checking DNS records: {}", e);
                process::exit(EXIT_EXTERNAL_TOOL_ERROR);
            }
        },
    }
}

/// Print a line like `+ 3 new domains` or `+ no new domains`.
fn print_count_line(prefix: &str, label: &str, count: usize) {
    if count > 0 {
        let suffix = if count == 1 { "" } else { "s" };
        println!("{} {} {}{}", prefix, count, label, suffix);
    } else {
        println!("{} no {}s", prefix, label);
    }
}

/// Print issue line only if count > 0.
fn print_issue_line(label: &str, count: usize, extra: &str) {
    if count > 0 {
        let suffix = if count == 1 { "" } else { "s" };
        println!("- {} {}{}{}", count, label, suffix, extra);
    }
}

fn cmd_org(org_cmd: OrgCommand, storage: &Arc<dyn StorageEngine>) {
    use app::org::*;
    use org::{BrandId, CompanyId, CountryCode, PersonId, ProjectId, Role};

    match org_cmd {
        OrgCommand::PersonAdd {
            first_name,
            last_name,
            email,
        } => match add_person(storage, &first_name, &last_name, &email, None) {
            Ok(rec) => {
                println!("Person added: {} {} <{}>", rec.first_name, rec.last_name, rec.email);
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
                        println!("  {} - {} {} <{}>", p.id, p.first_name, p.last_name, p.email);
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
                    eprintln!("Invalid country code: {}. Use ISO 3166-1 alpha-2 (e.g., DE, PL, US)", country);
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
                        println!("  {} - {} ({}){}", c.id, c.name, c.country.name(), brand_info);
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
                    eprintln!("Invalid role: {}. Valid roles: owner, manager, technician", role);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match assign_person_to_project(storage, person, project, role_enum) {
                Ok(rec) => {
                    println!(
                        "Person {} assigned to project {} as {}",
                        person,
                        rec.project,
                        rec.role
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
                    eprintln!("Invalid role: {}. Valid roles: owner, manager, technician", role);
                    process::exit(EXIT_INVALID_ARGUMENT);
                }
            };
            match assign_company_to_project(storage, company, project, role_enum) {
                Ok(rec) => {
                    println!(
                        "Company {} assigned to project {} as {}",
                        company,
                        rec.project,
                        rec.role
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
