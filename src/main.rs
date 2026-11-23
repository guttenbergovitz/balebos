use std::process;
use std::sync::Arc;

use balebos::cli::{handlers, parse_command, Command};
use balebos::exit_codes::*;
use balebos::ignore::load_ignore_patterns;
use balebos::storage::StorageEngine;

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
    eprintln!(
        "  balebos drift [--within <hours>]   - Classified drift since the window (default 24h)"
    );
    eprintln!(
        "  balebos related <target>           - Show related entities and shared-infra clues"
    );
    eprintln!("  balebos explain <a> <b>            - Explain connection between two entities");
    eprintln!("  balebos history <target>           - Show chronological relationship history");
    eprintln!("  balebos migrate [dir]              - Migrate legacy Parquet data to SQLite");
    eprintln!("  balebos archive build|verify|restore [id] - Manage sealed evidence archive");
    eprintln!("  balebos ui                         - Launch interactive TUI dashboard");
    eprintln!("  balebos probe subdomains <domain>  - Discover subdomains using subfinder");
    eprintln!("  balebos probe http <domain>        - Check HTTP status of a domain");
    eprintln!();
    eprintln!("Finding management:");
    eprintln!("  balebos finding run                - Run checks and persist findings");
    eprintln!("  balebos finding list               - List findings");
    eprintln!("  balebos finding list --status open - List open findings");
    eprintln!("  balebos finding list --severity high - List high severity findings");
    eprintln!("  balebos finding show <id>          - Show finding details");
    eprintln!("  balebos finding acknowledge <id>   - Acknowledge a finding");
    eprintln!("  balebos finding resolve <id>       - Resolve a finding");
    eprintln!("  balebos finding stats              - Show findings statistics");
    eprintln!("  balebos finding cleanup [--days N] - Cleanup old findings (default: 90 days)");
    eprintln!();
    eprintln!("Tag management:");
    eprintln!("  balebos tag add <asset> <tag>      - Add a tag to an asset");
    eprintln!("  balebos tag add-bulk <tag> <asset1> <asset2> ... - Add tag to multiple assets");
    eprintln!("  balebos tag remove <asset> <tag>   - Remove a tag from an asset");
    eprintln!("  balebos tag list [asset]           - List tags (all or for specific asset)");
    eprintln!("  balebos tag list-tags              - List all unique tag names");
    eprintln!("  balebos tag by-tag <tag>           - List all assets with a specific tag");
    eprintln!();
    eprintln!("Organizational management:");
    eprintln!("  balebos org person add <first> <last> <email>  - Add a person");
    eprintln!("  balebos org person list                        - List all persons");
    eprintln!(
        "  balebos org company add <name> <country>       - Add a company (ISO 3166-1 alpha-2)"
    );
    eprintln!("  balebos org company list                       - List all companies");
    eprintln!("  balebos org brand add <name>                   - Add a brand");
    eprintln!("  balebos org brand list                         - List all brands");
    eprintln!("  balebos org project add <name> --company <id>");
    eprintln!("                                                 - Add a project");
    eprintln!("  balebos org project list                       - List all projects");
    eprintln!("  balebos org assign asset <host> --project <id> - Assign asset to project");
    eprintln!("  balebos org assign person <id> --project <id> --role <role>");
    eprintln!("                                                 - Assign person to project");
    eprintln!("  balebos org assign company <id> --project <id> --role <role>");
    eprintln!("                                                 - Assign company to project");
    eprintln!("  balebos org unassign <host>                    - Unassign asset from project");
    eprintln!();
    eprintln!("Asset lifecycle management:");
    eprintln!("  balebos asset lifecycle set <asset> <state> [--reason <text>]");
    eprintln!("                                     - Set asset lifecycle state");
    eprintln!("                                     - States: active, under_review, deprecated, decommissioned, abandoned");
    eprintln!("  balebos asset lifecycle show <asset>           - Show asset lifecycle info");
    eprintln!("  balebos asset lifecycle history <asset>        - Show lifecycle history");
    eprintln!(
        "  balebos asset lifecycle list                   - List all assets by lifecycle state"
    );
    eprintln!();
    eprintln!("Risk scoring:");
    eprintln!("  balebos risk report                - Show full risk report for all assets");
    eprintln!(
        "  balebos risk top [N]               - Show top N highest risk assets (default: 10)"
    );
    eprintln!("  balebos risk show <asset>          - Show detailed risk score for an asset");
}

