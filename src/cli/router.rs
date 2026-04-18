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
    Web { output_dir: String },
    Org(OrgCommand),
    Tag(TagCommand),
    Finding(FindingCommand),
    Asset(AssetCommand),
    Risk(RiskCommand),
}

pub enum ProbeKind {
    Subdomains,
    Http,
    Dns,
    Asn,
    Hosting,
}

pub struct ExportArgs {
    pub format: String,
    pub output_file: Option<String>,
    #[allow(dead_code)]
    pub tag_filter: Option<String>,
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
        /// Brand ID. None means external contractor.
        brand_id: Option<String>,
    },
    CompanyList,
    BrandAdd {
        name: String,
    },
    BrandList,
    ProjectAdd {
        name: String,
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

/// Tag management commands.
pub enum TagCommand {
    Add {
        asset_name: String,
        tag: String,
    },
    AddBulk {
        asset_names: Vec<String>,
        tag: String,
    },
    Remove {
        asset_name: String,
        tag: String,
    },
    List {
        asset_name: Option<String>,
    },
    ListTags,
    ListByTag {
        tag: String,
    },
}

/// Finding management commands.
pub enum FindingCommand {
    Run,
    List {
        status: Option<FindingStatus>,
        severity: Option<FindingSeverityFilter>,
    },
    Show {
        id: String,
    },
    Acknowledge {
        id: String,
    },
    Resolve {
        id: String,
    },
    Stats,
    Cleanup {
        days: Option<u32>,
    },
}

pub enum FindingStatus {
    Open,
    Acknowledged,
    Resolved,
}

pub enum FindingSeverityFilter {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Asset management commands.
pub enum AssetCommand {
    LifecycleSet {
        asset_name: String,
        state: String,
        reason: Option<String>,
    },
    LifecycleShow {
        asset_name: String,
    },
    LifecycleHistory {
        asset_name: String,
    },
    LifecycleList,
}

pub enum RiskCommand {
    Report,
    Top { n: usize },
    Show { asset_name: String },
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
                let output_file = args.get(4).and_then(|s| {
                    if s.starts_with("--tag") {
                        None
                    } else {
                        Some(s.clone())
                    }
                });
                let tag_filter = find_flag_value(args.as_slice(), "--tag");
                Some(ExportArgs {
                    format,
                    output_file,
                    tag_filter,
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
                "asn" => ProbeKind::Asn,
                "hosting" => ProbeKind::Hosting,
                _ => return None,
            };
            Some(Command::Probe { kind, target })
        }
        "web" => {
            let action = args.get(2)?;
            match action.as_str() {
                "export" => {
                    let output_dir = find_flag_value(args.as_slice(), "--output-dir")
                        .or_else(|| find_flag_value(args.as_slice(), "-o"))
                        .unwrap_or_else(|| "web/data".to_string());
                    Some(Command::Web { output_dir })
                }
                _ => None,
            }
        }
        "org" => parse_org_command(&args),
        "tag" => parse_tag_command(&args),
        "finding" => parse_finding_command(&args),
        "asset" => parse_asset_command(&args),
        "risk" => parse_risk_command(&args),
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
            // balebos org company add "Name" "DE" [--brand <id>]
            let name = args.get(4)?.clone();
            let country = args.get(5)?.clone();
            let brand_id = find_flag_value(args, "--brand");
            Some(Command::Org(OrgCommand::CompanyAdd {
                name,
                country,
                brand_id,
            }))
        }
        ("company", "list") => Some(Command::Org(OrgCommand::CompanyList)),
        ("brand", "add") => {
            let name = args.get(4)?.clone();
            Some(Command::Org(OrgCommand::BrandAdd { name }))
        }
        ("brand", "list") => Some(Command::Org(OrgCommand::BrandList)),
        ("project", "add") => {
            // balebos org project add "Name" --company <id>
            let name = args.get(4)?.clone();
            let company_id = find_flag_value(args, "--company")?;
            Some(Command::Org(OrgCommand::ProjectAdd { name, company_id }))
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

fn parse_tag_command(args: &[String]) -> Option<Command> {
    let action = args.get(2)?;

    match action.as_str() {
        "add" => {
            let asset_name = args.get(3)?.clone();
            let tag = args.get(4)?.clone();
            Some(Command::Tag(TagCommand::Add { asset_name, tag }))
        }
        "add-bulk" => {
            // balebos tag add-bulk <tag> <asset1> <asset2> ...
            let tag = args.get(3)?.clone();
            let asset_names: Vec<String> = args.iter().skip(4).cloned().collect();
            if asset_names.is_empty() {
                return None;
            }
            Some(Command::Tag(TagCommand::AddBulk { asset_names, tag }))
        }
        "remove" => {
            let asset_name = args.get(3)?.clone();
            let tag = args.get(4)?.clone();
            Some(Command::Tag(TagCommand::Remove { asset_name, tag }))
        }
        "list" => {
            let asset_name = args.get(3).cloned();
            Some(Command::Tag(TagCommand::List { asset_name }))
        }
        "list-tags" => Some(Command::Tag(TagCommand::ListTags)),
        "by-tag" => {
            let tag = args.get(3)?.clone();
            Some(Command::Tag(TagCommand::ListByTag { tag }))
        }
        _ => None,
    }
}

fn parse_finding_command(args: &[String]) -> Option<Command> {
    let action = args.get(2)?;

    match action.as_str() {
        "run" => Some(Command::Finding(FindingCommand::Run)),
        "list" => {
            let mut status = None;
            let mut severity = None;

            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "--status" | "-s" => {
                        i += 1;
                        if i < args.len() {
                            status = match args[i].as_str() {
                                "open" => Some(FindingStatus::Open),
                                "acknowledged" => Some(FindingStatus::Acknowledged),
                                "resolved" => Some(FindingStatus::Resolved),
                                _ => return None,
                            };
                        }
                    }
                    "--severity" | "-l" => {
                        i += 1;
                        if i < args.len() {
                            severity = match args[i].as_str() {
                                "info" => Some(FindingSeverityFilter::Info),
                                "low" => Some(FindingSeverityFilter::Low),
                                "medium" => Some(FindingSeverityFilter::Medium),
                                "high" => Some(FindingSeverityFilter::High),
                                "critical" => Some(FindingSeverityFilter::Critical),
                                _ => return None,
                            };
                        }
                    }
                    _ => return None,
                }
                i += 1;
            }

            Some(Command::Finding(FindingCommand::List { status, severity }))
        }
        "show" => {
            let id = args.get(3)?.clone();
            Some(Command::Finding(FindingCommand::Show { id }))
        }
        "acknowledge" | "ack" => {
            let id = args.get(3)?.clone();
            Some(Command::Finding(FindingCommand::Acknowledge { id }))
        }
        "resolve" => {
            let id = args.get(3)?.clone();
            Some(Command::Finding(FindingCommand::Resolve { id }))
        }
        "stats" => Some(Command::Finding(FindingCommand::Stats)),
        "cleanup" => {
            let mut days = None;

            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "--days" | "-d" => {
                        i += 1;
                        if i < args.len() {
                            days = args[i].parse::<u32>().ok();
                        }
                    }
                    _ => return None,
                }
                i += 1;
            }

            Some(Command::Finding(FindingCommand::Cleanup { days }))
        }
        _ => None,
    }
}
fn parse_asset_command(args: &[String]) -> Option<Command> {
    let action = args.get(2)?;

    match action.as_str() {
        "lifecycle" => {
            let lifecycle_action = args.get(3)?;
            match lifecycle_action.as_str() {
                "set" => {
                    let asset_name = args.get(4)?.clone();
                    let state = args.get(5)?.clone();
                    let reason = find_flag_value(args, "--reason");
                    Some(Command::Asset(AssetCommand::LifecycleSet {
                        asset_name,
                        state,
                        reason,
                    }))
                }
                "show" => {
                    let asset_name = args.get(4)?.clone();
                    Some(Command::Asset(AssetCommand::LifecycleShow { asset_name }))
                }
                "history" => {
                    let asset_name = args.get(4)?.clone();
                    Some(Command::Asset(AssetCommand::LifecycleHistory { asset_name }))
                }
                "list" => Some(Command::Asset(AssetCommand::LifecycleList)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn parse_risk_command(args: &[String]) -> Option<Command> {
    let action = args.get(2)?;

    match action.as_str() {
        "report" => Some(Command::Risk(RiskCommand::Report)),
        "top" => {
            let n = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
            Some(Command::Risk(RiskCommand::Top { n }))
        }
        "show" => {
            let asset_name = args.get(3)?.clone();
            Some(Command::Risk(RiskCommand::Show { asset_name }))
        }
        _ => None,
    }
}
