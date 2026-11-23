//! SQLite operational storage (ADR 0012).
//!
//! The authoritative local store: append-only `records` rows inside
//! transactional batches, with a `quarantine` table for records whose type
//! is unknown. Every write commits before `save` returns, so durability
//! never depends on destructors or explicit flush calls. Historical rows
//! are never updated; current state is a query-time projection.
//!
//! Concurrency model: a single process owns the database file. The mutex
//! serialises access in-process; WAL allows readers during writes.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{BalebosError, Result};
use crate::inventory::ItemType;
use crate::storage::reader::{parse_records, Records};
use crate::storage::StorageEngine;

fn sqlite_err(e: rusqlite::Error) -> BalebosError {
    BalebosError::Storage(crate::error::StorageError::Sqlite(e.to_string()))
}

/// SQLite-backed storage engine.
pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteStorage").finish_non_exhaustive()
    }
}

impl SqliteStorage {
    /// Open (or create) the database at `path` and ensure the schema exists.
    pub fn new(path: &str) -> Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let conn = Connection::open(path).map_err(sqlite_err)?;
        // Durability first: every committed transaction survives OS crash.
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(sqlite_err)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(sqlite_err)?;

        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_schema()?;
        Ok(storage)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS records (
                rowid INTEGER PRIMARY KEY AUTOINCREMENT,
                record_type TEXT NOT NULL,
                record_id TEXT NOT NULL,
                body TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_records_type ON records(record_type);
            CREATE TABLE IF NOT EXISTS quarantine (
                rowid INTEGER PRIMARY KEY AUTOINCREMENT,
                reason TEXT NOT NULL,
                body TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS archive_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_sealed_rowid INTEGER NOT NULL
            );
            INSERT OR IGNORE INTO archive_state (id, last_sealed_rowid) VALUES (1, 0);
            CREATE TABLE IF NOT EXISTS archive_manifests (
                rowid INTEGER PRIMARY KEY AUTOINCREMENT,
                segment_id TEXT NOT NULL UNIQUE,
                sha256 TEXT NOT NULL,
                manifest_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )
        .map_err(sqlite_err)?;
        Ok(())
    }

    /// Highest rowid currently sealed into an archive segment.
    pub fn archive_last_sealed_rowid(&self) -> Result<i64> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let value: i64 = conn
            .query_row(
                "SELECT last_sealed_rowid FROM archive_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_err)?;
        Ok(value)
    }

