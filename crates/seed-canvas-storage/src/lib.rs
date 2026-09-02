//! SQLite-backed storage layer for seed-canvas.
//!
//! Owns the on-disk format of a gallery:
//!
//! * `artworks` table — one row per rendered artwork (seed, template, params,
//!   content hash, file path, created_at).
//! * `artworks_fts` virtual table — FTS5 index over the artwork's display
//!   string for fast full-text search.
//! * `galleries` table — collections of artworks.
//!
//! Every public method takes `&self` and acquires a connection from the
//! pool internally, so callers can hold a single [`Gallery`] handle and
//! share it across threads.

#![deny(missing_docs)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Errors raised by the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite I/O or schema failure.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Connection pool exhausted or unhealthy.
    #[error("connection pool: {0}")]
    Pool(#[from] r2d2::Error),

    /// JSON encoding/decoding failed.
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    /// Filesystem I/O error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// The requested record was not found.
    #[error("not found: {0}")]
    NotFound(String),
}

/// Result alias for the storage layer.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Connection-pool wrapper around a single SQLite database.
#[derive(Clone)]
pub struct Gallery {
    pool: Arc<Pool<SqliteConnectionManager>>,
    root: PathBuf,
}

impl std::fmt::Debug for Gallery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gallery")
            .field("root", &self.root)
            .field("pool_size", &self.pool.state().connections)
            .finish()
    }
}

impl Gallery {
    /// Open an existing gallery or create a new one at `root`.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        let db_path = root.join("gallery.db");
        let manager = SqliteConnectionManager::file(&db_path).with_init(|c| {
            c.execute_batch(
                "PRAGMA journal_mode = WAL;
                     PRAGMA foreign_keys = ON;
                     PRAGMA synchronous = NORMAL;
                     PRAGMA busy_timeout = 5000;",
            )
        });
        let pool = Pool::builder().max_size(8).build(manager)?;
        let gallery = Self {
            pool: Arc::new(pool),
            root,
        };
        gallery.migrate()?;
        Ok(gallery)
    }

    /// Apply the latest schema. Safe to call repeatedly — it is idempotent.
    pub fn migrate(&self) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(())
    }

    /// Absolute path of the SQLite database file.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.root.join("gallery.db")
    }

    /// Absolute path of the `artworks/` directory where rendered files live.
    #[must_use]
    pub fn artworks_dir(&self) -> PathBuf {
        self.root.join("artworks")
    }

    /// Insert or look up an artwork by its deterministic `(seed, template,
    /// params, format)` tuple. Returns the row's UUID.
    pub fn upsert_artwork(&self, record: NewArtwork<'_>) -> Result<Uuid> {
        let conn = self.pool.get()?;
        let params_json = serde_json::to_string(record.params)?;
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM artworks
                 WHERE template_id = ?1 AND seed_raw = ?2 AND format = ?3 AND params = ?4",
                params![
                    record.template_id,
                    record.seed_raw,
                    record.format,
                    params_json
                ],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Uuid::parse_str(&id).map_err(|e| StorageError::NotFound(e.to_string()));
        }

        let id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO artworks
             (id, template_id, template_version, seed_raw, seed_handle, params, format,
              content_hash, file_path, adapter, width, height, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id.to_string(),
                record.template_id,
                record.template_version,
                record.seed_raw,
                record.seed_handle,
                params_json,
                record.format,
                record.content_hash,
                record.file_path.to_string_lossy(),
                record.adapter,
                record.width as i64,
                record.height as i64,
                Utc::now().to_rfc3339(),
            ],
        )?;

        let display = format!(
            "{} seed={} ({})",
            record.template_id, record.seed_raw, record.format
        );
        conn.execute(
            "INSERT INTO artworks_fts(rowid, display) VALUES (last_insert_rowid(), ?1)",
            params![display],
        )?;
        Ok(id)
    }

    /// Fetch a single artwork by UUID.
    pub fn artwork(&self, id: Uuid) -> Result<Artwork> {
        let conn = self.pool.get()?;
        let row = conn
            .query_row(
                "SELECT id, template_id, template_version, seed_raw, seed_handle, params,
                        format, content_hash, file_path, adapter, width, height, created_at
                 FROM artworks WHERE id = ?1",
                params![id.to_string()],
                map_artwork,
            )
            .optional()?
            .ok_or_else(|| StorageError::NotFound(id.to_string()))?;
        Ok(row)
    }

    /// List artworks, most recent first.
    pub fn list_artworks(&self, limit: i64) -> Result<Vec<Artwork>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, template_id, template_version, seed_raw, seed_handle, params,
                    format, content_hash, file_path, adapter, width, height, created_at
             FROM artworks
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit], map_artwork)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Search artworks by full-text query.
    pub fn search(&self, query: &str, limit: i64) -> Result<Vec<Artwork>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.template_id, a.template_version, a.seed_raw, a.seed_handle,
                    a.params, a.format, a.content_hash, a.file_path, a.adapter,
                    a.width, a.height, a.created_at
             FROM artworks_fts f
             JOIN artworks a ON a.rowid = f.rowid
             WHERE artworks_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![query, limit], map_artwork)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Count rows. Used by `seed-canvas doctor` and `seed-canvas list`.
    pub fn count_artworks(&self) -> Result<i64> {
        let conn = self.pool.get()?;
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM artworks", [], |r| r.get(0))?;
        Ok(n)
    }
}

