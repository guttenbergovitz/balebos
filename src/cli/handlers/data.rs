//! Operational data directory management (`balebos data info|backup|reset`).

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;

use crate::error::Result;
use crate::storage::StorageEngine;

/// Display data directory location, database size and record counts.
pub fn cmd_data_info(data_dir: &str, storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let base = Path::new(data_dir);
    let db_path = base.join("balebos.db");
    let db_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    let records = storage.read_all_records()?;

    println!("Data directory: {}", base.display());
    println!("SQLite store:   {} ({} bytes)", db_path.display(), db_bytes);
    println!("  Assets (items):        {}", records.items.len());
    println!(
        "  DNS observations:      {}",
        records.dns_observations.len()
    );
    println!(
        "  HTTP observations:     {}",
        records.http_observations.len()
    );
    println!(
        "  ASN observations:      {}",
        records.asn_observations.len()
    );
    println!(
        "  Hosting observations:  {}",
        records.hosting_observations.len()
    );
    println!(
        "  Enrichment candidates: {}",
        records.enrichment_candidates.len()
    );
    println!(
        "  Candidate reviews:     {}",
        records.candidate_reviews.len()
    );
    println!("  Scan reports:          {}", records.scan_reports.len());
    println!("  Skipped records:       {}", records.skipped.len());
    Ok(())
}

/// Create a timestamped backup copy of `balebos.db` in `dest` (or `<data_dir>/backups`).
pub fn cmd_data_backup(data_dir: &str, dest: Option<&str>) -> Result<String> {
    let base = Path::new(data_dir);
    let db_path = base.join("balebos.db");
    let backup_dir = dest
        .map(Path::new)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base.join("backups"));
    std::fs::create_dir_all(&backup_dir)?;

    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let backup_file = backup_dir.join(format!("balebos-{ts}.db"));
    if db_path.exists() {
        std::fs::copy(&db_path, &backup_file)?;
    } else {
        std::fs::File::create(&backup_file)?;
    }
    println!("Backup written to {}", backup_file.display());
    Ok(backup_file.to_string_lossy().to_string())
}

/// Reset the operational SQLite database safely: requires `--dry-run` or `--backup`.
pub fn cmd_data_reset(data_dir: &str, dry_run: bool, backup: bool) -> Result<()> {
    let base = Path::new(data_dir);
    let db_path = base.join("balebos.db");
    let wal_path = base.join("balebos.db-wal");
    let shm_path = base.join("balebos.db-shm");

    if dry_run {
        println!(
            "Dry run — would remove operational SQLite files under {}:",
            base.display()
        );
        for p in [&db_path, &wal_path, &shm_path] {
            if p.exists() {
                println!("  - {}", p.display());
            }
        }
        println!("Archive directory (if any) would be preserved.");
        return Ok(());
    }

    if !backup {
        eprintln!("Refusing destructive reset without --dry-run or --backup.");
        std::process::exit(crate::exit_codes::EXIT_INVALID_ARGUMENT);
    }

    cmd_data_backup(data_dir, None)?;
    for p in [&db_path, &wal_path, &shm_path] {
        if p.exists() {
            std::fs::remove_file(p)?;
        }
    }
    println!("Operational database reset (archive preserved).");
    Ok(())
}
