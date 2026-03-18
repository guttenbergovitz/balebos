use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;

use serde::Serialize;

pub struct JsonlStorage {
    filepath: String,
}

impl JsonlStorage {
    pub fn new(filepath: &str) -> Self {
        Self {
            filepath: filepath.to_string(),
        }
    }

    pub fn save<T: Serialize>(&self, data: &T) -> Result<(), std::io::Error> {
        let path = Path::new(&self.filepath);

        // Create parent directory if it doesn't exist
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

        let json = serde_json::to_string(data)?;
        writeln!(writer, "{}", json)?;

        Ok(())
    }
}
