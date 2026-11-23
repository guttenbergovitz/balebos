//! Evidence archive: sealed Parquet segments with SHA-256 manifests
//! (ADR 0013, ADR 0014). Feature `parquet-archive`.
//!
//! Sealing is deterministic (a rowid prefix boundary), publication is
//! crash-safe (tmp files, fsync, verification, rename), and SQLite state
//! is updated only after the segment and manifest are durably published.
//! Re-runs are idempotent: a crash after rename but before state update
//! is recovered by verifying and adopting the already-published files.
//!
//! Archival never deletes or marks source rows in SQLite.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use chrono::{DateTime, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{BalebosError, Result};
use crate::storage::sqlite::SqliteStorage;
use crate::storage::StorageEngine;

/// Manifest schema version; bump on incompatible manifest changes.
pub const ARCHIVE_SCHEMA_VERSION: u32 = 1;

fn storage_err(msg: impl Into<String>) -> BalebosError {
    BalebosError::Storage(crate::error::StorageError::Io(std::io::Error::other(
        msg.into(),
    )))
}

/// The deterministic selection sealed into a segment: an inclusive rowid
/// prefix of the operational store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordBoundary {
    pub from_rowid: i64,
    pub to_rowid: i64,
}

/// Versioned manifest describing one sealed segment (ADR 0014).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub archive_id: String,
    pub schema_version: u32,
    pub segment_id: String,
    pub created_at: DateTime<Utc>,
    pub boundary: RecordBoundary,
    pub record_count: usize,
    pub record_types: BTreeMap<String, usize>,
    pub sha256: String,
    pub previous_segment_sha256: Option<String>,
    pub source: String,
}

/// Result of a seal run.
#[derive(Debug, Default)]
pub struct SealReport {
    pub segment_id: Option<String>,
    pub record_count: usize,
    pub boundary: Option<RecordBoundary>,
    pub adopted_existing: bool,
}

/// Result of a verification pass.
#[derive(Debug, Default)]
pub struct VerifyReport {
    pub segments_checked: usize,
    pub problems: Vec<String>,
}

impl VerifyReport {
    pub fn is_ok(&self) -> bool {
        self.problems.is_empty()
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Deterministic segment identity (review P0): derived from the
/// selection boundary, never random. Records are append-only and never
/// deleted, so a rowid range identifies its content; re-running a failed
/// attempt addresses the same files, which makes publish-then-crash
/// recovery work instead of orphaning segments. One archive directory
/// belongs to one database.
fn deterministic_segment_id(boundary: &RecordBoundary) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"balebos-segment-v1|");
    hasher.update(boundary.from_rowid.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(boundary.to_rowid.to_string().as_bytes());
    format!("{:x}", hasher.finalize())[..32].to_string()
}

/// Write-through SHA-256: hashes exactly the bytes handed to the writer
/// so the segment digest is known without re-reading the file.
struct HashingWriter<W: Write> {
    inner: W,
    hasher: Sha256,
}

impl<W: Write> HashingWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
        }
    }

    fn finish(self) -> String {
        format!("{:x}", self.hasher.finalize())
    }
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hasher.update(buf);
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Chunk size for streaming segment writes: bounded memory regardless of
/// selection size.
const SEGMENT_CHUNK: usize = 8192;

fn segment_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("rowid", DataType::Int64, false),
        Field::new("record_type", DataType::Utf8, false),
        Field::new("record_id", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
    ]))
}

fn segments_dir(base: &Path) -> PathBuf {
    base.join("segments")
}

fn incoming_dir(base: &Path) -> PathBuf {
    base.join("incoming")
}

fn segment_path(base: &Path, segment_id: &str) -> PathBuf {
    segments_dir(base).join(format!("{segment_id}.parquet"))
}

fn manifest_path(base: &Path, segment_id: &str) -> PathBuf {
    segments_dir(base).join(format!("{segment_id}.manifest.json"))
}

/// Rows of a sealed segment, in file order.
fn read_segment(path: &Path) -> Result<Vec<(i64, String, String, String)>> {
    let file = std::fs::File::open(path)
        .map_err(|e| storage_err(format!("cannot open segment {}: {e}", path.display())))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| storage_err(format!("cannot read segment {}: {e}", path.display())))?;
    let mut rows = Vec::new();
    for batch in builder.build().map_err(|e| storage_err(e.to_string()))? {
        let batch = batch.map_err(|e| storage_err(e.to_string()))?;
        let rowids = batch
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| storage_err("segment column 'rowid' has unexpected type"))?;
        let types = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| storage_err("segment column 'record_type' has unexpected type"))?;
        let ids = batch
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| storage_err("segment column 'record_id' has unexpected type"))?;
        let bodies = batch
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| storage_err("segment column 'body' has unexpected type"))?;
        for i in 0..batch.num_rows() {
            rows.push((
                rowids.value(i),
                types.value(i).to_string(),
                ids.value(i).to_string(),
                bodies.value(i).to_string(),
            ));
        }
    }
    Ok(rows)
}