    /// Highest rowid in the records table (0 when empty).
    pub fn max_rowid(&self) -> Result<i64> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let value: i64 = conn
            .query_row("SELECT COALESCE(MAX(rowid), 0) FROM records", [], |row| {
                row.get(0)
            })
            .map_err(sqlite_err)?;
        Ok(value)
    }

    /// Rows not yet covered by any sealed segment, in rowid order.
    pub fn rows_after(&self, rowid: i64) -> Result<Vec<(i64, String, String, String)>> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let mut stmt = conn
            .prepare(
                "SELECT rowid, record_type, record_id, body FROM records \
                 WHERE rowid > ?1 ORDER BY rowid",
            )
            .map_err(sqlite_err)?;
        let rows = stmt
            .query_map(params![rowid], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(sqlite_err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_err)
    }

    /// Stream rows in the half-open range (after_rowid, up_to_rowid] to
    /// `f`, one at a time, in rowid order. Memory-bounded alternative to
    /// `rows_after`; the upper bound pins the selection so rows appended
    /// mid-seal cannot escape the manifest boundary.
    pub fn each_row_after(
        &self,
        after_rowid: i64,
        up_to_rowid: i64,
        mut f: impl FnMut(i64, String, String, String),
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let mut stmt = conn
            .prepare(
                "SELECT rowid, record_type, record_id, body FROM records \
                 WHERE rowid > ?1 AND rowid <= ?2 ORDER BY rowid",
            )
            .map_err(sqlite_err)?;
        let rows = stmt
            .query_map(params![after_rowid, up_to_rowid], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(sqlite_err)?;
        for row in rows {
            let (rowid, record_type, record_id, body) = row.map_err(sqlite_err)?;
            f(rowid, record_type, record_id, body);
        }
        Ok(())
    }

    /// Durable identity of this evidence journal. Generated once per
    /// database; archive manifests bind segments to it so files from a
    /// different database can never be adopted here.
    pub fn archive_instance_id(&self) -> Result<String> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS archive_identity (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                instance_id TEXT NOT NULL
            );",
        )
        .map_err(sqlite_err)?;
        let existing: Option<String> = conn
            .query_row(
                "SELECT instance_id FROM archive_identity WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_err)?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO archive_identity (id, instance_id) VALUES (1, ?1)",
            params![id],
        )
        .map_err(sqlite_err)?;
        Ok(id)
    }

    /// Record a sealed segment: manifest row plus the new boundary, in one
    /// transaction. Called only after the segment and manifest files are
    /// durably published and verified (ADR 0014).
    pub fn archive_record_segment(
        &self,
        segment_id: &str,
        sha256: &str,
        manifest_json: &str,
        last_sealed_rowid: i64,
    ) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let tx = conn.transaction().map_err(sqlite_err)?;
        tx.execute(
            "INSERT INTO archive_manifests (segment_id, sha256, manifest_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![
                segment_id,
                sha256,
                manifest_json,
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .map_err(sqlite_err)?;
        tx.execute(
            "UPDATE archive_state SET last_sealed_rowid = ?1 WHERE id = 1",
            params![last_sealed_rowid],
        )
        .map_err(sqlite_err)?;
        tx.commit().map_err(sqlite_err)?;
        Ok(())
    }

    /// All recorded manifests in seal order.
    pub fn archive_manifests(&self) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let mut stmt = conn
            .prepare(
                "SELECT segment_id, sha256, manifest_json FROM archive_manifests ORDER BY rowid",
            )
            .map_err(sqlite_err)?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(sqlite_err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_err)
    }

    /// One recorded manifest by segment id.
    pub fn archive_manifest(&self, segment_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let mut stmt = conn
            .prepare("SELECT manifest_json FROM archive_manifests WHERE segment_id = ?1")
            .map_err(sqlite_err)?;
        let mut rows = stmt
            .query_map(params![segment_id], |row| row.get(0))
            .map_err(sqlite_err)?;
        match rows.next() {
            Some(value) => Ok(Some(value.map_err(sqlite_err)?)),
            None => Ok(None),
        }
    }

    /// Quarantine a raw record with an explicit reason.
    ///
    /// Used by legacy migration for rows that are not valid JSON or whose
    /// type is unknown; the body is preserved verbatim.
    pub fn quarantine_raw(&self, reason: &str, body: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        conn.execute(
            "INSERT INTO quarantine (reason, body) VALUES (?1, ?2)",
            params![reason, body],
        )
        .map_err(sqlite_err)?;
        Ok(())
    }

    /// Snapshot of every stored record key, for idempotent migration
    /// dedup. Bounded by the store's size; acceptable at Balebos volumes.
    pub fn existing_keys(&self) -> Result<std::collections::HashSet<(String, String, String)>> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let mut stmt = conn
            .prepare("SELECT record_type, record_id, body FROM records")
            .map_err(sqlite_err)?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(sqlite_err)?;
        rows.collect::<std::result::Result<std::collections::HashSet<_>, _>>()
            .map_err(sqlite_err)
    }

    /// Current per-type record counts and quarantine count, for
    /// post-migration verification.
    pub fn table_counts(&self) -> Result<(std::collections::BTreeMap<String, usize>, usize)> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let mut by_type = std::collections::BTreeMap::new();
        let mut stmt = conn
            .prepare("SELECT record_type, COUNT(*) FROM records GROUP BY record_type")
            .map_err(sqlite_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(sqlite_err)?;
        for row in rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_err)?
        {
            by_type.insert(row.0, row.1 as usize);
        }
        let quarantined: i64 = conn
            .query_row("SELECT COUNT(*) FROM quarantine", [], |row| row.get(0))
            .map_err(sqlite_err)?;
        Ok((by_type, quarantined as usize))
    }
}

