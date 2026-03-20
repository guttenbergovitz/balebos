use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

// Our storage system for inventory items
// We're using a simple JSONL (JSON Lines) file format
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

    // Read all items from storage and deserialize them
    // Returns a Result with a Vec of deserialized items, or an error
    // Generic type T must implement Deserialize
    #[allow(dead_code)]
    pub fn read_all<T: for<'de> Deserialize<'de>>(&self) -> Result<Vec<T>, io::Error> {
        let path = Path::new(&self.filepath);

        // Check if file exists, return empty Vec if not
        if !path.exists() {
            return Ok(Vec::new());
        }

        // Open the file
        let file = File::open(&self.filepath)?;
        let reader = BufReader::new(file);
        let mut items = Vec::new();

        // Process each line
        for line in reader.lines() {
            match line {
                Ok(line_content) => {
                    // Skip empty lines
                    if line_content.trim().is_empty() {
                        continue;
                    }

                    // Try to deserialize the JSON line
                    match serde_json::from_str::<T>(&line_content) {
                        Ok(item) => items.push(item),
                        Err(e) => {
                            // Log the error but continue processing other lines
                            // This gives us better resilience - one bad record doesn't break everything
                            eprintln!("Error deserializing line: {}", e);
                            // We don't panic! - that would crash the entire program
                        }
                    }
                }
                Err(e) => {
                    // If we can't read a line, report it but continue
                    eprintln!("Error reading line: {}", e);
                }
            }
        }

        Ok(items)
    }
}