/// Write and fsync a file at a temporary path; the caller renames it.
fn write_tmp_fsync(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{name}.tmp"));
    {
        let mut file = std::fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
    }
    Ok(tmp)
}

/// Seal every unarchived observation into a new immutable segment.
///
/// Deterministic selection (rowid prefix), crash-safe publication, and
/// idempotent re-runs. Source rows in SQLite are never modified.
pub fn seal_pending(storage: &SqliteStorage, archive_base: &str) -> Result<SealReport> {
    let base = Path::new(archive_base);
    std::fs::create_dir_all(segments_dir(base))?;
    std::fs::create_dir_all(incoming_dir(base))?;

    // Stale tmp files are inert by definition; clear them so a failed
    // previous run cannot accumulate garbage.
    for entry in std::fs::read_dir(incoming_dir(base))? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "tmp") {
            let _ = std::fs::remove_file(path);
        }
    }

    let last_sealed = storage.archive_last_sealed_rowid()?;
    let max_rowid = storage.max_rowid()?;
    if max_rowid <= last_sealed {
        return Ok(SealReport::default());
    }

    // The boundary is known before reading a single row: records are
    // append-only and never deleted, so the range identifies the content.
    let boundary = RecordBoundary {
        from_rowid: last_sealed + 1,
        to_rowid: max_rowid,
    };

    // Deterministic identity (review P0): the same selection always
    // addresses the same files. A crash after publication but before the
    // SQLite state update is recovered by the adoption path below —
    // nothing is orphaned by a fresh random id.
    let segment_id = deterministic_segment_id(&boundary);
    let final_segment_path = segment_path(base, &segment_id);
    let final_manifest_path = manifest_path(base, &segment_id);

    // Crash-after-rename recovery: both files exist but SQLite state was
    // never recorded. Verify the on-disk manifest against the bytes and
    // adopt it; never write a second segment for the same selection.
    if final_segment_path.exists() && final_manifest_path.exists() {
        let manifest_json = std::fs::read_to_string(&final_manifest_path).map_err(|e| {
            storage_err(format!(
                "cannot read manifest {}: {e}",
                final_manifest_path.display()
            ))
        })?;
        let on_disk: ArchiveManifest =
            serde_json::from_str(&manifest_json).map_err(crate::error::StorageError::from)?;
        let bytes = std::fs::read(&final_segment_path).map_err(|e| {
            storage_err(format!(
                "cannot read segment {}: {e}",
                final_segment_path.display()
            ))
        })?;
        if sha256_hex(&bytes) != on_disk.sha256 {
            return Err(storage_err(format!(
                "adoption failed: segment {} hash does not match its manifest",
                on_disk.segment_id
            )));
        }
        if count_segment_rows(&final_segment_path)? != on_disk.record_count {
            return Err(storage_err(format!(
                "adoption failed: segment {} row count does not match its manifest",
                on_disk.segment_id
            )));
        }
        storage.archive_record_segment(
            &on_disk.segment_id,
            &on_disk.sha256,
            &manifest_json,
            on_disk.boundary.to_rowid,
        )?;
        return Ok(SealReport {
            segment_id: Some(on_disk.segment_id),
            record_count: on_disk.record_count,
            boundary: Some(on_disk.boundary),
            adopted_existing: true,
        });
    }

    let previous = storage
        .archive_manifests()?
        .last()
        .map(|(_, sha256, _)| sha256.clone());

    // Write the segment streaming in fixed-size chunks: memory stays
    // bounded regardless of the selection size (review P1). The SHA-256
    // is computed by the write-through hasher, so the file is never
    // re-read for hashing.
    let segment_tmp = incoming_dir(base).join(format!("{segment_id}.parquet.tmp"));
    let schema = segment_schema();
    let mut record_count = 0usize;
    let mut record_types: BTreeMap<String, usize> = BTreeMap::new();
    let segment_hash: String = {
        let file = std::fs::File::create(&segment_tmp)?;
        let mut hash_writer = HashingWriter::new(&file);
        {
            let mut writer = ArrowWriter::try_new(&mut hash_writer, schema.clone(), None)
                .map_err(|e| storage_err(e.to_string()))?;

            let mut chunk_rowids: Vec<i64> = Vec::with_capacity(SEGMENT_CHUNK);
            let mut chunk_types: Vec<String> = Vec::with_capacity(SEGMENT_CHUNK);
            let mut chunk_ids: Vec<String> = Vec::with_capacity(SEGMENT_CHUNK);
            let mut chunk_bodies: Vec<String> = Vec::with_capacity(SEGMENT_CHUNK);

            let flush_chunk = |writer: &mut ArrowWriter<&mut HashingWriter<&std::fs::File>>,
                               chunk_rowids: &mut Vec<i64>,
                               chunk_types: &mut Vec<String>,
                               chunk_ids: &mut Vec<String>,
                               chunk_bodies: &mut Vec<String>|
             -> Result<()> {
                if chunk_rowids.is_empty() {
                    return Ok(());
                }
                let batch = RecordBatch::try_new(
                    schema.clone(),
                    vec![
                        Arc::new(Int64Array::from(std::mem::take(chunk_rowids))),
                        Arc::new(StringArray::from(std::mem::take(chunk_types))),
                        Arc::new(StringArray::from(std::mem::take(chunk_ids))),
                        Arc::new(StringArray::from(std::mem::take(chunk_bodies))),
                    ],
                )
                .map_err(|e| storage_err(format!("building segment batch: {e}")))?;
                writer
                    .write(&batch)
                    .map_err(|e| storage_err(format!("writing segment: {e}")))?;
                Ok(())
            };

            let mut stream_error: Option<BalebosError> = None;
            storage.each_row_after(last_sealed, |rowid, record_type, record_id, body| {
                if stream_error.is_some() {
                    return;
                }
                chunk_rowids.push(rowid);
                chunk_types.push(record_type.clone());
                chunk_ids.push(record_id);
                chunk_bodies.push(body);
                *record_types.entry(record_type).or_insert(0) += 1;
                record_count += 1;
                if chunk_rowids.len() >= SEGMENT_CHUNK {
                    if let Err(e) = flush_chunk(
                        &mut writer,
                        &mut chunk_rowids,
                        &mut chunk_types,
                        &mut chunk_ids,
                        &mut chunk_bodies,
                    ) {
                        stream_error = Some(e);
                    }
                }
            })?;
            if let Some(e) = stream_error {
                return Err(e);
            }
            flush_chunk(
                &mut writer,
                &mut chunk_rowids,
                &mut chunk_types,
                &mut chunk_ids,
                &mut chunk_bodies,
            )?;
            writer
                .close()
                .map_err(|e| storage_err(format!("finalising segment: {e}")))?;
        }
        // Durability before publication (review P1): fsync the segment
        // file itself; the directory fsync after rename remains
        // best-effort below.
        file.sync_all()?;
        hash_writer.finish()
    };

    if record_count == 0 {
        let _ = std::fs::remove_file(&segment_tmp);
        return Ok(SealReport::default());
    }

    let manifest = ArchiveManifest {
        archive_id: uuid::Uuid::new_v4().to_string(),
        schema_version: ARCHIVE_SCHEMA_VERSION,
        segment_id: segment_id.clone(),
        created_at: Utc::now(),
        boundary: boundary.clone(),
        record_count,
        record_types,
        sha256: segment_hash.clone(),
        previous_segment_sha256: previous,
        source: "balebos-sqlite".to_string(),
    };
    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(crate::error::StorageError::from)?;
    let manifest_tmp = write_tmp_fsync(
        &incoming_dir(base),
        &format!("{segment_id}.manifest.json"),
        manifest_json.as_bytes(),
    )?;

    // Verify before publishing: the file must decode and contain exactly
    // the rows we streamed.
    if count_segment_rows(&segment_tmp)? != record_count {
        let _ = std::fs::remove_file(&segment_tmp);
        let _ = std::fs::remove_file(&manifest_tmp);
        return Err(storage_err("segment row count mismatch before publication"));
    }

    // Publish atomically; only now may SQLite state change.
    std::fs::rename(&segment_tmp, &final_segment_path)?;
    std::fs::rename(&manifest_tmp, &final_manifest_path)?;
    if let Some(parent) = final_segment_path.parent().and_then(|p| p.parent()) {
        if let Ok(dir) = std::fs::File::open(parent) {
            // Best effort: not all filesystems support directory fsync.
            let _ = dir.sync_all();
        }
    }

    storage.archive_record_segment(
        &segment_id,
        &segment_hash,
        &manifest_json,
        boundary.to_rowid,
    )?;

    Ok(SealReport {
        segment_id: Some(segment_id),
        record_count,
        boundary: Some(boundary),
        adopted_existing: false,
    })
}

