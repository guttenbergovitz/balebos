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
    
    pub fn filepath(&self) -> &str {
        &self.filepath
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

    fn cleanup_old_records(&self, max_age_days: u32) -> Result<()> {
        use crate::storage::reader::parse_records;
        use chrono::{DateTime, Duration, Utc};
        use std::fs::{self, File};
        use std::io::{BufRead, BufReader, Write};

        let path = Path::new(&self.filepath);

        if !path.exists() {
            return Ok(());
        }

        // Read all lines and parse records
        let file = File::open(&self.filepath)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map_while(|r| r.ok()).collect();

        if lines.is_empty() {
            return Ok(());
        }

        // Parse records to check timestamps
        let records = parse_records(lines.into_iter())?;

        // Calculate cutoff date
        let cutoff_date = Utc::now() - Duration::days(max_age_days as i64);

        // Filter findings older than cutoff
        let old_findings: Vec<String> = records
            .findings
            .iter()
            .filter(|f| f.created_at < cutoff_date)
            .map(|f| f.id.clone())
            .collect();

        if old_findings.is_empty() {
            return Ok(());
        }

        // Create backup file
        let backup_path = format!("{}.backup", self.filepath);
        fs::copy(&self.filepath, &backup_path)?;

        // Read original file again and filter out old findings
        let file = File::open(&self.filepath)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map_while(|r| r.ok()).collect();

        // Create new file
        let new_path = format!("{}.new", self.filepath);
        let mut new_file = File::create(&new_path)?;

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            // Parse line to check if it's an old finding
            if let Ok(header) = serde_json::from_str::<crate::storage::reader::RecordHeader>(&line)
            {
                if header.item_type == crate::inventory::ItemType::Finding {
                    if let Ok(finding) = serde_json::from_str::<crate::app::Finding>(&line) {
                        if old_findings.contains(&finding.id) {
                            // Skip this old finding
                            continue;
                        }
                    }
                }
            }

            writeln!(new_file, "{}", line)?;
        }

        new_file.flush()?;

        // Replace original file with new one
        fs::rename(&new_path, &self.filepath)?;

        // Keep backup for safety
        println!(
            "Cleaned up {} old findings. Backup saved to: {}",
            old_findings.len(),
            backup_path
        );

        Ok(())
    }
}
