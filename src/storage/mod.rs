pub mod parquet;
pub mod reader;

pub use self::parquet::ParquetStorage;
use crate::error::Result;
use serde::Serialize;

pub trait StorageEngine: Send + Sync {
    fn name(&self) -> String;
    fn save_string(&self, json: &str) -> Result<()>;
    fn save_string_batch(&self, json_lines: &[String]) -> Result<()>;
    fn read_all_records(&self) -> Result<crate::storage::reader::Records>;

    /// Flush any buffered records to persistent storage.
    fn flush(&self) -> Result<()> {
        Ok(())
    }

    /// Cleanup old records based on retention policy
    fn cleanup_old_records(&self, _max_age_days: u32) -> Result<()> {
        // Default implementation does nothing
        Ok(())
    }

    /// Export all data to a file in specified format
    fn export_to_file(&self, _path: &str, _format: crate::tui::app::ExportFormat) -> Result<usize> {
        // Default implementation - override in concrete storage
        Err(crate::error::BalebosError::Storage(
            crate::error::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Export not implemented for this storage engine",
            )),
        ))
    }

    /// Import data from a file
    fn import_from_file(
        &self,
        _path: &str,
        _format: crate::tui::app::ExportFormat,
    ) -> Result<usize> {
        // Default implementation - override in concrete storage
        Err(crate::error::BalebosError::Storage(
            crate::error::StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Import not implemented for this storage engine",
            )),
        ))
    }
}

pub trait StorageEngineExt {
    fn save<T: Serialize>(&self, data: &T) -> Result<()>;
    fn save_batch<T: Serialize>(&self, data: &[T]) -> Result<()>;
}

impl StorageEngineExt for dyn StorageEngine {
    fn save<T: Serialize>(&self, data: &T) -> Result<()> {
        let json = serde_json::to_string(data).map_err(crate::error::StorageError::from)?;
        self.save_string(&json)
    }

    fn save_batch<T: Serialize>(&self, data: &[T]) -> Result<()> {
        let mut lines = Vec::with_capacity(data.len());
        for item in data {
            lines.push(serde_json::to_string(item).map_err(crate::error::StorageError::from)?);
        }
        self.save_string_batch(&lines)
    }
}

// Implement for Arc<dyn StorageEngine> as well for convenience
impl<T: StorageEngine + ?Sized> StorageEngineExt for std::sync::Arc<T> {
    fn save<D: Serialize>(&self, data: &D) -> Result<()> {
        let json = serde_json::to_string(data).map_err(crate::error::StorageError::from)?;
        self.save_string(&json)
    }

    fn save_batch<D: Serialize>(&self, data: &[D]) -> Result<()> {
        let mut lines = Vec::with_capacity(data.len());
        for item in data {
            lines.push(serde_json::to_string(item).map_err(crate::error::StorageError::from)?);
        }
        self.save_string_batch(&lines)
    }
}
