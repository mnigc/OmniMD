//! SQLite workspace data layer (M2 Workbench).
//!
//! Responsibilities:
//! - Workspace registry (local folder + display name)
//! - Document metadata table (path, title, size, mtime, favorite, source)
//! - FTS5 full-text index over document title/body/tags with CJK bigram
//!   tokenization so Chinese search works with two-character queries.
//! - Favorites / recent-history lookups.
//!
//! The markdown *content* itself is never stored as the source of truth:
//! documents live on disk, the DB only keeps metadata + the search index.

use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::SystemTime;

use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::models::task::{BatchTaskDto, BatchSummaryDto, OutputMode, ParseQuality};

// ---------------------------------------------------------------------------
// DTOs (serialized to the frontend, camelCase)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_opened_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDto {
    pub id: i64,
    pub workspace_id: i64,
    pub path: String,
    pub title: String,
    pub file_size: i64,
    pub favorite: bool,
    pub source: Option<String>,
    pub created_at: String,
    pub opened_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    pub name: String,
    pub path: String,
    pub doc_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub document: DocumentDto,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanResultDto {
    pub indexed: usize,
    pub updated: usize,
    pub removed: usize,
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

const SCHEMA_SQL: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS workspaces (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT NOT NULL,
    path           TEXT NOT NULL UNIQUE,
    created_at     TEXT NOT NULL,
    last_opened_at TEXT
);

CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS documents (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    path         TEXT NOT NULL,
    title        TEXT NOT NULL DEFAULT '',
    file_size    INTEGER NOT NULL DEFAULT 0,
    mtime        INTEGER NOT NULL DEFAULT 0,
    favorite     INTEGER NOT NULL DEFAULT 0,
    source       TEXT,
    created_at   TEXT NOT NULL,
    opened_at    TEXT,
    UNIQUE(workspace_id, path)
);

CREATE INDEX IF NOT EXISTS idx_documents_workspace ON documents(workspace_id);
CREATE INDEX IF NOT EXISTS idx_documents_favorite  ON documents(workspace_id, favorite);
CREATE INDEX IF NOT EXISTS idx_documents_opened    ON documents(workspace_id, opened_at);

-- Full-text index (rowid aligned with documents.id).
-- Body/title/tags are pre-tokenized with CJK bigrams before insertion,
-- so unicode61 can match two-character Chinese terms.
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
    title, body, tags,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE IF NOT EXISTS batch_tasks (
    id           TEXT PRIMARY KEY,
    source_path  TEXT NOT NULL,
    output_path  TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'Pending',
    progress     REAL NOT NULL DEFAULT 0.0,
    stage        TEXT NOT NULL DEFAULT 'Queued',
    error        TEXT,
    created_at   INTEGER NOT NULL,
    completed_at INTEGER,
    elapsed_secs INTEGER NOT NULL DEFAULT 0,
    retry_count  INTEGER NOT NULL DEFAULT 0,
    output_mode  TEXT NOT NULL DEFAULT 'aiReady',
    parse_quality TEXT NOT NULL DEFAULT 'auto'
);

CREATE INDEX IF NOT EXISTS idx_batch_tasks_status ON batch_tasks(status);
CREATE INDEX IF NOT EXISTS idx_batch_tasks_created ON batch_tasks(created_at);
"#;

pub struct WorkspaceDb {
    conn: Connection,
}

impl WorkspaceDb {
    /// Open (and initialize) the database at `path`.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    /// Open the workspace database inside the platform app-data directory.
    pub fn open_in_app_data(app: &tauri::AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("无法解析应用数据目录: {e}"))?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("无法创建应用数据目录 {}: {e}", dir.display()))?;
        let db_path = dir.join("omnimd.db");
        Self::open(&db_path).map_err(|e| format!("无法打开工作区数据库 {}: {e}", db_path.display()))
    }

    fn init(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(SCHEMA_SQL)
    }

    // -- workspaces ---------------------------------------------------------

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceDto>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, path, created_at, last_opened_at
                 FROM workspaces ORDER BY name COLLATE NOCASE",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], row_to_workspace)
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn get_workspace(&self, id: i64) -> Result<Option<WorkspaceDto>, String> {
        self.conn
            .query_row(
                "SELECT id, name, path, created_at, last_opened_at
                 FROM workspaces WHERE id = ?1",
                params![id],
                row_to_workspace,
            )
            .optional()
            .map_err(err)
    }

    pub fn add_workspace(&self, name: &str, path: &str) -> Result<WorkspaceDto, String> {
        let p = Path::new(path);
        if !p.is_dir() {
            return Err(format!("路径不是有效目录: {path}"));
        }
        let normalized = normalize_path(p);
        let name = if name.trim().is_empty() {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("未命名工作区")
                .to_string()
        } else {
            name.trim().to_string()
        };
        let now = now_rfc3339();
        self.conn
            .execute(
                "INSERT INTO workspaces (name, path, created_at, last_opened_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![name, normalized, now, now],
            )
            .map_err(err)?;
        let id = self.conn.last_insert_rowid();
        self.get_workspace(id)?
            .ok_or_else(|| "工作区创建后无法读取".to_string())
    }

    pub fn remove_workspace(&self, id: i64) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        tx.execute(
            "DELETE FROM documents_fts WHERE rowid IN (SELECT id FROM documents WHERE workspace_id = ?1)",
            params![id],
        )
        .map_err(err)?;
        tx.execute("DELETE FROM documents WHERE workspace_id = ?1", params![id])
            .map_err(err)?;
        tx.execute("DELETE FROM workspaces WHERE id = ?1", params![id])
            .map_err(err)?;
        tx.execute(
            "DELETE FROM app_settings WHERE key = 'active_workspace_id' AND value = ?1",
            params![id.to_string()],
        )
        .map_err(err)?;
        tx.commit().map_err(err)
    }

    pub fn set_workspace_opened(&self, id: i64) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE workspaces SET last_opened_at = ?1 WHERE id = ?2",
                params![now_rfc3339(), id],
            )
            .map_err(err)?;
        Ok(())
    }

    // -- settings -----------------------------------------------------------

    pub fn get_active_workspace_id(&self) -> Result<Option<i64>, String> {
        let v: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'active_workspace_id'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        Ok(v.and_then(|s| s.parse().ok()))
    }

    pub fn set_active_workspace_id(&self, id: i64) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO app_settings (key, value) VALUES ('active_workspace_id', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![id.to_string()],
            )
            .map_err(err)?;
        self.set_workspace_opened(id)
    }

    // -- document indexing (incremental scan) -------------------------------

    /// Incrementally index every `.md` file under the workspace root.
    /// New files are inserted, changed files (mtime differs) re-indexed,
    /// records whose file disappeared are removed.
    pub fn scan_workspace(&self, workspace_id: i64) -> Result<ScanResultDto, String> {
        let Some(ws) = self.get_workspace(workspace_id)? else {
            return Err("工作区不存在".to_string());
        };
        let root = PathBuf::from(&ws.path);

        let mut files = Vec::new();
        collect_md_files(&root, &mut files);

        let existing = self.indexed_docs(workspace_id)?; // normalized path -> (id, mtime)

        let mut result = ScanResultDto::default();
        let tx = self.conn.unchecked_transaction().map_err(err)?;

        for file in &files {
            let normalized = normalize_path(file);
            let meta = match std::fs::metadata(file) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = mtime_ns(&meta);
            let size = meta.len() as i64;

            if let Some(&(id, old_mtime)) = existing.get(&normalized) {
                if old_mtime == mtime {
                    continue;
                }
                let content = match std::fs::read_to_string(file) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let (title, source, tags, body) = extract_meta(&content, file);
                tx.execute(
                    "UPDATE documents SET title = ?1, file_size = ?2, mtime = ?3, source = ?4
                     WHERE id = ?5",
                    params![title, size, mtime, source, id],
                )
                .map_err(err)?;
                tx.execute(
                    "UPDATE documents_fts SET title = ?1, body = ?2, tags = ?3 WHERE rowid = ?4",
                    params![
                        cjk_bigram(&title),
                        cjk_bigram(&body),
                        cjk_bigram(&tags.join(" ")),
                        id
                    ],
                )
                .map_err(err)?;
                result.updated += 1;
            } else {
                let content = match std::fs::read_to_string(file) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let (title, source, tags, body) = extract_meta(&content, file);
                tx.execute(
                    "INSERT INTO documents
                         (workspace_id, path, title, file_size, mtime, favorite, source, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)",
                    params![workspace_id, normalized, title, size, mtime, source, now_rfc3339()],
                )
                .map_err(err)?;
                let id = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO documents_fts (rowid, title, body, tags) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        id,
                        cjk_bigram(&title),
                        cjk_bigram(&body),
                        cjk_bigram(&tags.join(" "))
                    ],
                )
                .map_err(err)?;
                result.indexed += 1;
            }
        }

        for (normalized, (id, _)) in &existing {
            if !Path::new(normalized).exists() {
                tx.execute("DELETE FROM documents_fts WHERE rowid = ?1", params![id])
                    .map_err(err)?;
                tx.execute("DELETE FROM documents WHERE id = ?1", params![id])
                    .map_err(err)?;
                result.removed += 1;
            }
        }

        tx.commit().map_err(err)?;
        result.total = self.doc_count(workspace_id)?;
        Ok(result)
    }

    fn indexed_docs(&self, workspace_id: i64) -> Result<BTreeMap<String, (i64, i64)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, mtime FROM documents WHERE workspace_id = ?1")
            .map_err(err)?;
        let rows = stmt
            .query_map(params![workspace_id], |r| {
                Ok((r.get::<_, String>(1)?, (r.get::<_, i64>(0)?, r.get::<_, i64>(2)?)))
            })
            .map_err(err)?
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    fn doc_count(&self, workspace_id: i64) -> Result<usize, String> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM documents WHERE workspace_id = ?1",
                params![workspace_id],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n as usize)
            .map_err(err)
    }

    // -- document lookups ---------------------------------------------------

    pub fn list_documents(
        &self,
        workspace_id: i64,
        folder: Option<&str>,
    ) -> Result<Vec<DocumentDto>, String> {
        let root = self.workspace_root(workspace_id)?;
        let target = normalize_folder(folder);
        Ok(self
            .all_docs(workspace_id)?
            .into_iter()
            .filter(|d| parent_of_rel(&d.path, &root) == target)
            .collect())
    }

    pub fn list_subfolders(
        &self,
        workspace_id: i64,
        folder: Option<&str>,
    ) -> Result<Vec<FolderDto>, String> {
        let root = self.workspace_root(workspace_id)?;
        let target = normalize_folder(folder);
        let base = if target.is_empty() {
            PathBuf::from(&root)
        } else {
            PathBuf::from(&root).join(&target)
        };
        let mut counts: BTreeMap<String, i64> = BTreeMap::new();
        // Prefix identifying "any file under `target`", e.g. "" (root) or "sub/".
        let prefix = if target.is_empty() {
            String::new()
        } else {
            format!("{target}/")
        };
        for doc in self.all_docs(workspace_id)? {
            let Some(rel) = rel_of(&doc.path, &root) else {
                continue;
            };
            if !prefix.is_empty() && !rel.starts_with(&prefix) {
                continue;
            }
            // First path segment *after* the target folder; only real
            // directories count as subfolders (not direct files).
            let rest = if prefix.is_empty() {
                rel.as_str()
            } else {
                &rel[prefix.len()..]
            };
            if let Some(first) = rest.split('/').next().filter(|s| !s.is_empty()) {
                if base.join(first).is_dir() {
                    *counts.entry(first.to_string()).or_insert(0) += 1;
                }
            }
        }
        Ok(counts
            .into_iter()
            .map(|(name, doc_count)| FolderDto {
                path: if target.is_empty() {
                    name.clone()
                } else {
                    format!("{target}/{name}")
                },
                name,
                doc_count,
            })
            .collect())
    }

    pub fn list_favorites(&self, workspace_id: i64) -> Result<Vec<DocumentDto>, String> {
        self.all_docs(workspace_id)?
            .into_iter()
            .filter(|d| d.favorite)
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    pub fn list_recent(&self, limit: i64) -> Result<Vec<DocumentDto>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, workspace_id, path, title, file_size, favorite, source, created_at, opened_at
                 FROM documents WHERE opened_at IS NOT NULL
                 ORDER BY opened_at DESC LIMIT ?1",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![limit], row_to_document)
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE documents SET favorite = ?1 WHERE id = ?2",
                params![favorite as i64, id],
            )
            .map_err(err)?;
        Ok(())
    }

    pub fn record_open(&self, id: i64) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE documents SET opened_at = ?1 WHERE id = ?2",
                params![now_rfc3339(), id],
            )
            .map_err(err)?;
        Ok(())
    }

    // -- full-text search ----------------------------------------------------

    /// FTS5 search over the active workspace. Queries are CJK-bigram-tokenized
    /// on the Rust side, so multi-character Chinese terms match naturally.
    pub fn search(
        &self,
        query: &str,
        workspace_id: i64,
        limit: i64,
    ) -> Result<Vec<SearchHitDto>, String> {
        let tokens = query_tokens(query);
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        // Quote every token (they are already sanitized to alphanumerics/CJK)
        // and join with space => AND semantics.
        let match_expr = tokens
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = self
            .conn
            .prepare(
                "SELECT d.id, d.workspace_id, d.path, d.title, d.file_size,
                        d.favorite, d.source, d.created_at, d.opened_at,
                        snippet(documents_fts, 1, '<mark>', '</mark>', '…', 24)
                 FROM documents_fts
                 JOIN documents d ON d.id = documents_fts.rowid
                 WHERE documents_fts MATCH ?1 AND d.workspace_id = ?2
                 ORDER BY rank
                 LIMIT ?3",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![match_expr, workspace_id, limit], |r| {
                let doc = DocumentDto {
                    id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    path: r.get(2)?,
                    title: r.get(3)?,
                    file_size: r.get(4)?,
                    favorite: r.get::<_, i64>(5)? != 0,
                    source: r.get(6)?,
                    created_at: r.get(7)?,
                    opened_at: r.get(8)?,
                };
                let raw: String = r.get(9)?;
                Ok(SearchHitDto {
                    document: doc,
                    snippet: Some(clean_cjk_spaces(&raw)),
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    // -- batch tasks ---------------------------------------------------------

    pub fn insert_batch_task(
        &self,
        id: &str,
        source_path: &str,
        output_path: &str,
        output_mode: &OutputMode,
        parse_quality: &ParseQuality,
        created_at: u64,
    ) -> Result<(), String> {
        let mode = serde_json::to_string(output_mode).unwrap_or_else(|_| "\"aiReady\"".to_string());
        let quality =
            serde_json::to_string(parse_quality).unwrap_or_else(|_| "\"auto\"".to_string());
        self.conn
            .execute(
                "INSERT INTO batch_tasks (id, source_path, output_path, status, created_at, output_mode, parse_quality)
                 VALUES (?1, ?2, ?3, 'Pending', ?4, ?5, ?6)",
                rusqlite::params![id, source_path, output_path, created_at, mode, quality],
            )
            .map_err(err)?;
        Ok(())
    }

    pub fn update_batch_task_status(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
        elapsed_secs: u64,
    ) -> Result<(), String> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let completed_at: Option<u64> = match status {
            "Completed" | "Failed" | "Cancelled" => Some(now),
            _ => None,
        };
        self.conn
            .execute(
                "UPDATE batch_tasks SET status = ?1, error = ?2, elapsed_secs = ?3, completed_at = ?4
                 WHERE id = ?5",
                rusqlite::params![status, error, elapsed_secs, completed_at, id],
            )
            .map_err(err)?;
        Ok(())
    }

    pub fn list_batch_tasks(
        &self,
        status: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<BatchTaskDto>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, source_path, output_path, status, progress, stage, error,
                        created_at, completed_at, elapsed_secs, output_mode, parse_quality, retry_count
                 FROM batch_tasks
                 WHERE status = ?1
                 ORDER BY created_at ASC
                 LIMIT ?2 OFFSET ?3",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(rusqlite::params![status, limit, offset], row_to_batch_task)
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn list_all_batch_tasks(&self) -> Result<Vec<BatchTaskDto>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, source_path, output_path, status, progress, stage, error,
                        created_at, completed_at, elapsed_secs, output_mode, parse_quality, retry_count
                 FROM batch_tasks
                 ORDER BY created_at ASC",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], row_to_batch_task)
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn delete_batch_tasks(&self, status: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM batch_tasks WHERE status = ?1", rusqlite::params![status])
            .map_err(err)?;
        Ok(())
    }

    /// Return the id of an existing non-terminal task for the same source path,
    /// if one exists. Used to deduplicate the batch queue so a file dropped
    /// multiple times (or duplicate drag-drop events) cannot create an endless
    /// stream of identical tasks.
    pub fn find_active_batch_task_by_source(
        &self,
        source_path: &str,
    ) -> Result<Option<String>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id FROM batch_tasks
                 WHERE source_path = ?1 AND status IN ('Pending', 'Processing', 'Paused')
                 LIMIT 1",
            )
            .map_err(err)?;
        let id: Option<String> = stmt
            .query_row(rusqlite::params![source_path], |r| r.get(0))
            .optional()
            .map_err(err)?;
        Ok(id)
    }

    pub fn get_batch_summary(&self) -> Result<BatchSummaryDto, String> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM batch_tasks", [], |r| r.get(0))
            .map_err(err)?;
        let pending: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM batch_tasks WHERE status = 'Pending'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let processing: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM batch_tasks WHERE status = 'Processing'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let completed: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM batch_tasks WHERE status = 'Completed'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let failed: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM batch_tasks WHERE status = 'Failed'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let cancelled: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM batch_tasks WHERE status = 'Cancelled'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let paused: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM batch_tasks WHERE status = 'Paused'",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        Ok(BatchSummaryDto {
            total: total as u64,
            pending: pending as u64,
            processing: processing as u64,
            completed: completed as u64,
            failed: failed as u64,
            cancelled: cancelled as u64,
            paused: paused as u64,
        })
    }

    pub fn get_batch_task_created_at(&self, id: &str) -> Result<Option<u64>, String> {
        self.conn
            .query_row(
                "SELECT created_at FROM batch_tasks WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map_err(err)
            .map(|opt| opt.map(|v| v as u64))
    }

    // -- helpers --------------------------------------------------------------

    fn workspace_root(&self, workspace_id: i64) -> Result<String, String> {
        self.get_workspace(workspace_id)?
            .map(|w| w.path)
            .ok_or_else(|| "工作区不存在".to_string())
    }

    fn all_docs(&self, workspace_id: i64) -> Result<Vec<DocumentDto>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, workspace_id, path, title, file_size, favorite, source, created_at, opened_at
                 FROM documents WHERE workspace_id = ?1 ORDER BY title COLLATE NOCASE",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![workspace_id], row_to_document)
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }
}

