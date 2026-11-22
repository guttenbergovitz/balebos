//! Migration command handler (ADR 0018).

use std::sync::Arc;

use crate::error::Result;
use crate::storage::{migration, SqliteStorage};

/// Migrate legacy per-type Parquet files into the SQLite store.
///
/// Always targets `data/balebos.db`: migration produces the new store;
/// switching the engine to it is a separate, deliberate act. On verified
/// success the legacy directory is renamed (never deleted) so rollback
/// stays possible.
pub fn cmd_migrate(from: Option<String>) -> Result<()> {
    let legacy_dir = from.unwrap_or_else(|| "data/parquet".to_string());

    println!("Migrating legacy Parquet data from: {legacy_dir}");
    println!("Target store: data/balebos.db");

    let storage = Arc::new(SqliteStorage::new("data/balebos.db")?);
    let report = migration::migrate_legacy_parquet(&legacy_dir, &storage)?;

    println!();
    println!("Migration complete:");
    println!("  Files processed:     {}", report.files);
    println!("  Rows read:           {}", report.rows_read);
    println!("  Rows accounted for:  {}", report.accounted());
    for (record_type, count) in &report.imported {
        println!("  Imported {record_type}: {count}");
    }
    println!("  Already present:     {}", report.already_present);
    println!("  Quarantined (unknown type): {}", report.unknown_type);
    println!("  Quarantined (malformed):    {}", report.malformed);
    println!("  Verification: per-type counts and quarantine counts match.");

    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let renamed = format!("{legacy_dir}.migrated-{stamp}");
    match std::fs::rename(&legacy_dir, &renamed) {
        Ok(()) => println!("Legacy data renamed to: {renamed} (kept for rollback)"),
        Err(e) => println!("Warning: could not rename legacy directory ({e}); left in place"),
    }

    Ok(())
}
