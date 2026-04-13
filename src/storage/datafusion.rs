//! DataFusion-based storage implementation
//!
//! Uses Apache Parquet for columnar storage with DataFusion query engine.

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::{ArrayRef, Int32Array, Int64Array, StringArray, TimestampNanosecondArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Datelike, Utc};
use datafusion::prelude::*;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;

use crate::error::{BalebosError, Result};
use crate::inventory::ItemType;
use crate::storage::StorageEngine;

/// Configuration for DataFusionStorage
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub base_dir: PathBuf,
    pub max_rows_per_file: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from("data/parquet"),
            max_rows_per_file: 1_000_000,
        }
    }
}

/// DataFusion-based storage implementation
pub struct DataFusionStorage {
    #[allow(dead_code)]
    ctx: SessionContext,
    config: StorageConfig,
    schemas: HashMap<ItemType, SchemaRef>,
}

impl std::fmt::Debug for DataFusionStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataFusionStorage")
            .field("config", &self.config)
            .field("schemas", &self.schemas.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl DataFusionStorage {
    /// Create new DataFusionStorage
    pub async fn new(config: StorageConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.base_dir)?;
        
        let ctx = SessionContext::new();
        let schemas = Self::register_schemas();
        
        let storage = Self {
            ctx,
            config,
            schemas,
        };
        
        storage.register_tables().await?;
        
        Ok(storage)
    }
    
    /// Get schema for item type
    pub fn get_schema(&self, item_type: ItemType) -> Option<SchemaRef> {
        self.schemas.get(&item_type).cloned()
    }
    
    /// Register Arrow schemas for all record types
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
        
        let lifecycle_schema = Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("asset_name", DataType::Utf8, false),
            Field::new("to_state", DataType::Utf8, false),
            Field::new("created_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
        ]);
        schemas.insert(ItemType::LifecycleEvent, Arc::new(lifecycle_schema));
        
        let inventory_schema = Schema::new(vec![
            Field::new("item_type", DataType::Utf8, false),
            Field::new("id", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("discovered_at", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
            Field::new("source", DataType::Utf8, false),
        ]);
        schemas.insert(ItemType::Domain, Arc::new(inventory_schema.clone()));
        schemas.insert(ItemType::Subdomain, Arc::new(inventory_schema));
        
        schemas
    }
    
    /// Get partition path for record
    fn get_partition_path(&self, item_type: ItemType, timestamp: DateTime<Utc>) -> PathBuf {
        let base = self.config.base_dir.join(item_type.as_str());
        
        match item_type {
            ItemType::HttpCheck => {
                base.join(format!("year={}", timestamp.year()))
                    .join(format!("month={:02}", timestamp.month()))
                    .join(format!("day={:02}", timestamp.day()))
            }
            ItemType::Finding => {
                base.join(format!("year={}", timestamp.year()))
                    .join(format!("month={:02}", timestamp.month()))
            }
            _ => base,
        }
    }
    
    /// Write RecordBatch to Parquet with partitioning
    pub fn write_batch(
        &self,
        item_type: ItemType,
        batch: RecordBatch,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        if batch.num_rows() == 0 {
            return Ok(());
        }
        
        let partition_path = self.get_partition_path(item_type, timestamp);
        std::fs::create_dir_all(&partition_path)?;
        
        let filename = format!(
            "{}_{}.parquet",
            item_type.as_str(),
            timestamp.format("%Y%m%d_%H%M%S_%f")
        );
        let file_path = partition_path.join(filename);
        
        let file = File::create(&file_path)?;
        let props = WriterProperties::builder()
            .set_compression(parquet::basic::Compression::ZSTD(Default::default()))
            .set_dictionary_enabled(true)
            .build();
        
        let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))
            .map_err(|e| BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("Parquet error: {}", e))
            )))?;
        
        writer.write(&batch)
            .map_err(|e| BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("Parquet write error: {}", e))
            )))?;
        
        writer.close()
            .map_err(|e| BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("Parquet close error: {}", e))
            )))?;
        
        Ok(())
    }
    
    /// Register existing parquet files
    async fn register_tables(&self) -> Result<()> {
        // DataFusion tables will be registered lazily on first query
        Ok(())
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
        unimplemented!("Use write_batch for DataFusionStorage")
    }
    
    fn save_string_batch(&self, _json_lines: &[String]) -> Result<()> {
        unimplemented!("Use write_batch for DataFusionStorage")
    }
    
    fn read_all_records(&self) -> Result<crate::storage::reader::Records> {
        unimplemented!("Transition to DataFusion read path")
    }
    
    fn cleanup_old_records(&self, _max_age_days: u32) -> Result<()> {
        Ok(())
    }
}