/// Count rows of a segment by streaming, without materialising them.
fn count_segment_rows(path: &Path) -> Result<usize> {
    let file = std::fs::File::open(path)
        .map_err(|e| storage_err(format!("cannot open segment {}: {e}", path.display())))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| storage_err(format!("cannot read segment {}: {e}", path.display())))?;
    let mut total = 0usize;
    for batch in builder
        .build()
        .map_err(|e| storage_err(format!("cannot read segment {}: {e}", path.display())))?
    {
        let batch = batch.map_err(|e| storage_err(e.to_string()))?;
        total += batch.num_rows();
    }
    Ok(total)
}

/// Verify one segment file pair against its manifest.
fn verify_segment_files(
    manifest: &ArchiveManifest,
    segment: &Path,
    manifest_file: &Path,
) -> Result<()> {
    let bytes = std::fs::read(segment)
        .map_err(|e| storage_err(format!("segment {} unreadable: {e}", segment.display())))?;
    if sha256_hex(&bytes) != manifest.sha256 {
        return Err(storage_err(format!(
            "segment {} hash does not match manifest",
            manifest.segment_id
        )));
    }
    let recorded: ArchiveManifest = serde_json::from_str(
        &std::fs::read_to_string(manifest_file)
            .map_err(|e| storage_err(format!("manifest unreadable: {e}")))?,
    )
    .map_err(crate::error::StorageError::from)?;
    if recorded != *manifest {
        return Err(storage_err(format!(
            "manifest file for {} does not match the recorded manifest",
            manifest.segment_id
        )));
    }
    if count_segment_rows(segment)? != manifest.record_count {
        return Err(storage_err(format!(
            "segment {} row count does not match manifest",
            manifest.segment_id
        )));
    }
    Ok(())
}

