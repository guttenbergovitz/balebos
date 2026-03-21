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
    pub fn save<T: Serialize>(&self, data: &T) -> Result<(), std::io::Error> {
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
        let json = serde_json::to_string(data)?;
        writeln!(writer, "{}", json)?;

        Ok(())
    }
}