fn main() {
    // SIGINT cancels in-flight scanner processes: the watchdog in
    // app::probe::process kills their process groups and reaps them
    // before the process exits, so no orphaned scanners outlive us.
    let _ = ctrlc::set_handler(|| {
        balebos::app::probe::process::cancel();
        eprintln!("\nCancelling: terminating scanner processes…");
    });

    // SQLite is the authoritative operational store (ADR 0012); it became
    // the default engine in Phase E after the migration path (ADR 0018)
    // shipped. Legacy Parquet data imports via `balebos migrate`.
    let storage: Arc<dyn StorageEngine> = Arc::new(
        balebos::storage::SqliteStorage::new("data/balebos.db")
            .expect("Failed to initialise SQLite storage"),
    );

    let ignore_patterns = load_ignore_patterns("balebos.ignore");

    let command = match parse_command() {
        Some(cmd) => cmd,
        None => {
            print_usage();
            process::exit(EXIT_INVALID_ARGUMENT);
        }
    };

    match command {
        Command::Scan { domain } => handlers::scan::cmd_scan(&domain, &storage),
        Command::Batch {
            file,
            allow_internal,
        } => handlers::scan::cmd_batch(&file, &storage, &ignore_patterns, allow_internal),
        Command::List { unique } => handlers::inventory::cmd_list(unique, &storage),
        Command::Tree => handlers::inventory::cmd_tree(&storage),
        Command::Status { export } => handlers::inventory::cmd_status(export, &storage),
        Command::Stats { by_domain } => handlers::inventory::cmd_stats(by_domain, &storage),
        Command::Diff => handlers::inventory::cmd_diff(&storage),
        Command::Check => handlers::check::cmd_check(&storage, &ignore_patterns),
        Command::Report => handlers::check::cmd_report(&storage, &ignore_patterns),
        Command::Probe {
            kind,
            target,
            allow_internal,
        } => handlers::probe::cmd_probe(kind, &target, &storage, allow_internal),
        Command::Drift { within_hours } => {
            if let Err(e) = handlers::drift::cmd_drift(within_hours, &storage) {
                eprintln!("Drift report failed: {e}");
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
        Command::Migrate { from } => {
            if let Err(e) = handlers::migrate::cmd_migrate(from) {
                eprintln!("Migration failed: {e}");
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
        Command::Archive { action, segment } => {
            if let Err(e) = handlers::archive::cmd_archive(&action, segment) {
                eprintln!("Archive operation failed: {e}");
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
        Command::Ui => handlers::ui::cmd_ui(storage.clone()),
        Command::Web { output_dir } => handlers::web::cmd_web_export(&output_dir, &storage),
        Command::Org(org_cmd) => handlers::org::cmd_org(org_cmd, &storage),
        Command::Tag(tag_cmd) => handlers::tag::cmd_tag(tag_cmd, &storage),
        Command::Finding(finding_cmd) => {
            handlers::finding::cmd_finding(finding_cmd, &storage, &ignore_patterns)
        }
        Command::Asset(asset_cmd) => handlers::asset::cmd_asset(asset_cmd, &storage),
        Command::Risk(risk_cmd) => handlers::risk::cmd_risk(risk_cmd, &storage),
        Command::Related { target } => {
            if let Err(e) = handlers::relationships::cmd_related(&target, &storage) {
                eprintln!("Related query failed: {e}");
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
        Command::Explain { entity_a, entity_b } => {
            if let Err(e) = handlers::relationships::cmd_explain(&entity_a, &entity_b, &storage) {
                eprintln!("Explain query failed: {e}");
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
        Command::History { target } => {
            if let Err(e) = handlers::relationships::cmd_history(&target, &storage) {
                eprintln!("History query failed: {e}");
                process::exit(EXIT_STORAGE_ERROR);
            }
        }
    }

    // Flush buffered records before exit.
    // CLI handlers use process::exit() which skips Drop destructors,
    // so any records saved but not yet flushed would be lost.
    let _ = storage.flush();
}