// ---------------------------------------------------------------------------
// Row mappers / small helpers
// ---------------------------------------------------------------------------

fn err(e: rusqlite::Error) -> String {
    format!("数据库错误: {e}")
}

fn row_to_workspace(r: &rusqlite::Row) -> rusqlite::Result<WorkspaceDto> {
    Ok(WorkspaceDto {
        id: r.get(0)?,
        name: r.get(1)?,
        path: r.get(2)?,
        created_at: r.get(3)?,
        last_opened_at: r.get(4)?,
    })
}

fn row_to_batch_task(r: &rusqlite::Row) -> rusqlite::Result<BatchTaskDto> {
    let output_mode_str: String = r.get(10)?;
    let parse_quality_str: String = r.get(11)?;
    let output_mode: OutputMode = serde_json::from_str(&output_mode_str).unwrap_or(OutputMode::AiReady);
    let parse_quality: ParseQuality =
        serde_json::from_str(&parse_quality_str).unwrap_or(ParseQuality::Auto);
    Ok(BatchTaskDto {
        id: r.get(0)?,
        source_path: r.get(1)?,
        output_path: r.get(2)?,
        status: r.get(3)?,
        progress: r.get(4)?,
        stage: r.get(5)?,
        error: r.get(6)?,
        created_at: r.get::<_, i64>(7)? as u64,
        completed_at: r.get::<_, Option<i64>>(8)?.map(|v| v as u64),
        elapsed_secs: r.get::<_, i64>(9)? as u64,
        output_mode,
        parse_quality,
        retry_count: r.get::<_, i32>(12)? as u32,
    })
}