/// Schema applied by [`Gallery::migrate`]. Kept as a single string so the
/// migration runs as one transaction.
const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS galleries (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    visibility TEXT NOT NULL DEFAULT 'private',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS artworks (
    id TEXT PRIMARY KEY NOT NULL,
    template_id TEXT NOT NULL,
    template_version TEXT NOT NULL,
    seed_raw TEXT NOT NULL,
    seed_handle TEXT NOT NULL,
    params TEXT NOT NULL DEFAULT '{}',
    format TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    file_path TEXT NOT NULL,
    adapter TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (template_id, seed_raw, format, params)
);

CREATE INDEX IF NOT EXISTS artworks_template_seed_idx
    ON artworks (template_id, seed_raw);

CREATE INDEX IF NOT EXISTS artworks_created_at_idx
    ON artworks (created_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS artworks_fts USING fts5(
    display,
    content='',
    tokenize = 'unicode61 remove_diacritics 2'
);
";

/// Input record for [`Gallery::upsert_artwork`].
#[derive(Debug, Clone)]
pub struct NewArtwork<'a> {
    /// Template identifier.
    pub template_id: &'a str,
    /// Template version at render time.
    pub template_version: &'a str,
    /// Raw seed string the user supplied.
    pub seed_raw: &'a str,
    /// Canonical 24-hex handle (`sc_…`).
    pub seed_handle: &'a str,
    /// Validated parameters object.
    pub params: &'a serde_json::Value,
    /// Output format (`png`, `svg`, `json`).
    pub format: &'a str,
    /// SHA-256 of the encoded bytes.
    pub content_hash: &'a str,
    /// Filesystem path to the artifact.
    pub file_path: &'a Path,
    /// Adapter used to render the artwork.
    pub adapter: &'a str,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
}

/// A row from the `artworks` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artwork {
    /// UUID primary key.
    pub id: Uuid,
    /// Template identifier.
    pub template_id: String,
    /// Template version.
    pub template_version: String,
    /// Raw seed string.
    pub seed_raw: String,
    /// Canonical handle.
    pub seed_handle: String,
    /// Original parameter object.
    pub params: serde_json::Value,
    /// Output format.
    pub format: String,
    /// Content hash (hex).
    pub content_hash: String,
    /// Filesystem path.
    pub file_path: PathBuf,
    /// Adapter used.
    pub adapter: String,
    /// Canvas width.
    pub width: u32,
    /// Canvas height.
    pub height: u32,
    /// When the row was created.
    pub created_at: DateTime<Utc>,
}

