use crate::error::Result;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;

use serde::Serialize;

// Our storage system for inventory items
// We're using a simple JSONL (JSON Lines) file format
#[derive(Clone)]
pub struct JsonlStorage {
    filepath: String,
}

impl JsonlStorage {
    pub fn new(filepath: &str) -> Self {
        Self {
            filepath: filepath.to_string(),
        }
    }

    // Getter dla ścieżki pliku
    pub fn get_filepath(&self) -> &str {
        &self.filepath
    }

    // Save an item to JSONL storage
    // This is append-only - we only add new records, never modify existing ones
    // Why append-only?
    // 1. It's simple (no need to manage updates or handle conflicts)
    // 2. We keep a history of all additions (useful for auditing)
    // 3. It's more resilient (partial writes don't corrupt existing data)
    // 4. It enables efficient writes (no need to rewrite the whole file)
    pub fn save<T: Serialize>(&self, data: &T) -> Result<()> {
        let path = Path::new(&self.filepath);

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Open file in append mode (create if doesn't exist)
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.filepath)?;

        let mut writer = BufWriter::new(file);

        // Serialize to JSON and write with newline
        let json = serde_json::to_string(data)
            .map_err(crate::error::StorageError::from)?;
        writeln!(writer, "{}", json)?;

        Ok(())
    }

    /// Save multiple items efficiently by opening the file only once
    pub fn save_batch<T: Serialize>(&self, data: &[T]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let path = Path::new(&self.filepath);

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.filepath)?;

        let mut writer = BufWriter::new(file);

        for item in data {
            let json = serde_json::to_string(item)
                .map_err(crate::error::StorageError::from)?;
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;
        Ok(())
    }
}
