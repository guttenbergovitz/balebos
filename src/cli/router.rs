/// Parsed CLI command with its arguments.
pub enum Command {
    Scan {
        domain: String,
    },
    Batch {
        file: String,
        allow_internal: bool,
    },
    List {
        unique: bool,
    },
    Tree,
    Status {
        export: Option<ExportArgs>,
    },
    Stats {
        by_domain: bool,
    },
    Diff,
    Check,
    Report,
    Probe {
        kind: ProbeKind,
        target: String,
        allow_internal: bool,
    },
    Drift {
        within_hours: u64,
    },
    Migrate {
        from: Option<String>,
    },
    Archive {
        action: String,
        segment: Option<String>,
    },
    Ui,
    Web {
        output_dir: String,
    },
    Org(OrgCommand),
    Tag(TagCommand),
    Finding(FindingCommand),
    Asset(AssetCommand),
    Risk(RiskCommand),
    Related {
        target: String,
    },
    Explain {
        entity_a: String,
        entity_b: String,
    },
    History {
        target: String,
    },
    Pivot {
        target: String,
        max_depth: usize,
        max_candidates: usize,
        scope_domains: Vec<String>,
    },
    SourcesStatus,
    Discover {
        source: String,
        domain: String,
        input_file: Option<String>,
        scope_domains: Vec<String>,
    },
    DiscoveryReport {
        json: bool,
    },
    CandidatesList,
    CandidatesReview {
        entity: String,
        decision: String,
    },
    DataInfo,
    DataBackup {
        dest: Option<String>,
    },
    DataReset {
        dry_run: bool,
        backup: bool,
    },
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
    Junk {
        asset_name: String,
    },
    Unjunk {
        asset_name: String,
    },
    ListJunk,
}

pub enum RiskCommand {
    Report,
    Top { n: usize },
    Show { asset_name: String },
}

/// Parse command-line arguments into a structured Command, stripping any
/// leading `--data-dir <path>` global flag.
///
/// Returns `None` if no valid command is recognized
/// (caller should print usage and exit).
pub fn parse_command() -> Option<Command> {
    let raw_args: Vec<String> = std::env::args().collect();
    let mut args = Vec::with_capacity(raw_args.len());
    let mut i = 0;
    while i < raw_args.len() {
        if raw_args[i] == "--data-dir" {
            i += 2;
            continue;
        }
        args.push(raw_args[i].clone());
        i += 1;
    }

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
            let allow_internal = args.iter().any(|a| a == "--allow-internal");
            Some(Command::Batch {
                file,
                allow_internal,
            })
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
        "drift" => {
            let within_hours = find_flag_value(args.as_slice(), "--within")
                .and_then(|value| value.parse().ok())
                .unwrap_or(24);
            Some(Command::Drift { within_hours })
        }
        "related" => {
            let target = args.get(2)?.clone();
            Some(Command::Related { target })
        }
        "explain" => {
            let entity_a = args.get(2)?.clone();
            let entity_b = args.get(3)?.clone();
            Some(Command::Explain { entity_a, entity_b })
        }
        "history" => {
            let target = args.get(2)?.clone();
            Some(Command::History { target })
        }
        "pivot" => {
            let target = args.get(2)?.clone();
            let max_depth = find_flag_value(args.as_slice(), "--depth")
                .and_then(|v| v.parse().ok())
                .unwrap_or(2);
            let max_candidates = find_flag_value(args.as_slice(), "--limit")
                .and_then(|v| v.parse().ok())
                .unwrap_or(50);
            let scope_domains = find_flag_value(args.as_slice(), "--scope")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            Some(Command::Pivot {
                target,
                max_depth,
                max_candidates,
                scope_domains,
            })
        }
        "sources" if args.get(2).is_some_and(|a| a == "status") => Some(Command::SourcesStatus),
        "discovery" if args.get(2).is_some_and(|a| a == "report") => {
            let json = args.iter().any(|a| a == "--json");
            Some(Command::DiscoveryReport { json })
        }
        "candidates" if args.get(2).is_some_and(|a| a == "list") => Some(Command::CandidatesList),
        "candidates" if args.get(2).is_some_and(|a| a == "review") => {
            let entity = args.get(3)?.clone();
            let decision = args.get(4)?.clone();
            Some(Command::CandidatesReview { entity, decision })
        }
        "data" if args.get(2).is_some_and(|a| a == "info") => Some(Command::DataInfo),
        "data" if args.get(2).is_some_and(|a| a == "backup") => {
            let dest = args.get(3).cloned();
            Some(Command::DataBackup { dest })
        }
        "data" if args.get(2).is_some_and(|a| a == "reset") => {
            let dry_run = args.iter().any(|a| a == "--dry-run");
            let backup = args.iter().any(|a| a == "--backup");
            Some(Command::DataReset { dry_run, backup })
        }
        "discover" => {
            let source = find_flag_value(args.as_slice(), "--source")?;
            let input_file = find_flag_value(args.as_slice(), "--input");
            let scope_domains = find_flag_value(args.as_slice(), "--scope")
                .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            let domain = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with("--"))
                .cloned()?;
            Some(Command::Discover {
                source,
                domain,
                input_file,
                scope_domains,
            })
        }
        "migrate" => {
            let from = args.get(2).cloned();
            Some(Command::Migrate { from })
        }
        "archive" => {
            let action = args.get(2)?.clone();
            let segment = args.get(3).cloned();
            Some(Command::Archive { action, segment })
        }
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
            let allow_internal = args.iter().any(|a| a == "--allow-internal");
            Some(Command::Probe {
                kind,
                target,
                allow_internal,
            })
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
                    Some(Command::Asset(AssetCommand::LifecycleHistory {
                        asset_name,
                    }))
                }
                "list" => Some(Command::Asset(AssetCommand::LifecycleList)),
                _ => None,
            }
        }
        "junk" => {
            let asset_name = args.get(3)?.clone();
            Some(Command::Asset(AssetCommand::Junk { asset_name }))
        }
        "unjunk" => {
            let asset_name = args.get(3)?.clone();
            Some(Command::Asset(AssetCommand::Unjunk { asset_name }))
        }
        "list-junk" => Some(Command::Asset(AssetCommand::ListJunk)),
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