fn map_artwork(row: &rusqlite::Row<'_>) -> rusqlite::Result<Artwork> {
    let id: String = row.get(0)?;
    let params_json: String = row.get(5)?;
    let file_path: String = row.get(8)?;
    let created_at: String = row.get(12)?;
    Ok(Artwork {
        id: Uuid::parse_str(&id).map_err(|e| {
            rusqlite::Error::InvalidColumnType(0, e.to_string(), rusqlite::types::Type::Text)
        })?,
        template_id: row.get(1)?,
        template_version: row.get(2)?,
        seed_raw: row.get(3)?,
        seed_handle: row.get(4)?,
        params: serde_json::from_str(&params_json).map_err(|e| {
            rusqlite::Error::InvalidColumnType(5, e.to_string(), rusqlite::types::Type::Text)
        })?,
        format: row.get(6)?,
        content_hash: row.get(7)?,
        file_path: PathBuf::from(file_path),
        adapter: row.get(9)?,
        width: row.get::<_, i64>(10)? as u32,
        height: row.get::<_, i64>(11)? as u32,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map_err(|e| {
                rusqlite::Error::InvalidColumnType(12, e.to_string(), rusqlite::types::Type::Text)
            })?
            .with_timezone(&Utc),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn open_creates_db_and_runs_migrations() {
        let dir = tempdir().unwrap();
        let gallery = Gallery::open(dir.path()).unwrap();
        assert!(gallery.db_path().exists());
    }

    #[test]
    fn upsert_is_idempotent() {
        let dir = tempdir().unwrap();
        let gallery = Gallery::open(dir.path()).unwrap();
        let params = serde_json::json!({"count": 100});
        let record = NewArtwork {
            template_id: "galaxy",
            template_version: "0.1.0",
            seed_raw: "cosmos",
            seed_handle: "sc_1eebd7175c6b0b26921647f4",
            params: &params,
            format: "png",
            content_hash: "deadbeef",
            file_path: Path::new("/tmp/cosmos.png"),
            adapter: "server",
            width: 1024,
            height: 1024,
        };
        let a = gallery.upsert_artwork(record.clone()).unwrap();
        let b = gallery.upsert_artwork(record).unwrap();
        assert_eq!(a, b, "upsert must return the same UUID on conflict");
    }

    #[test]
    fn list_returns_recent_artworks() {
        let dir = tempdir().unwrap();
        let gallery = Gallery::open(dir.path()).unwrap();
        let params = serde_json::json!({});
        for seed in ["a", "b", "c"] {
            gallery
                .upsert_artwork(NewArtwork {
                    template_id: "galaxy",
                    template_version: "0.1.0",
                    seed_raw: seed,
                    seed_handle: "sc_xxxxxxxxxxxxxxxxxxxxxxxx",
                    params: &params,
                    format: "png",
                    content_hash: "cafebabe",
                    file_path: Path::new("/tmp/x.png"),
                    adapter: "server",
                    width: 1,
                    height: 1,
                })
                .unwrap();
        }
        let rows = gallery.list_artworks(10).unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn fts_search_finds_records() {
        let dir = tempdir().unwrap();
        let gallery = Gallery::open(dir.path()).unwrap();
        let params = serde_json::json!({});
        gallery
            .upsert_artwork(NewArtwork {
                template_id: "galaxy",
                template_version: "0.1.0",
                seed_raw: "deep-space",
                seed_handle: "sc_xxxxxxxxxxxxxxxxxxxxxxxx",
                params: &params,
                format: "png",
                content_hash: "deadbeef",
                file_path: Path::new("/tmp/ds.png"),
                adapter: "server",
                width: 1,
                height: 1,
            })
            .unwrap();
        let hits = gallery.search("\"deep-space\"", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }
}
