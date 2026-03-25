use crate::error::Result;
use duckdb::{params, Connection};
use std::sync::Mutex;

use super::StorageEngine;

pub struct DuckDbStorage {
    conn: Mutex<Connection>,
}

impl DuckDbStorage {
    pub fn new(filepath: &str) -> Result<Self> {
        let conn = Connection::open(filepath).map_err(|e| {
            crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
            ))
        })?;

        conn.execute_batch(
            "CREATE SEQUENCE IF NOT EXISTS event_id_seq;
             CREATE TABLE IF NOT EXISTS events (
                 id BIGINT DEFAULT nextval('event_id_seq'),
                 data JSON
             );",
        )
        .map_err(|e| {
            crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
            ))
        })?;

        // --- Auto-migration logic ---
        let count: i64 = conn
            .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
            .unwrap_or(0);

        if count == 0 {
            let jsonl_path = "data/inventory.jsonl";
            if std::path::Path::new(jsonl_path).exists() {
                println!("Migrating data from {} to DuckDB...", jsonl_path);
                // read_json_objects is better for JSONL where we want the whole object in one column
                match conn.execute(
                    &format!("INSERT INTO events (data) SELECT * FROM read_json_objects('{}')", jsonl_path),
                    [],
                ) {
                    Ok(n) => println!("Successfully migrated {} records.", n),
                    Err(e) => eprintln!("Migration warning: failed to import JSONL: {}", e),
                }
            }
        }
        // ----------------------------

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl StorageEngine for DuckDbStorage {
    fn name(&self) -> String {
        "DuckDB".to_string()
    }

    fn save_string(&self, json: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO events (data) VALUES (?::JSON)", params![json])
            .map_err(|e| {
                crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                ))
            })?;
        Ok(())
    }

    fn save_string_batch(&self, json_lines: &[String]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| {
            crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
            ))
        })?;
        {
            let mut stmt = tx
                .prepare("INSERT INTO events (data) VALUES (?::JSON)")
                .map_err(|e| {
                    crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                        std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                    ))
                })?;
            for json in json_lines {
                stmt.execute(params![json]).map_err(|e| {
                    crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                        std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                    ))
                })?;
            }
        }
        tx.commit().map_err(|e| {
            crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
            ))
        })?;
        Ok(())
    }

    fn read_all_records(&self) -> Result<crate::storage::reader::Records> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT data::VARCHAR FROM events ORDER BY id ASC")
            .map_err(|e| {
                crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                ))
            })?;

        let rows = stmt
            .query_map([], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })
            .map_err(|e| {
                crate::error::BalebosError::Storage(crate::error::StorageError::Io(
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                ))
            })?;

        let iter = rows.filter_map(|r| r.ok());
        crate::storage::reader::parse_records(iter)
    }
}