/// Classify a raw JSON record: known type, unknown type (quarantine), or
/// reject entirely when it is not JSON at all.
enum Incoming {
    Known {
        item_type: ItemType,
        record_id: String,
        body: String,
    },
    Unknown {
        raw_type: String,
        body: String,
    },
}

fn classify(json: &str) -> Result<Incoming> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(crate::error::StorageError::from)?;
    let raw_type = value
        .get("item_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let record_id = value
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    match ItemType::from_type_str(&raw_type) {
        Some(item_type) => Ok(Incoming::Known {
            item_type,
            record_id,
            body: json.to_string(),
        }),
        None => Ok(Incoming::Unknown {
            raw_type,
            body: json.to_string(),
        }),
    }
}

impl StorageEngine for SqliteStorage {
    fn name(&self) -> String {
        "sqlite".to_string()
    }

    fn save_string(&self, json: &str) -> Result<()> {
        match classify(json)? {
            Incoming::Known {
                item_type,
                record_id,
                body,
            } => {
                let conn = self.conn.lock().map_err(|e| {
                    BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                        "Mutex poison: {e}"
                    )))
                })?;
                conn.execute(
                    "INSERT INTO records (record_type, record_id, body) VALUES (?1, ?2, ?3)",
                    params![item_type.as_str(), record_id, body],
                )
                .map_err(sqlite_err)?;
                Ok(())
            }
            Incoming::Unknown { raw_type, body } => {
                let conn = self.conn.lock().map_err(|e| {
                    BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                        "Mutex poison: {e}"
                    )))
                })?;
                conn.execute(
                    "INSERT INTO quarantine (reason, body) VALUES (?1, ?2)",
                    params![format!("unknown_type:{raw_type}"), body],
                )
                .map_err(sqlite_err)?;
                Ok(())
            }
        }
    }

    fn save_string_batch(&self, json_lines: &[String]) -> Result<()> {
        // Validate everything before writing: a batch with any invalid line
        // fails as a whole, so partial batches never commit silently.
        let mut incoming = Vec::with_capacity(json_lines.len());
        for line in json_lines {
            incoming.push(classify(line)?);
        }

        let mut conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;
        let tx = conn.transaction().map_err(sqlite_err)?;
        for record in incoming {
            match record {
                Incoming::Known {
                    item_type,
                    record_id,
                    body,
                } => {
                    tx.execute(
                        "INSERT INTO records (record_type, record_id, body) VALUES (?1, ?2, ?3)",
                        params![item_type.as_str(), record_id, body],
                    )
                    .map_err(sqlite_err)?;
                }
                Incoming::Unknown { raw_type, body } => {
                    tx.execute(
                        "INSERT INTO quarantine (reason, body) VALUES (?1, ?2)",
                        params![format!("unknown_type:{raw_type}"), body],
                    )
                    .map_err(sqlite_err)?;
                }
            }
        }
        tx.commit().map_err(sqlite_err)?;
        Ok(())
    }

    fn read_all_records(&self) -> Result<Records> {
        let conn = self.conn.lock().map_err(|e| {
            BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                "Mutex poison: {e}"
            )))
        })?;

        // Insertion order preserves the historical sequence of evidence.
        let mut stmt = conn
            .prepare("SELECT body FROM records ORDER BY rowid")
            .map_err(sqlite_err)?;
        let known: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(sqlite_err)?
            .collect::<std::result::Result<_, _>>()
            .map_err(sqlite_err)?;

        // Quarantined rows re-enter the reader so unknown types are reported
        // via Records::skipped — visible, never dropped.
        let mut stmt = conn
            .prepare("SELECT body FROM quarantine ORDER BY rowid")
            .map_err(sqlite_err)?;
        let quarantined: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(sqlite_err)?
            .collect::<std::result::Result<_, _>>()
            .map_err(sqlite_err)?;

        parse_records(known.into_iter().chain(quarantined))
    }

    fn flush(&self) -> Result<()> {
        // Every write commits inside its own call; nothing buffers.
        Ok(())
    }

    fn cleanup_old_records(&self, _max_age_days: u32) -> Result<()> {
        Ok(())
    }

    fn export_to_file(&self, path: &str, format: crate::tui::app::ExportFormat) -> Result<usize> {
        let records = self.read_all_records()?;
        let lines: Vec<String> = {
            // Re-export the raw bodies: storage keeps them verbatim, so
            // export is faithful to what was observed.
            let conn = self.conn.lock().map_err(|e| {
                BalebosError::Storage(crate::error::StorageError::Sqlite(format!(
                    "Mutex poison: {e}"
                )))
            })?;
            let mut stmt = conn
                .prepare("SELECT body FROM records ORDER BY rowid")
                .map_err(sqlite_err)?;
            let rows = stmt.query_map([], |row| row.get(0)).map_err(sqlite_err)?;
            rows.collect::<std::result::Result<_, _>>()
                .map_err(sqlite_err)?
        };

        match format {
            crate::tui::app::ExportFormat::Json => {
                std::fs::write(path, lines.join("\n"))?;
                Ok(lines.len())
            }
            crate::tui::app::ExportFormat::Csv => {
                let mut wtr = csv::Writer::from_path(path)
                    .map_err(|e| io_other(format!("CSV writer error: {e}")))?;
                wtr.write_record([
                    "item_type",
                    "id",
                    "name",
                    "discovered_at",
                    "source",
                    "original_input",
                ])
                .map_err(|e| io_other(format!("CSV write error: {e}")))?;
                for line in &lines {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                        let get = |key: &str| {
                            value
                                .get(key)
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string()
                        };
                        wtr.write_record([
                            get("item_type"),
                            get("id"),
                            get("name"),
                            get("discovered_at"),
                            get("source"),
                            get("original_input"),
                        ])
                        .map_err(|e| io_other(format!("CSV write error: {e}")))?;
                    }
                }
                wtr.flush()
                    .map_err(|e| io_other(format!("CSV flush error: {e}")))?;
                let _ = records;
                Ok(lines.len())
            }
            crate::tui::app::ExportFormat::Parquet => Err(io_other(
                "Parquet export requires the parquet-archive feature",
            )),
        }
    }

    fn import_from_file(&self, path: &str, format: crate::tui::app::ExportFormat) -> Result<usize> {
        match format {
            crate::tui::app::ExportFormat::Json => {
                let content = std::fs::read_to_string(path)?;
                let lines: Vec<String> = content
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(ToString::to_string)
                    .collect();
                // Strict batch: any invalid line rejects the whole import.
                self.save_string_batch(&lines)?;
                Ok(lines.len())
            }
            crate::tui::app::ExportFormat::Csv => {
                let mut rdr = csv::Reader::from_path(path)
                    .map_err(|e| io_other(format!("CSV reader error: {e}")))?;
                let headers = rdr
                    .headers()
                    .map_err(|e| io_other(format!("CSV header error: {e}")))?
                    .clone();
                let mut count = 0;
                let mut batch = Vec::new();
                for result in rdr.records() {
                    let record = result.map_err(|e| io_other(format!("CSV read error: {e}")))?;
                    let mut obj = serde_json::Map::new();
                    for (i, header) in headers.iter().enumerate() {
                        if let Some(value) = record.get(i) {
                            obj.insert(
                                header.to_string(),
                                serde_json::Value::String(value.to_string()),
                            );
                        }
                    }
                    batch.push(
                        serde_json::to_string(&serde_json::Value::Object(obj))
                            .map_err(crate::error::StorageError::from)?,
                    );
                    count += 1;
                }
                self.save_string_batch(&batch)?;
                Ok(count)
            }
            crate::tui::app::ExportFormat::Parquet => Err(io_other(
                "Parquet import requires the parquet-archive feature",
            )),
        }
    }
}

