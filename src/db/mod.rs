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

use std::collections::{BTreeMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::SystemTime;

use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::models::task::{BatchTaskDto, BatchSummaryDto};
use crate::text_utils::is_cjk;

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
-- 用于目录浏览（list_documents/list_subfolders）与迁移冲突探针的路径范围扫描。
CREATE INDEX IF NOT EXISTS idx_documents_ws_path   ON documents(workspace_id, path);

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
-- Speeds up the enqueue dedup lookup (source_path + status).
CREATE INDEX IF NOT EXISTS idx_batch_tasks_source_status ON batch_tasks(source_path, status);
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
        db.migrate_legacy_paths()?;
        Ok(db)
    }

    /// One-time cleanup for rows indexed before `normalize_path` stopped
    /// storing Windows extended-length prefixes: `//?/D:/…` paths are not
    /// valid for later `fs` calls (OS error 123) and must be rewritten to
    /// plain drive paths. Idempotent — the WHERE clause matches nothing once
    /// every row has been fixed.
    fn migrate_legacy_paths(&self) -> rusqlite::Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, path FROM documents WHERE path LIKE '//?/%'",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        if rows.is_empty() {
            return Ok(());
        }
        for (id, workspace_id, bad) in rows {
            let fixed = strip_extended_prefix(&bad);
            // `UNIQUE(workspace_id, path)`：若同 workspace 下已有同 path 的行，
            // 直接 UPDATE 会让迁移在每次 open 时失败，数据库从此打不开。
            // 此时旧行是重复索引，删除（连同 FTS 行）而不是改写。
            let conflict: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM documents WHERE workspace_id = ?1 AND path = ?2 AND id != ?3",
                    params![workspace_id, fixed, id],
                    |r| r.get(0),
                )
                .optional()?;
            if conflict.is_some() {
                self.conn
                    .execute("DELETE FROM documents_fts WHERE rowid = ?1", params![id])?;
                self.conn
                    .execute("DELETE FROM documents WHERE id = ?1", params![id])?;
            } else {
                self.conn
                    .execute("UPDATE documents SET path = ?1 WHERE id = ?2", params![fixed, id])?;
            }
        }
        Ok(())
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
    ///
    /// Convenience wrapper used by tests; the app calls
    /// [`scan_workspace_background`] instead, which keeps DB lock scopes short
    /// so a huge root (an entire drive) cannot starve every other command.
    pub fn scan_workspace(&self, workspace_id: i64) -> Result<ScanResultDto, String> {
        let (root, existing) = self.load_scan_baseline(workspace_id)?;
        let mut result = ScanResultDto::default();
        build_scan_plan(&root, &existing, |_| {}, |batch| {
            let (updated, indexed, removed) =
                self.apply_scan_batch(workspace_id, &batch.updates, &batch.inserts, &batch.removals)?;
            result.updated += updated;
            result.indexed += indexed;
            result.removed += removed;
            Ok(())
        })?;
        result.total = self.doc_count(workspace_id)?;
        Ok(result)
    }

    /// Phase A of a scan: load the workspace row and the current index
    /// (path -> (id, mtime)). Callers must drop the DB handle right after so
    /// the filesystem walk in phase B runs WITHOUT holding the global lock.
    pub fn load_scan_baseline(
        &self,
        workspace_id: i64,
    ) -> Result<(PathBuf, BTreeMap<String, (i64, i64, i64)>), String> {
        let Some(ws) = self.get_workspace(workspace_id)? else {
            return Err("工作区不存在".to_string());
        };
        let existing = self.indexed_docs(workspace_id)?;
        Ok((PathBuf::from(&ws.path), existing))
    }

    /// Phase C of a scan: apply a prepared plan in ONE short transaction.
    pub fn apply_scan_plan(
        &self,
        workspace_id: i64,
        existing: &BTreeMap<String, (i64, i64, i64)>,
        plan: ScanPlan,
    ) -> Result<ScanResultDto, String> {
        let mut result = ScanResultDto::default();
        let (updated, indexed, removed) =
            self.apply_scan_batch(workspace_id, &plan.updates, &plan.inserts, &plan.removals)?;
        result.updated = updated;
        result.indexed = indexed;
        result.removed = removed;
        let _ = existing;
        result.total = self.doc_count(workspace_id)?;
        Ok(result)
    }

    /// 在一个短事务内写入一批扫描记录，返回 (updated, indexed, removed)。
    /// 供 [`apply_scan_plan`] 与分批 flush（`build_scan_plan` 的回调）共用，
    /// 避免把最多 10 万个文件的 bigram 全部堆在内存里最后才刷库。
    fn apply_scan_batch(
        &self,
        workspace_id: i64,
        updates: &[ScanUpdate],
        inserts: &[ScanInsert],
        removals: &[i64],
    ) -> Result<(usize, usize, usize), String> {
        let tx = self.conn.unchecked_transaction().map_err(err)?;

        for u in updates {
            if u.update_content {
                tx.execute(
                    "UPDATE documents SET title = ?1, file_size = ?2, mtime = ?3, source = ?4
                     WHERE id = ?5",
                    params![u.title, u.size, u.mtime, u.source, u.id],
                )
                .map_err(err)?;
                // `INSERT OR REPLACE` also repairs a missing FTS row, so the
                // index cannot silently drift out of sync with `documents`.
                tx.execute(
                    "INSERT OR REPLACE INTO documents_fts (rowid, title, body, tags)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![u.id, u.title_bigram, u.body_bigram, u.tags_bigram],
                )
                .map_err(err)?;
            } else {
                // Oversized file: refresh only cheap metadata and leave the
                // existing title/source and full-text index untouched.
                tx.execute(
                    "UPDATE documents SET file_size = ?1, mtime = ?2 WHERE id = ?3",
                    params![u.size, u.mtime, u.id],
                )
                .map_err(err)?;
            }
        }

        for d in inserts {
            tx.execute(
                "INSERT INTO documents
                     (workspace_id, path, title, file_size, mtime, favorite, source, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)",
                params![
                    workspace_id,
                    d.normalized,
                    d.title,
                    d.size,
                    d.mtime,
                    d.source,
                    now_rfc3339()
                ],
            )
            .map_err(err)?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO documents_fts (rowid, title, body, tags) VALUES (?1, ?2, ?3, ?4)",
                params![id, d.title_bigram, d.body_bigram, d.tags_bigram],
            )
            .map_err(err)?;
        }

        for id in removals {
            tx.execute("DELETE FROM documents_fts WHERE rowid = ?1", params![id])
                .map_err(err)?;
            tx.execute("DELETE FROM documents WHERE id = ?1", params![id])
                .map_err(err)?;
        }

        tx.commit().map_err(err)?;
        Ok((updates.len(), inserts.len(), removals.len()))
    }

    fn indexed_docs(&self, workspace_id: i64) -> Result<BTreeMap<String, (i64, i64, i64)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, mtime, file_size FROM documents WHERE workspace_id = ?1")
            .map_err(err)?;
        let rows = stmt
            .query_map(params![workspace_id], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    (r.get::<_, i64>(0)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?),
                ))
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
        // SQL 级过滤：path 落在 `root/target/` 前缀内的所有后代文档（递归，
        // 与树上 doc_count 徽章的语义一致——点进文件夹应看到徽章数字那么多
        // 文档）。用范围扫描而非 LIKE，路径里的 %/_ 不会干扰匹配；大工作区
        // 不再整表载入内存过滤。
        let prefix = folder_prefix(&root, &target);
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, workspace_id, path, title, file_size, favorite, source, created_at, opened_at
                 FROM documents
                 WHERE workspace_id = ?1 AND path >= ?2 || '/' AND path < ?2 || '0'
                 ORDER BY title COLLATE NOCASE",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![workspace_id, prefix], row_to_document)
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn list_subfolders(
        &self,
        workspace_id: i64,
        folder: Option<&str>,
    ) -> Result<Vec<FolderDto>, String> {
        let root = self.workspace_root(workspace_id)?;
        let target = normalize_folder(folder);
        // 与 list_documents 相同的前缀范围，但按"target 之下的第一段路径"
        // 分组计数。只统计嵌套段（剩余部分含 '/'），与旧的内存过滤语义一致。
        let prefix = folder_prefix(&root, &target);
        // SQLite substr() 按字符计数，而 str::len() 是字节数——工作区路径含
        // 中文时（CJK 每 char 3 bytes）按字节算偏移会切进第一段目录名，
        // "content-main" 会被截成 "main"。必须用字符数。
        let offset = prefix.chars().count() as i64 + 2;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT substr(rel, 1, instr(rel, '/') - 1) AS name, COUNT(*) AS cnt
                 FROM (
                     SELECT substr(path, ?3) AS rel
                     FROM documents
                     WHERE workspace_id = ?1 AND path >= ?2 || '/' AND path < ?2 || '0'
                 )
                 WHERE instr(rel, '/') > 0
                 GROUP BY name ORDER BY name",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![workspace_id, prefix, offset], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows
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
        // Filter in SQL so `idx_documents_favorite` is actually used instead of
        // loading every document and filtering in memory.
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, workspace_id, path, title, file_size, favorite, source, created_at, opened_at
                 FROM documents
                 WHERE workspace_id = ?1 AND favorite = 1
                 ORDER BY title COLLATE NOCASE",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![workspace_id], row_to_document)
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn list_recent(&self, workspace_id: Option<i64>, limit: i64) -> Result<Vec<DocumentDto>, String> {
        let mut stmt = if workspace_id.is_some() {
            self.conn
                .prepare(
                    "SELECT id, workspace_id, path, title, file_size, favorite, source, created_at, opened_at
                     FROM documents WHERE opened_at IS NOT NULL AND workspace_id = ?1
                     ORDER BY opened_at DESC LIMIT ?2",
                )
                .map_err(err)?
        } else {
            self.conn
                .prepare(
                    "SELECT id, workspace_id, path, title, file_size, favorite, source, created_at, opened_at
                     FROM documents WHERE opened_at IS NOT NULL
                     ORDER BY opened_at DESC LIMIT ?1",
                )
                .map_err(err)?
        };
        let rows = match workspace_id {
            Some(ws_id) => stmt
                .query_map(params![ws_id, limit], row_to_document)
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?,
            None => stmt
                .query_map(params![limit], row_to_document)
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?,
        };
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

    /// Search over the active workspace. Content goes through FTS5 (queries
    /// are CJK-bigram-tokenized on the Rust side); the document title (file
    /// stem) is matched as a plain substring. Title hits rank above content
    /// hits; a document matching both keeps its content snippet.
    pub fn search(
        &self,
        query: &str,
        workspace_id: i64,
        limit: i64,
    ) -> Result<Vec<SearchHitDto>, String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let tokens = query_tokens(query);

        // -- content hits (FTS) -------------------------------------------
        // Quote every token (they are already sanitized to alphanumerics/CJK)
        // and join with space => AND semantics. Tokens may be empty (e.g. a
        // single ASCII letter is not bigram material) — the FTS pass is then
        // skipped but the title pass below still runs.
        let mut content_hits: Vec<SearchHitDto> = Vec::new();
        if !tokens.is_empty() {
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
                            snippet(documents_fts, -1, ?4, ?5, '…', 24)
                     FROM documents_fts
                     JOIN documents d ON d.id = documents_fts.rowid
                     WHERE documents_fts MATCH ?1 AND d.workspace_id = ?2
                     ORDER BY rank
                     LIMIT ?3",
                )
                .map_err(err)?;
            // Sentinel markers instead of literal `<mark>` so the snippet text can
            // be HTML-escaped first, then have the markers restored. This keeps the
            // frontend's `dangerouslySetInnerHTML` from ever receiving raw document
            // HTML.
            content_hits = stmt
                .query_map(
                    params![match_expr, workspace_id, limit, MARK_OPEN, MARK_CLOSE],
                    |r| {
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
                        snippet: Some(escape_snippet(&raw)),
                    })
                })
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?;
        }

        // -- title hits (file-name substring) ------------------------------
        // Escape LIKE wildcards so "%"/"_" in the query match literally.
        let mut pattern = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        pattern.insert_str(0, "%");
        pattern.push('%');
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, workspace_id, path, title, file_size, favorite, source, created_at, opened_at
                 FROM documents
                 WHERE workspace_id = ?1 AND title LIKE ?2 ESCAPE '\\'
                 ORDER BY title COLLATE NOCASE
                 LIMIT ?3",
            )
            .map_err(err)?;
        let title_hits = stmt
            .query_map(params![workspace_id, pattern, limit], |r| {
                Ok(SearchHitDto {
                    document: DocumentDto {
                        id: r.get(0)?,
                        workspace_id: r.get(1)?,
                        path: r.get(2)?,
                        title: r.get(3)?,
                        file_size: r.get(4)?,
                        favorite: r.get::<_, i64>(5)? != 0,
                        source: r.get(6)?,
                        created_at: r.get(7)?,
                        opened_at: r.get(8)?,
                    },
                    snippet: None,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;

        // -- merge: title hits first, no duplicates, capped at limit --------
        // A document that matches both stays in the title group but keeps its
        // content snippet from the FTS pass; the bare title row is dropped.
        let title_ids: HashSet<i64> = title_hits.iter().map(|h| h.document.id).collect();
        let mut both: Vec<SearchHitDto> = Vec::new();
        let mut rest: Vec<SearchHitDto> = Vec::new();
        for hit in content_hits {
            if title_ids.contains(&hit.document.id) {
                both.push(hit);
            } else {
                rest.push(hit);
            }
        }
        let both_ids: HashSet<i64> = both.iter().map(|h| h.document.id).collect();
        let mut merged: Vec<SearchHitDto> = Vec::with_capacity(both.len() + title_hits.len() + rest.len());
        merged.extend(both);
        merged.extend(
            title_hits
                .into_iter()
                .filter(|h| !both_ids.contains(&h.document.id)),
        );
        merged.extend(rest);
        merged.truncate(limit as usize);
        Ok(merged)
    }

    // -- batch tasks ---------------------------------------------------------

    pub fn insert_batch_task(
        &self,
        id: &str,
        source_path: &str,
        output_path: &str,
        created_at: u64,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO batch_tasks (id, source_path, output_path, status, created_at)
                 VALUES (?1, ?2, ?3, 'Pending', ?4)",
                rusqlite::params![id, source_path, output_path, created_at],
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
                        created_at, completed_at, elapsed_secs, retry_count
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
                        created_at, completed_at, elapsed_secs, retry_count
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

    /// On startup, mark any stale `Processing` tasks as `Failed` — they
    /// were left behind by a previous session that was killed or crashed.
    pub fn reconcile_stale_tasks(&self) -> Result<u64, String> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let count = self
            .conn
            .execute(
                "UPDATE batch_tasks
                 SET status = 'Failed',
                     error = '应用上次关闭时任务未完成',
                     completed_at = ?1
                 WHERE status = 'Processing'",
                rusqlite::params![now],
            )
            .map_err(err)?;
        Ok(count as u64)
    }

    pub fn get_batch_summary(&self) -> Result<BatchSummaryDto, String> {
        // One grouped scan instead of seven full-table COUNTs.
        let mut stmt = self
            .conn
            .prepare("SELECT status, COUNT(*) FROM batch_tasks GROUP BY status")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        let mut summary = BatchSummaryDto {
            total: 0,
            pending: 0,
            processing: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
            paused: 0,
        };
        for (status, count) in rows {
            let count = count as u64;
            summary.total += count;
            match status.as_str() {
                "Pending" => summary.pending = count,
                "Processing" => summary.processing = count,
                "Completed" => summary.completed = count,
                "Failed" => summary.failed = count,
                "Cancelled" => summary.cancelled = count,
                "Paused" => summary.paused = count,
                _ => {}
            }
        }
        Ok(summary)
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
        retry_count: r.get::<_, i32>(10)? as u32,
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
///
/// `canonicalize()` on Windows returns extended-length paths
/// (`\\?\D:\foo\bar.md`); that prefix must be stripped BEFORE converting to
/// forward slashes — the mixed form `//?/D:/…` is not a valid Windows path
/// and every later `fs` call fails with OS error 123 (ERROR_INVALID_NAME).
fn normalize_path(p: &Path) -> String {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    let canonical = abs.canonicalize().unwrap_or(abs);
    strip_extended_prefix(&canonical.to_string_lossy())
}

/// Remove the Windows extended-length / UNC device prefix from a canonical
/// path string (`\\?\C:\x` → `C:\x`, `\\?\UNC\srv\share` → `\\srv\share`),
/// then normalize separators to forward slashes.
fn strip_extended_prefix(path: &str) -> String {
    let stripped = if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    };
    stripped.replace('\\', "/")
}

/// list_documents / list_subfolders 的 SQL 前缀：`root[/target]`（root 去掉
/// 尾部 '/'）。配合 `path >= prefix || '/' AND path < prefix || '0'` 可精确
/// 匹配"位于该目录之下"的所有文档（'0' 是 '/' + 1），不受路径中 `%`/`_`
/// 等 LIKE 通配符影响。
fn folder_prefix(root: &str, target: &str) -> String {
    let root = root.trim_end_matches('/');
    if target.is_empty() {
        root.to_string()
    } else {
        format!("{root}/{target}")
    }
}

fn normalize_folder(folder: Option<&str>) -> String {
    let raw = folder
        .map(|f| {
            f.trim()
                .trim_start_matches("./")
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string()
        })
        .unwrap_or_default();
    // Reject parent-directory escapes: a folder fragment must stay inside the
    // workspace root (this value is later joined to the root for a dir probe).
    if raw.split('/').any(|seg| seg == "..") || raw.contains('\\') {
        return String::new();
    }
    raw
}

/// Recursively collect every `*.md` file, skipping dot-directories.
/// Safety caps so a pathological root (e.g. an entire drive) cannot make the
/// scan run unbounded.
const SCAN_MAX_FILES: usize = 100_000;
const SCAN_MAX_DEPTH: u32 = 48;
/// Files larger than this are indexed by name only; their body is not
/// bigram-tokenized (reading + tokenizing hundreds of MB would stall a pass).
const SCAN_MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;
/// 分批落库的批大小：build_scan_plan 每积累这么多条记录就 flush 一次，
/// 避免整个工作区的 bigram 全部堆在内存里。
const SCAN_FLUSH_BATCH: usize = 500;

/// Directory names that are never worth indexing. Compared lowercased; the
/// dot-prefix rule handles VCS/hidden dirs separately.
const SCAN_SKIP_DIRS: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "node_modules",
    "target",
    "vendor",
    "__pycache__",
];

fn collect_md_files(dir: &Path, depth: u32, budget: &mut usize, out: &mut Vec<PathBuf>) {
    if *budget == 0 || depth > SCAN_MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        // file_type() does NOT follow symlinks/junctions, so reparse-point
        // cycles (common on Windows) can never recurse infinitely.
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            if SCAN_SKIP_DIRS.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                continue;
            }
            collect_md_files(&path, depth + 1, budget, out);
        } else if ft.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
        {
            out.push(path);
            *budget -= 1;
        }
    }
}

