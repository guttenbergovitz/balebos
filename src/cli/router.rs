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
        _ => None,
    }
}
