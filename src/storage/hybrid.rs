//! Hybrid storage: JSONL for reads, Parquet for analytics
//!
//! During the transition to DataFusion, this storage ensures:
//! - All reads go through JSONL (backward compatible)
//! - All writes go to both JSONL and Parquet
//! - DataFusion queries work on Parquet files

use std::path::PathBuf;
use std::sync::Arc;

use crate::error::Result;
use crate::storage::datafusion::DataFusionStorage;
use crate::storage::jsonl::JsonlStorage;
use crate::storage::StorageEngine;

/// Hybrid storage that writes to both JSONL and Parquet
pub struct HybridStorage {
    jsonl: JsonlStorage,
    datafusion: DataFusionStorage,
}

impl HybridStorage {
    /// Create new hybrid storage
    pub async fn new(jsonl_path: &str, parquet_dir: &str) -> Result<Self> {
        let jsonl = JsonlStorage::new(jsonl_path);
        let config = crate::storage::datafusion::StorageConfig {
            base_dir: PathBuf::from(parquet_dir),
            ..Default::default()
        };
        let datafusion = DataFusionStorage::new(config).await?;
        
        Ok(Self { jsonl, datafusion })
    }
    
    /// Get reference to DataFusion storage for queries
    pub fn datafusion(&self) -> &DataFusionStorage {
        &self.datafusion
    }
    
    /// Get reference to JSONL storage
    pub fn jsonl(&self) -> &JsonlStorage {
        &self.jsonl
    }
    
    /// Run migration from JSONL to Parquet if needed
    pub async fn migrate_if_needed(&self) -> Result<Option<crate::storage::migration::MigrationReport>> {
        use crate::storage::migration::{migrate_jsonl_to_parquet, needs_migration, MigrationConfig};
        
        let parquet_dir = PathBuf::from("data/parquet");
        if needs_migration(&parquet_dir) {
            let config = MigrationConfig {
                source_path: PathBuf::from(self.jsonl.filepath()),
                target_storage: Arc::new(DataFusionStorage::new(
                    crate::storage::datafusion::StorageConfig {
                        base_dir: parquet_dir,
                        ..Default::default()
                    }
                ).await?),
                batch_size: 10000,
            };
            
            let report = migrate_jsonl_to_parquet(config).await?;
            
            // Save metadata
            let metadata = crate::storage::migration::MigrationMetadata {
                migrated_at: chrono::Utc::now(),
                source_file: self.jsonl.filepath().to_string(),
                total_records: report.total_records,
                version: env!("CARGO_PKG_VERSION").to_string(),
            };
            let _ = metadata.save(&PathBuf::from("data/parquet/_metadata/migration.json"));
            
            Ok(Some(report))
        } else {
            Ok(None)
        }
    }
}

impl StorageEngine for HybridStorage {
    fn name(&self) -> String {
        "hybrid".to_string()
    }
    
    fn save_string(&self, json: &str) -> Result<()> {
        // Write to JSONL first (source of truth)
        self.jsonl.save_string(json)?;
        
        // Also write to Parquet (best-effort during transition)
        // For now, we skip DataFusion write to avoid duplicating all records
        // In production, this would buffer and batch-write to Parquet
        let _ = self.datafusion.save_string(json);
        
        Ok(())
    }
    
    fn save_string_batch(&self, json_lines: &[String]) -> Result<()> {
        self.jsonl.save_string_batch(json_lines)?;
        let _ = self.datafusion.save_string_batch(json_lines);
        Ok(())
    }
    
    fn read_all_records(&self) -> Result<crate::storage::reader::Records> {
        self.jsonl.read_all_records()
    }
    
    fn cleanup_old_records(&self, max_age_days: u32) -> Result<()> {
        self.jsonl.cleanup_old_records(max_age_days)?;
        self.datafusion.cleanup_old_records(max_age_days)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_hybrid_storage_new() {
        let temp_jsonl = std::env::temp_dir().join(format!("test_hybrid_{}.jsonl", uuid::Uuid::new_v4()));
        let temp_parquet = std::env::temp_dir().join(format!("test_hybrid_parquet_{}", uuid::Uuid::new_v4()));
        
        let storage = HybridStorage::new(
            temp_jsonl.to_str().unwrap(),
            temp_parquet.to_str().unwrap(),
        ).await;
        
        assert!(storage.is_ok());
    }
}
