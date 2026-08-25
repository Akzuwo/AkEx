use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::{
    models::{
        Entry, ExtensionUsage, IndexStatus, Page, StorageAnalysis, VerificationResult, Volume,
    },
    search::SearchQuery,
};

const MIGRATION_001: &str = include_str!("../../migrations/0001_initial.sql");

#[derive(Debug, Clone)]
pub struct Database {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct NewEntry<'a> {
    pub parent_id: Option<i64>,
    pub volume_id: i64,
    pub name: &'a str,
    pub full_path: &'a str,
    pub extension: Option<&'a str>,
    pub is_directory: bool,
    pub size: u64,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub file_attributes: Option<u32>,
    pub hidden: bool,
    pub read_only: bool,
    pub system: bool,
}

pub struct IndexWriter {
    conn: Connection,
    pub volume_id: i64,
    pending: usize,
    in_transaction: bool,
}

impl Database {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn initialize(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = self.connect()?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(MIGRATION_001)?;
            tx.execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (1, ?1)",
                [Utc::now().to_rfc3339()],
            )?;
            tx.pragma_update(None, "user_version", 1)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.path).with_context(|| {
            format!(
                "SQLite-Datenbank {} konnte nicht geöffnet werden",
                self.path.display()
            )
        })?;
        conn.busy_timeout(std::time::Duration::from_secs(8))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA temp_store=MEMORY; PRAGMA mmap_size=268435456;",
        )?;
        Ok(conn)
    }

    pub fn ensure_volume(
        &self,
        volume_id: &str,
        root: &str,
        label: Option<&str>,
        filesystem: Option<&str>,
        total: Option<u64>,
        free: Option<u64>,
    ) -> Result<i64> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO volumes(volume_id, root_path, label, filesystem_type, total_bytes, free_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(root_path) DO UPDATE SET volume_id=excluded.volume_id, label=excluded.label,
               filesystem_type=excluded.filesystem_type, total_bytes=excluded.total_bytes, free_bytes=excluded.free_bytes",
            params![volume_id, root, label, filesystem, u64_to_i64(total), u64_to_i64(free)],
        )?;
        Ok(
            conn.query_row("SELECT id FROM volumes WHERE root_path=?1", [root], |r| {
                r.get(0)
            })?,
        )
    }

    pub fn list_volumes(&self) -> Result<Vec<Volume>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, volume_id, root_path, label, filesystem_type, total_bytes, free_bytes,
                    last_full_scan, index_status, entry_count, last_error FROM volumes ORDER BY root_path",
        )?;
        let rows = stmt
            .query_map([], volume_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn create_index_writer(&self, volume_id: i64) -> Result<IndexWriter> {
        let conn = self.connect()?;
        conn.execute("UPDATE volumes SET index_status='Indexing', last_error=NULL, entry_count=0 WHERE id=?1", [volume_id])?;
        conn.execute("DELETE FROM entries WHERE volume_id=?1", [volume_id])?;
        Ok(IndexWriter {
            conn,
            volume_id,
            pending: 0,
            in_transaction: false,
        })
    }

    pub fn set_volume_error(&self, root_path: &str, message: &str) -> Result<()> {
        self.connect()?.execute(
            "UPDATE volumes SET index_status='Error', last_error=?2 WHERE root_path=?1",
            params![root_path, message],
        )?;
        Ok(())
    }

    pub fn mark_out_of_date(&self, path: &str, message: &str) -> Result<()> {
        let conn = self.connect()?;
        if let Some(id) = volume_id_for_path(&conn, path)? {
            conn.execute(
                "UPDATE volumes SET index_status='OutOfDate', last_error=?2 WHERE id=?1",
                params![id, message],
            )?;
        }
        Ok(())
    }

    pub fn list_directory(
        &self,
        path: &str,
        offset: u64,
        limit: u64,
        sort_field: &str,
        sort_direction: &str,
    ) -> Result<Page<Entry>> {
        let conn = self.connect()?;
        let parent_id: i64 = conn.query_row(
            "SELECT id FROM entries WHERE full_path=?1 AND is_directory=1",
            [path],
            |r| r.get(0),
        )?;
        let total = conn
            .query_row(
                "SELECT COUNT(*) FROM entries WHERE parent_id=?1",
                [parent_id],
                |r| r.get::<_, i64>(0),
            )?
            .max(0) as u64;
        let order = entry_order(sort_field, sort_direction, "")?;
        let mut stmt = conn.prepare(&format!(
            "SELECT id,parent_id,volume_id,name,full_path,extension,is_directory,size,recursive_size,
                    created_at,modified_at,hidden,read_only,system
             FROM entries WHERE parent_id=?1
             ORDER BY {order} LIMIT ?2 OFFSET ?3",
        ))?;
        let items = stmt
            .query_map(
                params![
                    parent_id,
                    clamp_limit(limit) as i64,
                    offset.min(i64::MAX as u64) as i64
                ],
                entry_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Page {
            items,
            total,
            offset,
            limit: clamp_limit(limit),
        })
    }

    pub fn get_entry(&self, path: &str) -> Result<Option<Entry>> {
        let conn = self.connect()?;
        Ok(conn.query_row(
            "SELECT id,parent_id,volume_id,name,full_path,extension,is_directory,size,recursive_size,
                    created_at,modified_at,hidden,read_only,system FROM entries WHERE full_path=?1",
            [path], entry_from_row,
        ).optional()?)
    }

    pub fn direct_child_paths(&self, path: &str) -> Result<Vec<String>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT full_path FROM entries WHERE parent_id=(SELECT id FROM entries WHERE full_path=?1)",
        )?;
        let rows = stmt
            .query_map([path], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn search(
        &self,
        query: &SearchQuery,
        offset: u64,
        limit: u64,
        sort_field: &str,
        sort_direction: &str,
    ) -> Result<Page<Entry>> {
        let conn = self.connect()?;
        let (from, where_sql, values) = query.to_sql();
        let count_sql = format!("SELECT COUNT(*) {from} WHERE {where_sql}");
        let total = conn
            .query_row(&count_sql, rusqlite::params_from_iter(values.iter()), |r| {
                r.get::<_, i64>(0)
            })?
            .max(0) as u64;
        let order = entry_order(sort_field, sort_direction, "e.")?;
        let sql = format!(
            "SELECT e.id,e.parent_id,e.volume_id,e.name,e.full_path,e.extension,e.is_directory,e.size,e.recursive_size,
                    e.created_at,e.modified_at,e.hidden,e.read_only,e.system
             {from} WHERE {where_sql} ORDER BY {order} LIMIT ? OFFSET ?"
        );
        let mut paged_values = values;
        paged_values.push(rusqlite::types::Value::Integer(clamp_limit(limit) as i64));
        paged_values.push(rusqlite::types::Value::Integer(offset as i64));
        let mut stmt = conn.prepare(&sql)?;
        let items = stmt
            .query_map(
                rusqlite::params_from_iter(paged_values.iter()),
                entry_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Page {
            items,
            total,
            offset,
            limit: clamp_limit(limit),
        })
    }

    pub fn storage_analysis(&self, path: &str, limit: u64) -> Result<StorageAnalysis> {
        let conn = self.connect()?;
        let pattern = descendant_pattern(path);
        let scope = "(full_path=?1 OR full_path LIKE ?2 ESCAPE '\\')";
        let (total_bytes, file_count, folder_count): (i64, i64, i64) = conn.query_row(
            &format!(
                "SELECT COALESCE(SUM(CASE WHEN is_directory=0 THEN size ELSE 0 END),0),
                    SUM(CASE WHEN is_directory=0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN is_directory=1 THEN 1 ELSE 0 END) FROM entries WHERE {scope}"
            ),
            params![path, pattern],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let largest_folders = query_entries(&conn, &format!(
            "SELECT id,parent_id,volume_id,name,full_path,extension,is_directory,size,recursive_size,
             created_at,modified_at,hidden,read_only,system FROM entries WHERE {scope} AND is_directory=1 AND full_path<>?1
             ORDER BY recursive_size DESC LIMIT ?3"), params![path, pattern, clamp_limit(limit) as i64])?;
        let largest_files = query_entries(&conn, &format!(
            "SELECT id,parent_id,volume_id,name,full_path,extension,is_directory,size,recursive_size,
             created_at,modified_at,hidden,read_only,system FROM entries WHERE {scope} AND is_directory=0
             ORDER BY size DESC LIMIT ?3"), params![path, pattern, clamp_limit(limit) as i64])?;
        let mut stmt = conn.prepare(&format!(
            "SELECT COALESCE(NULLIF(extension,''),'(ohne Endung)'), SUM(size), COUNT(*) FROM entries
             WHERE {scope} AND is_directory=0 GROUP BY extension ORDER BY SUM(size) DESC LIMIT 20"
        ))?;
        let extensions = stmt
            .query_map(params![path, pattern], |r| {
                Ok(ExtensionUsage {
                    extension: r.get(0)?,
                    bytes: r.get::<_, i64>(1)?.max(0) as u64,
                    count: r.get::<_, i64>(2)?.max(0) as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(StorageAnalysis {
            total_bytes: total_bytes.max(0) as u64,
            file_count: file_count.max(0) as u64,
            folder_count: folder_count.max(0) as u64,
            largest_folders,
            largest_files,
            extensions,
        })
    }

    pub fn verify(&self, volume_id: i64) -> Result<VerificationResult> {
        let conn = self.connect()?;
        let integrity_message: String =
            conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        let orphan_count = conn
            .query_row(
                "SELECT COUNT(*) FROM entries e LEFT JOIN entries p ON e.parent_id=p.id
             WHERE e.volume_id=?1 AND e.parent_id IS NOT NULL AND p.id IS NULL",
                [volume_id],
                |r| r.get::<_, i64>(0),
            )?
            .max(0) as u64;
        let size_mismatch_count = conn.query_row(
            "SELECT COUNT(*) FROM entries d WHERE d.volume_id=?1 AND d.is_directory=1 AND d.recursive_size !=
             COALESCE((SELECT SUM(CASE WHEN c.is_directory=1 THEN c.recursive_size ELSE c.size END) FROM entries c WHERE c.parent_id=d.id),0)",
            [volume_id], |r| r.get::<_, i64>(0),
        )?.max(0) as u64;
        Ok(VerificationResult {
            ok: integrity_message == "ok" && orphan_count == 0 && size_mismatch_count == 0,
            integrity_message,
            orphan_count,
            size_mismatch_count,
        })
    }

    pub fn remove_path(&self, path: &str) -> Result<()> {
        let mut conn = self.connect()?;
        let tx = conn.transaction()?;
        if let Some((id, parent_id, bytes)) = tx.query_row(
            "SELECT id,parent_id,CASE WHEN is_directory=1 THEN recursive_size ELSE size END FROM entries WHERE full_path=?1",
            [path], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, i64>(2)?)),
        ).optional()? {
            tx.execute("DELETE FROM entries WHERE id=?1", [id])?;
            apply_delta(&tx, parent_id, -bytes)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_path(&self, path: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        let full_path = path.to_string_lossy().to_string();
        let parent = path.parent().map(|p| p.to_string_lossy().to_string());
        let mut conn = self.connect()?;
        let tx = conn.transaction()?;
        let Some(volume_id) = volume_id_for_path(&tx, &full_path)? else {
            return Ok(());
        };
        let parent_id = parent.as_deref().and_then(|p| {
            tx.query_row("SELECT id FROM entries WHERE full_path=?1", [p], |r| {
                r.get(0)
            })
            .optional()
            .ok()
            .flatten()
        });
        let old: Option<(i64, i64)> = tx.query_row(
            "SELECT id,CASE WHEN is_directory=1 THEN recursive_size ELSE size END FROM entries WHERE full_path=?1",
            [&full_path], |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional()?;
        let is_directory = metadata.is_dir();
        let size = if is_directory {
            0
        } else {
            metadata.len() as i64
        };
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| full_path.clone());
        let extension = path.extension().map(|s| s.to_string_lossy().to_lowercase());
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO entries(parent_id,volume_id,name,full_path,extension,is_directory,size,recursive_size,created_at,modified_at,indexed_at,hidden,read_only,system)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?7,?8,?9,?10,?11,?12,0)
             ON CONFLICT(volume_id,full_path) DO UPDATE SET parent_id=excluded.parent_id,name=excluded.name,extension=excluded.extension,
               is_directory=excluded.is_directory,size=excluded.size,recursive_size=CASE WHEN excluded.is_directory=1 THEN entries.recursive_size ELSE excluded.size END,
               created_at=excluded.created_at,modified_at=excluded.modified_at,indexed_at=excluded.indexed_at,hidden=excluded.hidden,read_only=excluded.read_only",
            params![parent_id,volume_id,name,full_path,extension,is_directory,size,
                system_time(metadata.created().ok()),system_time(metadata.modified().ok()),now,
                name.starts_with('.'),metadata.permissions().readonly()],
        )?;
        let old_bytes = old.map(|v| v.1).unwrap_or(0);
        let new_bytes = if is_directory { old_bytes } else { size };
        apply_delta(&tx, parent_id, new_bytes - old_bytes)?;
        tx.commit()?;
        Ok(())
    }

    pub fn move_path(&self, source: &str, destination: &str) -> Result<()> {
        let mut conn = self.connect()?;
        let tx = conn.transaction()?;
        let Some((id, old_parent, bytes)) = tx.query_row(
            "SELECT id,parent_id,CASE WHEN is_directory=1 THEN recursive_size ELSE size END FROM entries WHERE full_path=?1",
            [source], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, i64>(2)?)),
        ).optional()? else { return Ok(()); };
        let destination_path = Path::new(destination);
        let new_parent_path = destination_path
            .parent()
            .map(|p| p.to_string_lossy().to_string());
        let new_parent: Option<i64> = new_parent_path.as_deref().and_then(|p| {
            tx.query_row("SELECT id FROM entries WHERE full_path=?1", [p], |r| {
                r.get(0)
            })
            .optional()
            .ok()
            .flatten()
        });
        let name = destination_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let extension = destination_path
            .extension()
            .map(|s| s.to_string_lossy().to_lowercase());
        tx.execute("UPDATE entries SET parent_id=?2,name=?3,extension=?4,full_path=?5 || substr(full_path, ?6),indexed_at=?7
                    WHERE id=?1", params![id,new_parent,name,extension,destination,source.chars().count() as i64 + 1,Utc::now().to_rfc3339()])?;
        let escaped = escape_like(source);
        tx.execute(
            "UPDATE entries SET full_path=?1 || substr(full_path, ?2),indexed_at=?3 WHERE full_path LIKE ?4 ESCAPE '\\'",
            params![destination, source.chars().count() as i64 + 1, Utc::now().to_rfc3339(), format!("{}\\\\%", escaped)],
        )?;
        if old_parent != new_parent {
            apply_delta(&tx, old_parent, -bytes)?;
            apply_delta(&tx, new_parent, bytes)?;
        }
        tx.commit()?;
        Ok(())
    }
}

impl IndexWriter {
    fn begin(&mut self) -> Result<()> {
        if !self.in_transaction {
            self.conn.execute_batch("BEGIN IMMEDIATE")?;
            self.in_transaction = true;
        }
        Ok(())
    }

    pub fn insert(&mut self, entry: NewEntry<'_>) -> Result<i64> {
        self.begin()?;
        let indexed_at = Utc::now().to_rfc3339();
        self.conn.prepare_cached(
            "INSERT INTO entries(parent_id,volume_id,name,full_path,extension,is_directory,size,recursive_size,
             created_at,modified_at,indexed_at,file_attributes,hidden,read_only,system)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)"
        )?.execute(params![entry.parent_id,entry.volume_id,entry.name,entry.full_path,entry.extension,
            entry.is_directory,entry.size.min(i64::MAX as u64) as i64,if entry.is_directory {0_i64} else {entry.size.min(i64::MAX as u64) as i64},entry.created_at,
            entry.modified_at,indexed_at,entry.file_attributes,entry.hidden,entry.read_only,entry.system])?;
        let id = self.conn.last_insert_rowid();
        self.pending += 1;
        if self.pending >= 2_000 {
            self.commit_batch()?;
        }
        Ok(id)
    }

    pub fn set_directory_size(&mut self, id: i64, bytes: u64) -> Result<()> {
        self.begin()?;
        self.conn.execute(
            "UPDATE entries SET recursive_size=?2 WHERE id=?1",
            params![id, bytes.min(i64::MAX as u64) as i64],
        )?;
        self.pending += 1;
        if self.pending >= 2_000 {
            self.commit_batch()?;
        }
        Ok(())
    }

    pub fn commit_batch(&mut self) -> Result<()> {
        if self.in_transaction {
            self.conn.execute_batch("COMMIT")?;
            self.in_transaction = false;
            self.pending = 0;
        }
        Ok(())
    }

    pub fn finish(mut self, count: u64) -> Result<()> {
        self.commit_batch()?;
        self.conn.execute(
            "UPDATE volumes SET index_status='Ready',last_full_scan=?2,last_error=NULL,entry_count=?3 WHERE id=?1",
            params![self.volume_id,Utc::now().to_rfc3339(),count.min(i64::MAX as u64) as i64],
        )?;
        self.conn.execute_batch("PRAGMA optimize")?;
        Ok(())
    }
}

fn volume_from_row(row: &Row<'_>) -> rusqlite::Result<Volume> {
    let status: String = row.get(8)?;
    Ok(Volume {
        id: row.get(0)?,
        volume_id: row.get(1)?,
        root_path: row.get(2)?,
        label: row.get(3)?,
        filesystem_type: row.get(4)?,
        total_bytes: row.get::<_, Option<i64>>(5)?.map(|v| v.max(0) as u64),
        free_bytes: row.get::<_, Option<i64>>(6)?.map(|v| v.max(0) as u64),
        last_full_scan: row.get(7)?,
        index_status: parse_status(&status),
        entry_count: row.get::<_, i64>(9)?.max(0) as u64,
        last_error: row.get(10)?,
    })
}

pub fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        volume_id: row.get(2)?,
        name: row.get(3)?,
        full_path: row.get(4)?,
        extension: row.get(5)?,
        is_directory: row.get(6)?,
        size: row.get::<_, i64>(7)?.max(0) as u64,
        recursive_size: row.get::<_, i64>(8)?.max(0) as u64,
        created_at: row.get(9)?,
        modified_at: row.get(10)?,
        hidden: row.get(11)?,
        read_only: row.get(12)?,
        system: row.get(13)?,
    })
}

fn query_entries<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<Entry>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(params, entry_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn apply_delta(conn: &Connection, mut parent_id: Option<i64>, delta: i64) -> Result<()> {
    while let Some(id) = parent_id {
        conn.execute(
            "UPDATE entries SET recursive_size=MAX(0,recursive_size+?2) WHERE id=?1",
            params![id, delta],
        )?;
        parent_id = conn
            .query_row("SELECT parent_id FROM entries WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
    }
    Ok(())
}

fn volume_id_for_path(conn: &Connection, path: &str) -> Result<Option<i64>> {
    Ok(conn.query_row(
        "SELECT id FROM volumes WHERE ?1 LIKE root_path || '%' ORDER BY length(root_path) DESC LIMIT 1",
        [path], |r| r.get(0),
    ).optional()?)
}

fn parse_status(value: &str) -> IndexStatus {
    match value {
        "Indexing" => IndexStatus::Indexing,
        "Ready" => IndexStatus::Ready,
        "OutOfDate" => IndexStatus::OutOfDate,
        "Error" => IndexStatus::Error,
        _ => IndexStatus::NotIndexed,
    }
}

fn clamp_limit(limit: u64) -> u64 {
    limit.clamp(1, 500)
}
fn u64_to_i64(value: Option<u64>) -> Option<i64> {
    value.map(|v| v.min(i64::MAX as u64) as i64)
}
fn system_time(value: Option<SystemTime>) -> Option<String> {
    value
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
        .map(|d| d.to_rfc3339())
}
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
fn descendant_pattern(path: &str) -> String {
    format!("{}\\\\%", escape_like(path.trim_end_matches('\\')))
}

fn entry_order(sort_field: &str, sort_direction: &str, prefix: &str) -> Result<String> {
    let direction = match sort_direction {
        "asc" => "ASC",
        "desc" => "DESC",
        _ => anyhow::bail!("Ungültige Sortierrichtung: {sort_direction}"),
    };
    let value = match sort_field {
        "name" => format!("{prefix}name COLLATE NOCASE"),
        "type" => format!(
            "CASE WHEN {prefix}is_directory=1 THEN 'Ordner' WHEN COALESCE({prefix}extension,'')='' THEN 'Datei' ELSE {prefix}extension END COLLATE NOCASE"
        ),
        "size" => format!(
            "CASE WHEN {prefix}is_directory=1 THEN {prefix}recursive_size ELSE {prefix}size END"
        ),
        "modified" => format!("{prefix}modified_at"),
        _ => anyhow::bail!("Ungültiges Sortierfeld: {sort_field}"),
    };
    Ok(format!(
        "{value} {direction}, {prefix}name COLLATE NOCASE ASC, {prefix}id ASC"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn entry_sorting_accepts_every_visible_column() {
        for field in ["name", "type", "size", "modified"] {
            assert!(entry_order(field, "asc", "").is_ok());
            assert!(entry_order(field, "desc", "e.").is_ok());
        }
        assert!(entry_order("path", "asc", "").is_err());
        assert!(entry_order("name", "sideways", "").is_err());
    }

    #[test]
    fn migration_and_parent_delta_are_consistent() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("index.db"));
        db.initialize().unwrap();
        let volume = db
            .ensure_volume("test", "T:\\", None, Some("NTFS"), None, None)
            .unwrap();
        let mut writer = db.create_index_writer(volume).unwrap();
        let root = writer
            .insert(NewEntry {
                parent_id: None,
                volume_id: volume,
                name: "T:\\",
                full_path: "T:\\",
                extension: None,
                is_directory: true,
                size: 0,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        let child = writer
            .insert(NewEntry {
                parent_id: Some(root),
                volume_id: volume,
                name: "data",
                full_path: "T:\\data",
                extension: None,
                is_directory: true,
                size: 0,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        writer
            .insert(NewEntry {
                parent_id: Some(child),
                volume_id: volume,
                name: "a.bin",
                full_path: "T:\\data\\a.bin",
                extension: Some("bin"),
                is_directory: false,
                size: 100,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        writer.set_directory_size(child, 100).unwrap();
        writer.set_directory_size(root, 100).unwrap();
        writer.finish(3).unwrap();
        db.remove_path("T:\\data\\a.bin").unwrap();
        assert_eq!(db.get_entry("T:\\data").unwrap().unwrap().recursive_size, 0);
        assert_eq!(db.get_entry("T:\\").unwrap().unwrap().recursive_size, 0);
    }

    #[test]
    fn fts_search_and_filters_query_the_index() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("index.db"));
        db.initialize().unwrap();
        let volume = db
            .ensure_volume("test", "T:\\", None, Some("NTFS"), None, None)
            .unwrap();
        let mut writer = db.create_index_writer(volume).unwrap();
        let root = writer
            .insert(NewEntry {
                parent_id: None,
                volume_id: volume,
                name: "T:\\",
                full_path: "T:\\",
                extension: None,
                is_directory: true,
                size: 0,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        writer
            .insert(NewEntry {
                parent_id: Some(root),
                volume_id: volume,
                name: "scene.blend",
                full_path: "T:\\Projekte\\scene.blend",
                extension: Some("blend"),
                is_directory: false,
                size: 800 * 1024 * 1024,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        writer.finish(2).unwrap();
        let query =
            SearchQuery::parse("scene ext:blend size:>500mb type:file path:Projekte").unwrap();
        let result = db.search(&query, 0, 20, "modified", "desc").unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].name, "scene.blend");
        assert_eq!(
            db.storage_analysis("T:\\", 20).unwrap().total_bytes,
            800 * 1024 * 1024
        );
    }

    #[test]
    fn directory_rename_updates_descendant_paths() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("index.db"));
        db.initialize().unwrap();
        let volume = db
            .ensure_volume("test", "T:\\", None, Some("NTFS"), None, None)
            .unwrap();
        let mut writer = db.create_index_writer(volume).unwrap();
        let root = writer
            .insert(NewEntry {
                parent_id: None,
                volume_id: volume,
                name: "T:\\",
                full_path: "T:\\",
                extension: None,
                is_directory: true,
                size: 0,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        let source = writer
            .insert(NewEntry {
                parent_id: Some(root),
                volume_id: volume,
                name: "old",
                full_path: "T:\\old",
                extension: None,
                is_directory: true,
                size: 0,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        writer
            .insert(NewEntry {
                parent_id: Some(source),
                volume_id: volume,
                name: "data.bin",
                full_path: "T:\\old\\data.bin",
                extension: Some("bin"),
                is_directory: false,
                size: 42,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        writer.set_directory_size(source, 42).unwrap();
        writer.set_directory_size(root, 42).unwrap();
        writer.finish(3).unwrap();
        db.move_path("T:\\old", "T:\\renamed").unwrap();
        assert!(db.get_entry("T:\\old\\data.bin").unwrap().is_none());
        assert_eq!(
            db.get_entry("T:\\renamed\\data.bin").unwrap().unwrap().size,
            42
        );
    }

    #[test]
    fn file_change_applies_only_the_size_delta_to_all_parents() {
        let dir = tempdir().unwrap();
        let root_path = dir.path().to_string_lossy().to_string();
        let child_path = dir.path().join("data");
        std::fs::create_dir(&child_path).unwrap();
        let file_path = child_path.join("payload.bin");
        let db = Database::new(dir.path().join("index.db"));
        db.initialize().unwrap();
        let volume = db
            .ensure_volume("temp-volume", &root_path, None, Some("NTFS"), None, None)
            .unwrap();
        let mut writer = db.create_index_writer(volume).unwrap();
        let root = writer
            .insert(NewEntry {
                parent_id: None,
                volume_id: volume,
                name: "root",
                full_path: &root_path,
                extension: None,
                is_directory: true,
                size: 0,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        writer
            .insert(NewEntry {
                parent_id: Some(root),
                volume_id: volume,
                name: "data",
                full_path: &child_path.to_string_lossy(),
                extension: None,
                is_directory: true,
                size: 0,
                created_at: None,
                modified_at: None,
                file_attributes: None,
                hidden: false,
                read_only: false,
                system: false,
            })
            .unwrap();
        writer.finish(2).unwrap();

        std::fs::write(&file_path, vec![0_u8; 100]).unwrap();
        db.upsert_path(&file_path).unwrap();
        assert_eq!(
            db.get_entry(&child_path.to_string_lossy())
                .unwrap()
                .unwrap()
                .recursive_size,
            100
        );
        assert_eq!(
            db.get_entry(&root_path).unwrap().unwrap().recursive_size,
            100
        );

        std::fs::write(&file_path, vec![0_u8; 150]).unwrap();
        db.upsert_path(&file_path).unwrap();
        assert_eq!(
            db.get_entry(&child_path.to_string_lossy())
                .unwrap()
                .unwrap()
                .recursive_size,
            150
        );
        assert_eq!(
            db.get_entry(&root_path).unwrap().unwrap().recursive_size,
            150
        );
    }
}