/// Verify every recorded segment: hashes, counts, chain continuity,
/// boundary gaps and duplicates (ADR 0014 stage 6).
pub fn verify_archive(storage: &SqliteStorage, archive_base: &str) -> Result<VerifyReport> {
    let base = Path::new(archive_base);
    let mut report = VerifyReport::default();

    let manifests = storage.archive_manifests()?;
    let mut previous_hash: Option<String> = None;
    let mut previous_to: Option<i64> = None;
    let mut seen_segments = std::collections::HashSet::new();

    for (_, recorded_sha256, manifest_json) in &manifests {
        report.segments_checked += 1;
        let manifest: ArchiveManifest = match serde_json::from_str(manifest_json) {
            Ok(m) => m,
            Err(e) => {
                report
                    .problems
                    .push(format!("manifest in store is not valid JSON: {e}"));
                continue;
            }
        };

        if !seen_segments.insert(manifest.segment_id.clone()) {
            report
                .problems
                .push(format!("duplicate segment id {}", manifest.segment_id));
        }
        if manifest.schema_version != ARCHIVE_SCHEMA_VERSION {
            report.problems.push(format!(
                "segment {} has unsupported schema version {}",
                manifest.segment_id, manifest.schema_version
            ));
        }
        if recorded_sha256.as_str() != manifest.sha256 {
            report.problems.push(format!(
                "stored sha256 for {} does not match its manifest",
                manifest.segment_id
            ));
        }
        match &previous_hash {
            Some(prev) if manifest.previous_segment_sha256.as_ref() != Some(prev) => report
                .problems
                .push(format!("chain break at segment {}", manifest.segment_id)),
            _ => {}
        }
        match previous_to {
            Some(to) if manifest.boundary.from_rowid != to + 1 => report.problems.push(format!(
                "boundary gap before segment {} (expected from {})",
                manifest.segment_id,
                to + 1
            )),
            _ => {}
        }
        if manifest.record_count != manifest.record_types.values().sum::<usize>() {
            report.problems.push(format!(
                "segment {} record_count does not match per-type counts",
                manifest.segment_id
            ));
        }

        let segment = segment_path(base, &manifest.segment_id);
        let manifest_file = manifest_path(base, &manifest.segment_id);
        if !segment.exists() || !manifest_file.exists() {
            report.problems.push(format!(
                "segment {} missing files on disk",
                manifest.segment_id
            ));
        } else if let Err(e) = verify_segment_files(&manifest, &segment, &manifest_file) {
            report.problems.push(e.to_string());
        }

        previous_hash = Some(manifest.sha256.clone());
        previous_to = Some(manifest.boundary.to_rowid);
    }

    // Inert leftovers are reported, never treated as corruption.
    if incoming_dir(base).exists() {
        for entry in std::fs::read_dir(incoming_dir(base))? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "tmp") {
                report
                    .problems
                    .push(format!("stale tmp file (inert): {}", path.display()));
            }
        }
    }

    Ok(report)
}

