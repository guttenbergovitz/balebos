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

/// Create a WAL-safe, verified backup copy of `balebos.db` in `dest` (or
/// `<data_dir>/backups`) using SQLite `VACUUM INTO` and `PRAGMA integrity_check`.
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
        if backup_file.exists() {
            std::fs::remove_file(&backup_file)?;
        }
        let conn = rusqlite::Connection::open(&db_path).map_err(|e| {
            crate::error::BalebosError::Io(std::io::Error::other(format!(
                "Failed to open SQLite DB for backup: {}",
                e
            )))
        })?;
        let dest_str = backup_file.to_string_lossy().to_string();
        conn.execute("VACUUM INTO ?1", [&dest_str]).map_err(|e| {
            crate::error::BalebosError::Io(std::io::Error::other(format!(
                "SQLite VACUUM INTO backup failed: {}",
                e
            )))
        })?;

        // Verify the resulting backup before reporting success or allowing reset.
        let verify_conn = rusqlite::Connection::open(&backup_file).map_err(|e| {
            crate::error::BalebosError::Io(std::io::Error::other(format!(
                "Failed to open backup DB for verification: {}",
                e
            )))
        })?;
        let integrity: String = verify_conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|e| {
                crate::error::BalebosError::Io(std::io::Error::other(format!(
                    "Backup integrity check failed: {}",
                    e
                )))
            })?;
        if integrity != "ok" {
            return Err(crate::error::BalebosError::Io(std::io::Error::other(
                format!("Backup integrity check returned '{}'", integrity),
            )));
        }
    } else {
        std::fs::File::create(&backup_file)?;
    }
    println!("Verified backup written to {}", backup_file.display());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryItem, ItemType};
    use crate::storage::{SqliteStorage, StorageEngineExt};

    #[test]
    fn wal_safe_backup_preserves_uncheckpointed_wal_transactions() {
        let dir = std::env::temp_dir().join(format!("balebos_wal_bak_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("balebos.db");
        let db_str = db_path.to_str().unwrap();

        let storage: Arc<dyn StorageEngine> = Arc::new(SqliteStorage::new(db_str).unwrap());
        storage
            .save(&InventoryItem {
                item_type: ItemType::Domain,
                id: "wal-item-1".to_string(),
                name: "wal-test.example.com".to_string(),
                original_input: Some("wal-test.example.com".to_string()),
                discovered_at: Utc::now(),
                source: "cli".to_string(),
            })
            .unwrap();
        storage.flush().unwrap();

        // Keep `storage` open (WAL active) while running backup!
        let backup_path = cmd_data_backup(dir.to_str().unwrap(), None).unwrap();
        drop(storage);

        let restored = SqliteStorage::new(&backup_path).unwrap();
        let records = restored.read_all_records().unwrap();
        assert_eq!(records.items.len(), 1);
        assert_eq!(records.items[0].name, "wal-test.example.com");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
