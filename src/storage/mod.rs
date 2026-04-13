pub mod datafusion;
// duckdb temporarily disabled
// #[cfg(feature = "duckdb")]
// pub mod duckdb;
pub mod jsonl;
pub mod reader;

pub use self::jsonl::JsonlStorage;
use crate::error::Result;
use serde::Serialize;

pub trait StorageEngine: Send + Sync {
    fn name(&self) -> String;
    fn save_string(&self, json: &str) -> Result<()>;
    fn save_string_batch(&self, json_lines: &[String]) -> Result<()>;
    fn read_all_records(&self) -> Result<crate::storage::reader::Records>;

    /// Cleanup old records based on retention policy
    fn cleanup_old_records(&self, max_age_days: u32) -> Result<()> {
        // Default implementation does nothing
        Ok(())
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