/// Re-import a sealed segment into the operational store (disaster
/// recovery). Idempotent via the same body dedup as migration.
pub fn restore_segment(
    storage: &SqliteStorage,
    archive_base: &str,
    segment_id: &str,
) -> Result<usize> {
    let manifest_json = storage
        .archive_manifest(segment_id)?
        .ok_or_else(|| storage_err(format!("unknown segment: {segment_id}")))?;
    let manifest: ArchiveManifest =
        serde_json::from_str(&manifest_json).map_err(crate::error::StorageError::from)?;

    let rows = read_segment(&segment_path(Path::new(archive_base), segment_id))?;
    if rows.len() != manifest.record_count {
        return Err(storage_err(format!(
            "segment {segment_id} row count {} does not match manifest {}",
            rows.len(),
            manifest.record_count
        )));
    }

    let bodies: Vec<String> = rows.into_iter().map(|r| r.3).collect();
    storage.save_string_batch(&bodies)?;
    Ok(bodies.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (String, String, String) {
        let base = std::env::temp_dir().join(format!("balebos_archive_{}", uuid::Uuid::new_v4()));
        let db = base.join("balebos.db");
        let archive = base.join("archive");
        std::fs::create_dir_all(&base).unwrap();
        (
            base.to_str().unwrap().to_string(),
            db.to_str().unwrap().to_string(),
            archive.to_str().unwrap().to_string(),
        )
    }

    fn save_domain(storage: &SqliteStorage, name: &str) {
        let json = serde_json::to_string(&serde_json::json!({
            "item_type": "domain",
            "id": name,
            "name": name,
            "discovered_at": "2023-01-01T00:00:00Z",
            "source": "test"
        }))
        .unwrap();
        storage.save_string(&json).unwrap();
    }

    fn cleanup(base: &str) {
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn seal_publishes_verifiable_segment_and_chain_continues() {
        let (base, db, archive) = setup();
        let storage = SqliteStorage::new(&db).unwrap();
        save_domain(&storage, "first.example.com");

        let first = seal_pending(&storage, &archive).unwrap();
        assert_eq!(first.record_count, 1);
        assert!(first.segment_id.is_some());

        save_domain(&storage, "second.example.com");
        let second = seal_pending(&storage, &archive).unwrap();
        assert_eq!(second.record_count, 1);
        assert!(!second.adopted_existing);

        let report = verify_archive(&storage, &archive).unwrap();
        assert!(report.is_ok(), "problems: {:?}", report.problems);
        assert_eq!(report.segments_checked, 2);

        // Chain: second manifest points at the first segment hash.
        let manifests = storage.archive_manifests().unwrap();
        let first_manifest: ArchiveManifest = serde_json::from_str(&manifests[0].2).unwrap();
        let second_manifest: ArchiveManifest = serde_json::from_str(&manifests[1].2).unwrap();
        assert_eq!(
            second_manifest.previous_segment_sha256.as_deref(),
            Some(first_manifest.sha256.as_str())
        );
        assert_eq!(
            second_manifest.boundary.from_rowid,
            first_manifest.boundary.to_rowid + 1
        );

        // Nothing left to seal.
        let third = seal_pending(&storage, &archive).unwrap();
        assert_eq!(third.record_count, 0);

        cleanup(&base);
    }

    #[test]
    fn tampered_segment_fails_verification() {
        let (base, db, archive) = setup();
        let storage = SqliteStorage::new(&db).unwrap();
        save_domain(&storage, "example.com");
        let report = seal_pending(&storage, &archive).unwrap();
        let segment_id = report.segment_id.unwrap();

        let path = segment_path(Path::new(&archive), &segment_id);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 0xff;
        std::fs::write(&path, bytes).unwrap();

        let report = verify_archive(&storage, &archive).unwrap();
        assert!(!report.is_ok());

        cleanup(&base);
    }

    #[test]
    fn restore_reimports_segment_rows() {
        let (base, db, archive) = setup();
        let storage = SqliteStorage::new(&db).unwrap();
        save_domain(&storage, "example.com");
        let report = seal_pending(&storage, &archive).unwrap();
        let segment_id = report.segment_id.unwrap();

        // Disaster recovery: fresh store, restore from the segment.
        let db2 = format!("{db}.restored");
        let fresh = SqliteStorage::new(&db2).unwrap();
        // The manifest lives in the original store; copy it across.
        let manifest_json = storage.archive_manifest(&segment_id).unwrap().unwrap();
        fresh
            .archive_record_segment(&segment_id, "x", &manifest_json, 1)
            .unwrap();

        let restored = restore_segment(&fresh, &archive, &segment_id).unwrap();
        assert_eq!(restored, 1);
        let records = fresh.read_all_records().unwrap();
        assert_eq!(records.items.len(), 1);
        assert_eq!(records.items[0].name, "example.com");

        let _ = std::fs::remove_file(db2);
        cleanup(&base);
    }

    #[test]
    fn manifest_records_exact_boundary_and_counts() {
        let (base, db, archive) = setup();
        let storage = SqliteStorage::new(&db).unwrap();
        save_domain(&storage, "a.example.com");
        save_domain(&storage, "b.example.com");

        seal_pending(&storage, &archive).unwrap();
        let manifests = storage.archive_manifests().unwrap();
        let manifest: ArchiveManifest = serde_json::from_str(&manifests[0].2).unwrap();

        assert_eq!(manifest.schema_version, ARCHIVE_SCHEMA_VERSION);
        assert_eq!(manifest.record_count, 2);
        assert_eq!(manifest.boundary.from_rowid, 1);
        assert_eq!(manifest.boundary.to_rowid, 2);
        assert_eq!(manifest.record_types.get("domain"), Some(&2));
        assert!(manifest.sha256.len() == 64);
        assert!(manifest.previous_segment_sha256.is_none());

        cleanup(&base);
    }

    #[test]
    fn segment_id_is_deterministic_for_the_same_selection() {
        let boundary = RecordBoundary {
            from_rowid: 1,
            to_rowid: 42,
        };
        let a = deterministic_segment_id(&boundary);
        let b = deterministic_segment_id(&boundary);
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);

        let other = deterministic_segment_id(&RecordBoundary {
            from_rowid: 1,
            to_rowid: 43,
        });
        assert_ne!(a, other);
    }

    /// Review P0: a crash after publication but before the SQLite state
    /// update must be recovered by verify-and-adopt, never by writing a
    /// second segment. Simulated with an equivalent store whose archive
    /// directory already holds the published files.
    #[test]
    fn crash_after_publish_is_recovered_by_adoption() {
        let (base, db, archive) = setup();
        let storage = SqliteStorage::new(&db).unwrap();
        save_domain(&storage, "a.example.com");
        save_domain(&storage, "b.example.com");
        let first = seal_pending(&storage, &archive).unwrap();
        assert!(!first.adopted_existing);
        let segment_id = first.segment_id.clone().unwrap();

        // Disaster: a second store with the same content (same rowids)
        // whose archive directory already holds the published files but
        // has no SQLite state for them — exactly the crash window.
        let db2 = format!("{db}.recovered");
        let recovered = SqliteStorage::new(&db2).unwrap();
        save_domain(&recovered, "a.example.com");
        save_domain(&recovered, "b.example.com");

        let second = seal_pending(&recovered, &archive).unwrap();
        assert!(
            second.adopted_existing,
            "published files must be adopted, not resealed"
        );
        assert_eq!(second.segment_id.as_deref(), Some(segment_id.as_str()));
        assert_eq!(second.record_count, 2);

        // No duplicate files were produced.
        let segments: Vec<_> = std::fs::read_dir(segments_dir(Path::new(&archive)))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "parquet"))
            .collect();
        assert_eq!(segments.len(), 1, "exactly one segment on disk");

        // Both stores now verify green and agree on state.
        let report = verify_archive(&recovered, &archive).unwrap();
        assert!(report.is_ok(), "problems: {:?}", report.problems);
        assert_eq!(recovered.archive_last_sealed_rowid().unwrap(), 2);

        let _ = std::fs::remove_file(db2);
        cleanup(&base);
    }
}
