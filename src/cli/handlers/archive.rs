//! Archive command handler (ADR 0013/0014). Feature `parquet-archive`.

#[cfg(feature = "parquet-archive")]
use std::sync::Arc;

use crate::error::Result;
#[cfg(feature = "parquet-archive")]
use crate::storage::SqliteStorage;

#[cfg(feature = "parquet-archive")]
const ARCHIVE_DIR: &str = "data/archive";

#[cfg(feature = "parquet-archive")]
fn open_store() -> Result<Arc<SqliteStorage>> {
    Ok(Arc::new(SqliteStorage::new("data/balebos.db")?))
}

/// Entry point; without the feature the commands explain what is missing.
pub fn cmd_archive(action: &str, segment: Option<String>) -> Result<()> {
    #[cfg(feature = "parquet-archive")]
    {
        run(action, segment)
    }
    #[cfg(not(feature = "parquet-archive"))]
    {
        let _ = (action, segment);
        eprintln!("The 'archive' commands require a build with --features parquet-archive.");
        eprintln!("Rebuild with: cargo build --release --features parquet-archive");
        std::process::exit(crate::exit_codes::EXIT_GENERAL_ERROR);
    }
}

#[cfg(feature = "parquet-archive")]
fn run(action: &str, segment: Option<String>) -> Result<()> {
    let storage = open_store()?;
    match action {
        "build" => {
            let report = crate::storage::archive::seal_pending(&storage, ARCHIVE_DIR)?;
            if report.record_count == 0 {
                println!("Nothing to archive: all observations are already sealed.");
            } else {
                println!(
                    "Sealed segment {} ({} records, rows {}..{}, adopted existing: {}).",
                    report.segment_id.unwrap_or_default(),
                    report.record_count,
                    report.boundary.as_ref().map(|b| b.from_rowid).unwrap_or(0),
                    report.boundary.as_ref().map(|b| b.to_rowid).unwrap_or(0),
                    report.adopted_existing
                );
            }
        }
        "verify" => {
            let report = crate::storage::archive::verify_archive(&storage, ARCHIVE_DIR)?;
            println!(
                "Checked {} segment(s): {} problem(s).",
                report.segments_checked,
                report.problems.len()
            );
            for problem in &report.problems {
                println!("  - {problem}");
            }
            if !report.is_ok() {
                std::process::exit(crate::exit_codes::EXIT_STORAGE_ERROR);
            }
        }
        "restore" => {
            let segment_id = segment.ok_or_else(|| {
                crate::error::BalebosError::MissingField("segment id".to_string())
            })?;
            let count =
                crate::storage::archive::restore_segment(&storage, ARCHIVE_DIR, &segment_id)?;
            println!("Restored {count} record(s) from segment {segment_id}.");
        }
        _ => {
            eprintln!("Unknown archive action: {action} (expected build|verify|restore)");
            std::process::exit(crate::exit_codes::EXIT_INVALID_ARGUMENT);
        }
    }
    Ok(())
}