/// Convert JSON values to Arrow RecordBatch
pub fn json_to_record_batch(
    json_values: Vec<serde_json::Value>,
    schema: &SchemaRef,
) -> Result<RecordBatch> {
    if json_values.is_empty() {
        return Ok(RecordBatch::new_empty(schema.clone()));
    }
    
    let mut columns: Vec<ArrayRef> = Vec::new();
    
    for field in schema.fields() {
        let column = build_column(field, &json_values)
            .map_err(|e| BalebosError::Storage(crate::error::StorageError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("Column build error: {}", e))
            )))?;
        columns.push(column);
    }
    
    RecordBatch::try_new(schema.clone(), columns)
        .map_err(|e| BalebosError::Storage(crate::error::StorageError::Io(
            std::io::Error::new(std::io::ErrorKind::Other, format!("Record batch error: {}", e))
        )))
}

/// Build a single column from JSON values
fn build_column(field: &Field, json_values: &[serde_json::Value]) -> std::result::Result<ArrayRef, String> {
    match field.data_type() {
        DataType::Utf8 => {
            let values: Vec<Option<String>> = json_values
                .iter()
                .map(|v| {
                    v.get(field.name())
                        .and_then(|val| val.as_str().map(|s| s.to_string()))
                })
                .collect();
            Ok(Arc::new(StringArray::from(values)) as ArrayRef)
        }
        DataType::Int32 => {
            let values: Vec<Option<i32>> = json_values
                .iter()
                .map(|v| {
                    v.get(field.name())
                        .and_then(|val| val.as_i64().map(|i| i as i32))
                })
                .collect();
            Ok(Arc::new(Int32Array::from(values)) as ArrayRef)
        }
        DataType::Int64 => {
            let values: Vec<Option<i64>> = json_values
                .iter()
                .map(|v| v.get(field.name()).and_then(|val| val.as_i64()))
                .collect();
            Ok(Arc::new(Int64Array::from(values)) as ArrayRef)
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            let values: Vec<Option<i64>> = json_values
                .iter()
                .map(|v| {
                    v.get(field.name())
                        .and_then(|val| val.as_str())
                        .and_then(|s| {
                            DateTime::parse_from_rfc3339(s)
                                .ok()
                                .map(|dt| dt.timestamp_nanos_opt().unwrap_or(0))
                        })
                })
                .collect();
            Ok(Arc::new(TimestampNanosecondArray::from(values)) as ArrayRef)
        }
        _ => {
            let values: Vec<Option<String>> = json_values
                .iter()
                .map(|v| {
                    v.get(field.name())
                        .map(|val| val.to_string().trim_matches('"').to_string())
                })
                .collect();
            Ok(Arc::new(StringArray::from(values)) as ArrayRef)
        }
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
    
    #[test]
    fn test_partition_path() {
        let config = StorageConfig::default();
        let storage = DataFusionStorage {
            ctx: SessionContext::new(),
            config,
            schemas: HashMap::new(),
        };
        
        let ts = Utc::now();
        let path = storage.get_partition_path(ItemType::HttpCheck, ts);
        
        assert!(path.to_string_lossy().contains("year="));
        assert!(path.to_string_lossy().contains("month="));
        assert!(path.to_string_lossy().contains("day="));
    }
    
    #[test]
    fn test_json_to_record_batch() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("target", DataType::Utf8, false),
            Field::new("status", DataType::Int32, true),
        ]));
        
        let json = vec![
            serde_json::json!({"target": "example.com", "status": 200}),
            serde_json::json!({"target": "test.com", "status": null}),
        ];
        
        let batch = json_to_record_batch(json, &schema).unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 2);
    }
}
