use crate::error::Result;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::StorageEngine;

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
}

impl StorageEngine for JsonlStorage {
    fn name(&self) -> String {
        "JSONL".to_string()
    }

    fn save_string(&self, json: &str) -> Result<()> {
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

        writeln!(writer, "{}", json)?;

        Ok(())
    }

    fn save_string_batch(&self, json_lines: &[String]) -> Result<()> {
        if json_lines.is_empty() {
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

        for json in json_lines {
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;
        Ok(())
    }

    fn read_all_records(&self) -> Result<crate::storage::reader::Records> {
        crate::storage::reader::read_records(&self.filepath)
    }
}