/// Prepared diff between the index and the filesystem, built WITHOUT holding
/// the DB lock; [`WorkspaceDb::apply_scan_plan`] flushes it in one transaction.
#[derive(Default)]
pub struct ScanPlan {
    updates: Vec<ScanUpdate>,
    inserts: Vec<ScanInsert>,
    removals: Vec<i64>,
}

struct ScanUpdate {
    id: i64,
    title: String,
    size: i64,
    mtime: i64,
    source: Option<String>,
    title_bigram: String,
    body_bigram: String,
    tags_bigram: String,
    /// When false only `file_size`/`mtime` are refreshed; title, source and
    /// the FTS row are preserved. Used for oversized files so their existing
    /// full-text index is not wiped.
    update_content: bool,
}

struct ScanInsert {
    normalized: String,
    title: String,
    size: i64,
    mtime: i64,
    source: Option<String>,
    title_bigram: String,
    body_bigram: String,
    tags_bigram: String,
}

/// Phase B of a scan: walk the workspace root and read ONLY new/changed
/// files. Runs without the DB lock — this is the slow part (filesystem +
/// content + bigram tokenization) and must never starve other commands.
///
/// 内存上限：每积累约 [`SCAN_FLUSH_BATCH`] 个文件就通过 `flush` 在一个短事务
/// 内落库并清空缓冲，因此最多 10 万个文件的 bigram 不会全部堆在内存里。
/// 计数由调用方在 `flush` 回调中跨批累加，最终一致性不受影响。
fn build_scan_plan(
    root: &Path,
    existing: &BTreeMap<String, (i64, i64, i64)>,
    mut on_progress: impl FnMut(usize),
    mut flush: impl FnMut(ScanPlan) -> Result<(), String>,
) -> Result<(), String> {
    let mut files = Vec::new();
    let mut budget = SCAN_MAX_FILES;
    collect_md_files(root, 0, &mut budget, &mut files);

    let mut plan = ScanPlan::default();
    for (i, file) in files.iter().enumerate() {
        if i % 500 == 0 {
            on_progress(i);
        }
        let normalized = normalize_path(file);
        let Ok(meta) = std::fs::metadata(file) else { continue };
        let mtime = mtime_ns(&meta);
        let size = meta.len() as i64;

        if let Some(&(id, old_mtime, old_size)) = existing.get(&normalized) {
            // Compare size as well as mtime: coarse-timestamp filesystems (FAT,
            // network shares) or mtime-preserving rewrites would otherwise be
            // missed, and a size-only change is always a content change.
            if old_mtime == mtime && old_size == size {
                continue;
            }
            // Oversized files: refresh only size/mtime and KEEP the existing
            // title/source and full-text index. Writing empty bigrams here (as
            // before) permanently wiped their searchability.
            if size as u64 > SCAN_MAX_BODY_BYTES {
                plan.updates.push(ScanUpdate {
                    id,
                    title: String::new(),
                    size,
                    mtime,
                    source: None,
                    title_bigram: String::new(),
                    body_bigram: String::new(),
                    tags_bigram: String::new(),
                    update_content: false,
                });
            } else {
                let content = match std::fs::read_to_string(file) {
                    Ok(c) => c,
                    Err(_) => {
                        // Unreadable / non-UTF8 (e.g. GBK): refresh size+mtime only
                        // and keep whatever index already exists for this file.
                        plan.updates.push(ScanUpdate {
                            id,
                            title: String::new(),
                            size,
                            mtime,
                            source: None,
                            title_bigram: String::new(),
                            body_bigram: String::new(),
                            tags_bigram: String::new(),
                            update_content: false,
                        });
                        continue;
                    }
                };
                let (title, source, tags, body) = extract_meta(&content, file);
                let title_bigram = cjk_bigram(&title);
                let body_bigram = cjk_bigram(&body);
                let tags_bigram = cjk_bigram(&tags.join(" "));
                plan.updates.push(ScanUpdate {
                    id,
                    title,
                    size,
                    mtime,
                    source,
                    title_bigram,
                    body_bigram,
                    tags_bigram,
                    update_content: true,
                });
            }
        } else {
            // Oversized new files: index by filename so they are still
            // listed/browsable, just not full-text searchable.
            let (title, source, tags, body) = if size as u64 > SCAN_MAX_BODY_BYTES {
                (file_stem_title(file), None, Vec::new(), String::new())
            } else {
                match std::fs::read_to_string(file) {
                    Ok(c) => extract_meta(&c, file),
                    // Non-UTF8 / unreadable: still index by filename so the
                    // file remains listed and browsable.
                    Err(_) => (file_stem_title(file), None, Vec::new(), String::new()),
                }
            };
            let title_bigram = cjk_bigram(&title);
            let body_bigram = cjk_bigram(&body);
            let tags_bigram = cjk_bigram(&tags.join(" "));
            plan.inserts.push(ScanInsert {
                normalized,
                title,
                size,
                mtime,
                source,
                title_bigram,
                body_bigram,
                tags_bigram,
            });
        }

        // 批次已满：先落库再继续，避免计划在内存中无界增长。
        if plan.updates.len() + plan.inserts.len() >= SCAN_FLUSH_BATCH {
            flush(std::mem::take(&mut plan))?;
        }
    }

    // Removals are judged against the disk (not this walk), so a capped walk
    // never deletes records it simply did not reach.
    for (normalized, (id, _, _)) in existing {
        if !Path::new(normalized).exists() {
            plan.removals.push(*id);
        }
    }
    // Flush the remaining tail (including removals).
    if !plan.updates.is_empty() || !plan.inserts.is_empty() || !plan.removals.is_empty() {
        flush(plan)?;
    }
    Ok(())
}