fn row_to_document(r: &rusqlite::Row) -> rusqlite::Result<DocumentDto> {
    Ok(DocumentDto {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        path: r.get(2)?,
        title: r.get(3)?,
        file_size: r.get(4)?,
        favorite: r.get::<_, i64>(5)? != 0,
        source: r.get(6)?,
        created_at: r.get(7)?,
        opened_at: r.get(8)?,
    })
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

fn now_rfc3339() -> String {
    Local::now().to_rfc3339()
}

fn mtime_ns(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Canonical, forward-slash normalized absolute path (stable lookup key).
fn normalize_path(p: &Path) -> String {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    abs.canonicalize()
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

fn rel_of(path: &str, root: &str) -> Option<String> {
    Path::new(path)
        .strip_prefix(Path::new(root))
        .ok()
        .map(|r| r.to_string_lossy().replace('\\', "/"))
}

/// Direct parent directory of `path` relative to `root`, or "" for root level.
fn parent_of_rel(path: &str, root: &str) -> String {
    rel_of(path, root)
        .and_then(|rel| {
            Path::new(&rel)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .filter(|p| !p.is_empty() && *p != ".")
        .unwrap_or_default()
}

fn normalize_folder(folder: Option<&str>) -> String {
    folder
        .map(|f| {
            f.trim()
                .trim_start_matches("./")
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string()
        })
        .unwrap_or_default()
}

/// Recursively collect every `*.md` file, skipping dot-directories.
fn collect_md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            collect_md_files(&path, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

/// Parse light frontmatter (title / source / tags) out of a markdown file.
fn extract_meta(content: &str, path: &Path) -> (String, Option<String>, Vec<String>, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未命名文档")
        .to_string();
    let body = strip_frontmatter(content);
    let mut title = None;
    let mut source = None;
    let mut tags = Vec::new();

    let rest = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"));
    if let Some(rest) = rest {
        let end = rest.find("\n---").or_else(|| rest.find("\r\n---"));
        if let Some(end) = end {
            for line in rest[..end].lines() {
                let line = line.trim();
                let Some((k, v)) = line.split_once(':') else {
                    continue;
                };
                match k.trim() {
                    "title" => title = Some(v.trim().trim_matches('"').trim_matches('\'').to_string()),
                    "source" => source = Some(v.trim().trim_matches('"').trim_matches('\'').to_string()),
                    "tags" => {
                        let v = v.trim();
                        tags = if v.starts_with('[') && v.ends_with(']') {
                            v[1..v.len() - 1]
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect()
                        } else if !v.is_empty() {
                            v.split_whitespace().map(|s| s.to_string()).collect()
                        } else {
                            Vec::new()
                        };
                    }
                    _ => {}
                }
            }
        }
    }

    (title.unwrap_or(stem), source, tags, body.to_string())
}

/// Strip a leading `---` frontmatter block.
fn strip_frontmatter(content: &str) -> &str {
    let Some(rest) = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    else {
        return content;
    };
    match rest.find("\n---").or_else(|| rest.find("\r\n---")) {
        Some(end) => &rest[end..],
        None => content,
    }
}

// ---------------------------------------------------------------------------
// CJK bigram tokenization for FTS5
// ---------------------------------------------------------------------------

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3400}'..='\u{4DBF}' | // Extension A
        '\u{4E00}'..='\u{9FFF}' | // Unified Ideographs
        '\u{F900}'..='\u{FAFF}' | // Compatibility Ideographs
        '\u{20000}'..='\u{2A6DF}' | // Extension B
        '\u{2F800}'..='\u{2FA1F}' | // Compatibility Supplement
        '\u{3040}'..='\u{30FF}' | // Hiragana + Katakana
        '\u{31F0}'..='\u{31FF}' | // Katakana Phonetic Ext.
        '\u{AC00}'..='\u{D7AF}'   // Hangul syllables
    )
}

/// Emit CJK bigrams (adjacent character pairs) for a continuous CJK run.
fn flush_cjk(buf: &mut Vec<char>, tokens: &mut Vec<String>) {
    if buf.len() >= 2 {
        for w in buf.windows(2) {
            tokens.push(w.iter().collect());
        }
    }
    buf.clear();
}

/// Pre-tokenize text so FTS5 (unicode61) can index/query Chinese:
/// CJK runs -> bigrams, ASCII runs -> words.
fn cjk_bigram(text: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();
    let mut cjk_buf: Vec<char> = Vec::new();
    let mut ascii_buf: Vec<char> = Vec::new();

    for c in text.chars() {
        if is_cjk(c) {
            flush_ascii(&mut ascii_buf, &mut tokens);
            cjk_buf.push(c);
        } else if c.is_ascii_alphanumeric() {
            flush_cjk(&mut cjk_buf, &mut tokens);
            ascii_buf.push(c);
        } else {
            flush_cjk(&mut cjk_buf, &mut tokens);
            flush_ascii(&mut ascii_buf, &mut tokens);
        }
    }
    flush_cjk(&mut cjk_buf, &mut tokens);
    flush_ascii(&mut ascii_buf, &mut tokens);

    tokens.join(" ")
}

fn flush_ascii(buf: &mut Vec<char>, tokens: &mut Vec<String>) {
    if !buf.is_empty() {
        tokens.push(buf.iter().collect());
        buf.clear();
    }
}

/// Query-side tokenization: CJK bigrams + ASCII words with length >= 2.
fn query_tokens(query: &str) -> Vec<String> {
    cjk_bigram(query)
        .split_whitespace()
        .filter(|t| t.chars().any(is_cjk) || t.chars().count() >= 2)
        .map(|s| s.to_string())
        .collect()
}

/// Remove spaces inserted between adjacent CJK characters in FTS5 snippets.
fn clean_cjk_spaces(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, c) in chars.iter().enumerate() {
        if *c == ' ' {
            let prev_cjk = i > 0 && is_cjk(chars[i - 1]);
            let next_cjk = i + 1 < chars.len() && is_cjk(chars[i + 1]);
            if prev_cjk && next_cjk {
                continue;
            }
        }
        out.push(*c);
    }
    out
}

// ---------------------------------------------------------------------------
// Tauri command glue
// ---------------------------------------------------------------------------

/// Locked, lazily-initialized handle to the workspace DB.
///
/// The connection lives in a process-wide singleton so the returned guard does
/// not borrow through Tauri's `State` (whose Deref chain would tie the guard
/// lifetime to a local variable). Desktop apps are single-instance, so a
/// process-wide DB is the correct scope.
pub fn db(app: &tauri::AppHandle) -> Result<WorkspaceDbHandle<'static>, String> {
    let lock: &'static Mutex<Option<WorkspaceDb>> = GLOBAL_DB.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().map_err(|_| "数据库锁异常".to_string())?;
    if guard.is_none() {
        *guard = Some(WorkspaceDb::open_in_app_data(app)?);
    }
    Ok(WorkspaceDbHandle { guard })
}

static GLOBAL_DB: OnceLock<Mutex<Option<WorkspaceDb>>> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn cjk_bigram_tokenizes() {
        assert_eq!(cjk_bigram("支持中文"), "支持 持中 中文");
        assert_eq!(cjk_bigram("Hello 世界 World"), "Hello 世界 World");
        assert_eq!(cjk_bigram(""), "");
        // '-' is a tokenizer separator (same as unicode61), not kept in words
        assert_eq!(cjk_bigram("状态-of-the-art"), "状态 of the art");
        assert_eq!(query_tokens("人工智能"), vec!["人工", "工智", "智能"]);
        // 2+ char ASCII words are kept, single chars are dropped
        assert_eq!(query_tokens("ai"), vec!["ai"]);
        assert_eq!(query_tokens("a"), Vec::<String>::new());
    }

    #[test]
    fn workspace_lifecycle_and_search() {
        let dir = std::env::temp_dir().join(format!("omnimd_db_test_{}", std::process::id()));
        let db_path = dir.join("test.db");
        fs::create_dir_all(&dir).unwrap();

        let db = WorkspaceDb::open(&db_path).unwrap();

        let ws_dir = dir.join("docs");
        fs::create_dir_all(ws_dir.join("sub")).unwrap();
        fs::write(
            ws_dir.join("a.md"),
            "---\ntitle: 人工智能报告\nsource: https://example.com/a\n---\n人工智能在医疗文档中的应用。\n",
        )
        .unwrap();
        fs::write(ws_dir.join("sub/b.md"), "# 医疗影像\n影像分析技术概述。\n").unwrap();

        let ws = db.add_workspace("测试库", ws_dir.to_str().unwrap()).unwrap();
        let scan = db.scan_workspace(ws.id).unwrap();
        assert_eq!(scan.indexed, 2);
        assert_eq!(scan.total, 2);

        // 二次扫描应为增量无变更
        let scan = db.scan_workspace(ws.id).unwrap();
        assert_eq!(scan.indexed, 0);
        assert_eq!(scan.updated, 0);

        // 中文 bigram 搜索（a.md：frontmatter title/source 应被解析）
        let hits = db.search("人工智能", ws.id, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document.title, "人工智能报告");
        assert_eq!(
            hits[0].document.source.as_deref(),
            Some("https://example.com/a")
        );
        // Snippet wraps each matched bigram token in <mark>; the CJK bigrams
        // are separated by tags so assert on the highlight markers instead.
        let snippet = hits[0].snippet.as_deref().unwrap_or("");
        assert!(snippet.contains("<mark>人工</mark>"), "snippet: {snippet}");
        assert!(snippet.contains("<mark>智能</mark>"), "snippet: {snippet}");

        // 单子目录中的文档可被跨目录搜索
        let hits = db.search("影像", ws.id, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].document.path.ends_with("b.md"));

        // 收藏 + 最近
        db.set_favorite(hits[0].document.id, true).unwrap();
        let favs = db.list_favorites(ws.id).unwrap();
        assert_eq!(favs.len(), 1);
        db.record_open(hits[0].document.id).unwrap();
        let recent = db.list_recent(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, hits[0].document.id);

        // 目录浏览
        let folders = db.list_subfolders(ws.id, None).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "sub");
        assert_eq!(folders[0].doc_count, 1);
        assert_eq!(db.list_documents(ws.id, None).unwrap().len(), 1);
        assert_eq!(db.list_documents(ws.id, Some("sub")).unwrap().len(), 1);

        // 删除工作区（级联清理文档 + FTS）
        db.remove_workspace(ws.id).unwrap();
        assert!(db.get_workspace(ws.id).unwrap().is_none());
        assert_eq!(db.search("人工智能", ws.id, 10).unwrap().len(), 0);

        fs::remove_dir_all(&dir).ok();
    }
}

/// RAII handle owning the DB mutex; derefs to the initialized database.
pub struct WorkspaceDbHandle<'a> {
    guard: MutexGuard<'a, Option<WorkspaceDb>>,
}

impl Deref for WorkspaceDbHandle<'_> {
    type Target = WorkspaceDb;
    fn deref(&self) -> &WorkspaceDb {
        self.guard.as_ref().expect("db initialized")
    }
}

impl DerefMut for WorkspaceDbHandle<'_> {
    fn deref_mut(&mut self) -> &mut WorkspaceDb {
        self.guard.as_mut().expect("db initialized")
    }
}