fn io_other(msg: impl Into<String>) -> BalebosError {
    BalebosError::Storage(crate::error::StorageError::Io(std::io::Error::other(
        msg.into(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageEngineExt;
    use std::sync::Arc;

    fn setup_test_db() -> String {
        let path = std::env::temp_dir().join(format!("balebos_sqlite_{}.db", uuid::Uuid::new_v4()));
        path.to_str().unwrap().to_string()
    }

    fn cleanup(path: &str) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{path}-wal"));
        let _ = std::fs::remove_file(format!("{path}-shm"));
    }

    #[test]
    fn save_and_read_roundtrip_without_flush() {
        let path = setup_test_db();
        let storage = Arc::new(SqliteStorage::new(&path).unwrap());

        let item = serde_json::json!({
            "item_type": "domain",
            "id": "1",
            "name": "example.com",
            "discovered_at": "2023-01-01T00:00:00Z",
            "source": "test"
        });
        storage.save(&item).unwrap();
        storage.save(&item).unwrap();

        let records = storage.read_all_records().unwrap();
        assert_eq!(records.items.len(), 2);

        cleanup(&path);
    }

    #[test]
    fn batch_is_all_or_nothing() {
        let path = setup_test_db();
        let storage = Arc::new(SqliteStorage::new(&path).unwrap());

        let valid = serde_json::to_string(&serde_json::json!({
            "item_type": "domain",
            "id": "1",
            "name": "example.com",
            "discovered_at": "2023-01-01T00:00:00Z",
            "source": "test"
        }))
        .unwrap();
        let result = storage.save_string_batch(&[valid, "not json".to_string()]);
        assert!(result.is_err());

        // The valid line must not have leaked into the store.
        let records = storage.read_all_records().unwrap();
        assert!(records.items.is_empty());

        cleanup(&path);
    }

    #[test]
    fn unknown_types_are_quarantined_and_reported() {
        let path = setup_test_db();
        let storage = Arc::new(SqliteStorage::new(&path).unwrap());

        let unknown = serde_json::json!({
            "item_type": "mystery_type",
            "id": "9",
            "name": "x.example.com"
        });
        storage.save(&unknown).unwrap();

        let records = storage.read_all_records().unwrap();
        assert!(records.items.is_empty());
        assert_eq!(records.skipped.len(), 1);
        assert_eq!(
            records.skipped[0].reason,
            crate::storage::reader::SkipReason::UnknownItemType("mystery_type".to_string())
        );

        cleanup(&path);
    }

    #[test]
    fn evidence_survives_reopen() {
        let path = setup_test_db();
        {
            let storage = Arc::new(SqliteStorage::new(&path).unwrap());
            let item = serde_json::json!({
                "item_type": "domain",
                "id": "1",
                "name": "example.com",
                "discovered_at": "2023-01-01T00:00:00Z",
                "source": "test"
            });
            storage.save(&item).unwrap();
            // No flush: durability must not depend on destructors.
        }

        let storage = SqliteStorage::new(&path).unwrap();
        let records = storage.read_all_records().unwrap();
        assert_eq!(records.items.len(), 1);
        assert_eq!(records.items[0].name, "example.com");

        cleanup(&path);
    }

    #[test]
    fn insertion_order_is_preserved() {
        let path = setup_test_db();
        let storage = Arc::new(SqliteStorage::new(&path).unwrap());

        for name in [
            "first.example.com",
            "second.example.com",
            "third.example.com",
        ] {
            storage
                .save(&serde_json::json!({
                    "item_type": "domain",
                    "id": name,
                    "name": name,
                    "discovered_at": "2023-01-01T00:00:00Z",
                    "source": "test"
                }))
                .unwrap();
        }

        let records = storage.read_all_records().unwrap();
        let names: Vec<&str> = records.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "first.example.com",
                "second.example.com",
                "third.example.com"
            ]
        );

        cleanup(&path);
    }
}