fn file_stem_title(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "未命名文档".to_string())
}

/// Full scan orchestrated with SHORT DB lock scopes:
/// A) lock → load baseline, release;  B) no lock → walk/read/build plan;
/// C) lock → apply in one transaction. Progress is emitted on
/// `workspace-scan-progress` every few hundred entries so a huge root cannot
/// make the app look dead even though the command returns only at the end.
pub fn scan_workspace_background(
    app: &tauri::AppHandle,
    workspace_id: i64,
) -> Result<ScanResultDto, String> {
    use tauri::Emitter;

    // Phase A — short lock.
    let (root, existing) = {
        let handle = db(app)?;
        handle.load_scan_baseline(workspace_id)?
    };

    // Phase B — NO lock. The guard above is already dropped here. Each batch
    // is flushed in its own short transaction (Phase C inline), so the plan
    // never accumulates the whole workspace's bigrams in memory.
    let app_for_cb = app.clone();
    let mut result = ScanResultDto::default();
    build_scan_plan(&root, &existing, move |processed| {
        let _ = app_for_cb.emit(
            "workspace-scan-progress",
            serde_json::json!({ "workspaceId": workspace_id, "processed": processed }),
        );
    }, |batch| {
        let handle = db(app)?;
        let (updated, indexed, removed) =
            handle.apply_scan_batch(workspace_id, &batch.updates, &batch.inserts, &batch.removals)?;
        result.updated += updated;
        result.indexed += indexed;
        result.removed += removed;
        Ok(())
    })?;
    result.total = db(app)?.doc_count(workspace_id)?;

    let _ = app.emit(
        "workspace-scan-progress",
        serde_json::json!({
            "workspaceId": workspace_id,
            "done": true,
            "indexed": result.indexed,
            "updated": result.updated,
            "removed": result.removed,
            "total": result.total,
        }),
    );
    Ok(result)
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
        Some(end) => {
            // Skip past the closing `---\n` or `---\r\n` delimiter.
            let skip = if rest[end..].starts_with("\n---\n") {
                5
            } else if rest[end..].starts_with("\n---\r\n") {
                6
            } else {
                4
            };
            &rest[end + skip..]
        }
        None => content,
    }
}

