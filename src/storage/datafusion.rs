//! DataFusion-based storage implementation
//!
//! Uses Apache Parquet for columnar storage with DataFusion query engine.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow::record_batch::RecordBatch;
use datafusion::prelude::*;

use crate::error::{BalebosError, Result};
use crate::inventory::ItemType;
use crate::storage::StorageEngine;

/// Configuration for DataFusionStorage
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub base_dir: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from("data/parquet"),
        }
    }
}

/// DataFusion-based storage implementation
pub struct DataFusionStorage {
    #[allow(dead_code)]
    ctx: SessionContext,
    #[allow(dead_code)]
    config: StorageConfig,
    schemas: HashMap<ItemType, SchemaRef>,
}

impl DataFusionStorage {
    /// Create new DataFusionStorage
    pub async fn new(config: StorageConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.base_dir)?;
        
        let ctx = SessionContext::new();
        let schemas = Self::register_schemas();
        
        Ok(Self {
            ctx,
            config,
            schemas,
        })
    }
    
    fn register_schemas() -> HashMap<ItemType, SchemaRef> {
        let mut schemas = HashMap::new();
        
        let http_schema = Schema::new(vec![
            Field::new("target", DataType::Utf8, false),
            Field::new("status", DataType::Int32, true),
            Field::new("response_time_ms", DataType::Int32, false),
            Field::new("checked_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
        ]);
        schemas.insert(ItemType::HttpCheck, Arc::new(http_schema));
        
        let finding_schema = Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("severity", DataType::Utf8, false),
            Field::new("target", DataType::Utf8, false),
            Field::new("created_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
        ]);
        schemas.insert(ItemType::Finding, Arc::new(finding_schema));
        
        schemas
    }
    
    /// Execute SQL query
    pub async fn query(&self, sql: &str) -> Result<Vec<RecordBatch>> {
        let df = self.ctx.sql(sql).await
            .map_err(|e| BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("DataFusion error: {}", e))
            )))?;
        let batches = df.collect().await
            .map_err(|e| BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("DataFusion error: {}", e))
            )))?;
        Ok(batches)
    }
}

impl StorageEngine for DataFusionStorage {
    fn name(&self) -> String {
        "datafusion".to_string()
    }
    
    fn save_string(&self, _json: &str) -> Result<()> {
        unimplemented!("Use save_batch for DataFusionStorage")
    }
    
    fn save_string_batch(&self, _json_lines: &[String]) -> Result<()> {
        unimplemented!("Use save_batch for DataFusionStorage")
    }
    
    fn read_all_records(&self) -> Result<crate::storage::reader::Records> {
        unimplemented!("Transition to DataFusion read path")
    }
    
    fn cleanup_old_records(&self, _max_age_days: u32) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_datafusion_storage_new() {
        let config = StorageConfig {
            base_dir: std::env::temp_dir().join(format!("test_balebos_{}", uuid::Uuid::new_v4())),
            ..Default::default()
        };
        
        let storage = DataFusionStorage::new(config).await;
        assert!(storage.is_ok());
    }
}
