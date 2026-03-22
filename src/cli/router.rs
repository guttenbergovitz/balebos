/// Parsed CLI command with its arguments.
pub enum Command {
    Scan { domain: String },
    Batch { file: String },
    List { unique: bool },
    Tree,
    Status { export: Option<ExportArgs> },
    Stats { by_domain: bool },
    Diff,
    Check,
    Report,
    Probe { kind: ProbeKind, target: String },
    Ui,
    Org(OrgCommand),
}

pub enum ProbeKind {
    Subdomains,
    Http,
    Dns,
}

pub struct ExportArgs {
    pub format: String,
    pub output_file: Option<String>,
}

/// Organizational management commands.
pub enum OrgCommand {
    PersonAdd {
        first_name: String,
        last_name: String,
        email: String,
    },
    PersonList,
    CompanyAdd {
        name: String,
        country: String,
        is_external: bool,
    },
    CompanyList,
    BrandAdd {
        name: String,
    },
    BrandList,
    ProjectAdd {
        name: String,
        brand_id: String,
        company_id: String,
    },
    ProjectList,
    AssignAsset {
        host: String,
        project_id: String,
    },
    AssignPerson {
        person_id: String,
        project_id: String,
        role: String,
    },
    AssignCompany {
        company_id: String,
        project_id: String,
        role: String,
    },
    Unassign {
        host: String,
    },
}

/// Parse command-line arguments into a structured Command.
///
/// Returns `None` if no valid command is recognized
/// (caller should print usage and exit).
pub fn parse_command() -> Option<Command> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        return None;
    }

    match args[1].as_str() {
        "scan" => {
            let domain = args.get(2)?.clone();
            Some(Command::Scan { domain })
        }
        "batch" => {
            let file = args.get(2)?.clone();
            Some(Command::Batch { file })
        }
        "list" => {
            let unique = args.get(2).is_some_and(|a| a == "--unique");
            Some(Command::List { unique })
        }
        "tree" => Some(Command::Tree),
        "status" => {
            let export = if args.get(2).is_some_and(|a| a == "--export" || a == "-e") {
                let format = args.get(3)?.clone();
                let output_file = args.get(4).cloned();
                Some(ExportArgs {
                    format,
                    output_file,
                })
            } else {
                None
            };
            Some(Command::Status { export })
        }
        "stats" => {
            let by_domain = args.get(2).is_some_and(|a| a == "--by-domain");
            Some(Command::Stats { by_domain })
        }
        "diff" => Some(Command::Diff),
        "check" => Some(Command::Check),
        "report" => Some(Command::Report),
        "ui" => Some(Command::Ui),
        "probe" => {
            let kind_str = args.get(2)?;
            let target = args.get(3)?.clone();
            let kind = match kind_str.as_str() {
                "subdomains" => ProbeKind::Subdomains,
                "http" => ProbeKind::Http,
                "dns" => ProbeKind::Dns,
                _ => return None,
            };
            Some(Command::Probe { kind, target })
        }
        "org" => parse_org_command(&args),
        _ => None,
    }
}

fn parse_org_command(args: &[String]) -> Option<Command> {
    let entity = args.get(2)?;
    let action = args.get(3)?;

    match (entity.as_str(), action.as_str()) {
        ("person", "add") => {
            let first_name = args.get(4)?.clone();
            let last_name = args.get(5)?.clone();
            let email = args.get(6)?.clone();
            Some(Command::Org(OrgCommand::PersonAdd {
                first_name,
                last_name,
                email,
            }))
        }
        ("person", "list") => Some(Command::Org(OrgCommand::PersonList)),
        ("company", "add") => {
            let name = args.get(4)?.clone();
            let country = args.get(5)?.clone();
            let is_external = args.get(6).is_some_and(|a| a == "--external");
            Some(Command::Org(OrgCommand::CompanyAdd {
                name,
                country,
                is_external,
            }))
        }
        ("company", "list") => Some(Command::Org(OrgCommand::CompanyList)),
        ("brand", "add") => {
            let name = args.get(4)?.clone();
            Some(Command::Org(OrgCommand::BrandAdd { name }))
        }
        ("brand", "list") => Some(Command::Org(OrgCommand::BrandList)),
        ("project", "add") => {
            // balebos org project add "Name" --brand <id> --company <id>
            let name = args.get(4)?.clone();
            let mut brand_id = None;
            let mut company_id = None;
            let mut i = 5;
            while i < args.len() {
                match args[i].as_str() {
                    "--brand" => {
                        brand_id = args.get(i + 1).cloned();
                        i += 2;
                    }
                    "--company" => {
                        company_id = args.get(i + 1).cloned();
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            Some(Command::Org(OrgCommand::ProjectAdd {
                name,
                brand_id: brand_id?,
                company_id: company_id?,
            }))
        }
        ("project", "list") => Some(Command::Org(OrgCommand::ProjectList)),
        ("assign", "asset") => {
            // balebos org assign asset <host> --project <id>
            let host = args.get(4)?.clone();
            let project_id = find_flag_value(args, "--project")?;
            Some(Command::Org(OrgCommand::AssignAsset { host, project_id }))
        }
        ("assign", "person") => {
            // balebos org assign person <person_id> --project <id> --role <role>
            let person_id = args.get(4)?.clone();
            let project_id = find_flag_value(args, "--project")?;
            let role = find_flag_value(args, "--role")?;
            Some(Command::Org(OrgCommand::AssignPerson {
                person_id,
                project_id,
                role,
            }))
        }
        ("assign", "company") => {
            // balebos org assign company <company_id> --project <id> --role <role>
            let company_id = args.get(4)?.clone();
            let project_id = find_flag_value(args, "--project")?;
            let role = find_flag_value(args, "--role")?;
            Some(Command::Org(OrgCommand::AssignCompany {
                company_id,
                project_id,
                role,
            }))
        }
        ("unassign", _) => {
            // balebos org unassign <host>
            let host = action.clone();
            Some(Command::Org(OrgCommand::Unassign { host }))
        }
        _ => None,
    }
}

fn find_flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}