// ---------------------------------------------------------------------------
// CJK bigram tokenization for FTS5
// ---------------------------------------------------------------------------

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

/// Sentinel markers used inside FTS5 snippets; replaced with `<mark>` tags
/// after the surrounding text has been HTML-escaped.
const MARK_OPEN: &str = "\u{1}";
const MARK_CLOSE: &str = "\u{2}";

/// HTML-escape a raw FTS5 snippet and then restore the highlight markers, so
/// the result is safe to inject with `dangerouslySetInnerHTML`.
fn escape_snippet(raw: &str) -> String {
    let escaped = raw
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    clean_cjk_spaces(&escaped)
        .replace(MARK_OPEN, "<mark>")
        .replace(MARK_CLOSE, "</mark>")
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

/// Mark stale `Processing` batch tasks from a previous session as `Failed`.
/// Call this once during app startup, after the DB is first opened.
pub fn reconcile_stale_batch_tasks(app: &tauri::AppHandle) -> Result<u64, String> {
    db(app)?.reconcile_stale_tasks()
}

static GLOBAL_DB: OnceLock<Mutex<Option<WorkspaceDb>>> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn strips_windows_extended_prefix() {
        assert_eq!(strip_extended_prefix(r"\\?\D:\notes\a.md"), "D:/notes/a.md");
        assert_eq!(
            strip_extended_prefix(r"\\?\UNC\server\share\a.md"),
            "//server/share/a.md"
        );
        assert_eq!(strip_extended_prefix("D:/plain/path.md"), "D:/plain/path.md");
    }

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

        // 工作区根路径刻意包含中文：list_subfolders 曾按字节数算 substr 偏移，
        // CJK 根路径会把第一段目录名（如 "content-main"）截成 "main"。
        let ws_dir = dir.join("测试数据/docs");
        fs::create_dir_all(ws_dir.join("sub/deep")).unwrap();
        fs::write(
            ws_dir.join("a.md"),
            "---\ntitle: 人工智能报告\nsource: https://example.com/a\n---\n人工智能在医疗文档中的应用。\n",
        )
        .unwrap();
        fs::write(ws_dir.join("sub/b.md"), "# 医疗影像\n影像分析技术概述。\n").unwrap();
        fs::write(ws_dir.join("sub/deep/c.md"), "# 深层文档\n正文。\n").unwrap();

        let ws = db.add_workspace("测试库", ws_dir.to_str().unwrap()).unwrap();
        let scan = db.scan_workspace(ws.id).unwrap();
        assert_eq!(scan.indexed, 3);
        assert_eq!(scan.total, 3);

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
        let recent = db.list_recent(Some(ws.id), 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, hits[0].document.id);

        // 文件名搜索："c" 是单字符，不参与 FTS 分词，只能按 title 命中
        let by_name = db.search("c", ws.id, 10).unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].document.title, "c");
        assert!(by_name[0].snippet.is_none());

        // 纯内容命中（b.md 的 title 是文件干 "b"，不含关键词）仍有摘要
        let by_content = db.search("影像", ws.id, 10).unwrap();
        assert_eq!(by_content.len(), 1);
        assert!(by_content[0].snippet.is_some());

        // 内容 + 文件名同时命中（a.md：title "人工智能报告" + 正文）只出现一次
        let both = db.search("人工智能", ws.id, 10).unwrap();
        assert_eq!(both.len(), 1);
        assert!(both[0].snippet.is_some());

        // 目录浏览
        // 目录浏览：文档列表是递归的（含所有子目录），与徽章计数一致；
        // 子文件夹首段名不得被 CJK 路径的字节/字符偏移截断
        let folders = db.list_subfolders(ws.id, None).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "sub");
        assert_eq!(folders[0].doc_count, 2);
        let root_docs = db.list_documents(ws.id, None).unwrap();
        assert_eq!(root_docs.len(), 3);
        let sub_docs = db.list_documents(ws.id, Some("sub")).unwrap();
        assert_eq!(sub_docs.len(), 2);
        assert!(sub_docs.iter().any(|d| d.path.ends_with("deep/c.md")));

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
