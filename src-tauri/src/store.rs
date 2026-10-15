//! SQLite persistence. One file in the app-data folder; images live beside it on disk.
//! The `events` table is an append-only ledger: nothing ever rewrites or deletes it except
//! deleting its whole project.

use crate::error::{msg, AppResult};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

pub fn sha256(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

const MIGRATIONS: &[&str] = &[
    r#"
CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  idea TEXT NOT NULL DEFAULT '',
  style_brief TEXT NOT NULL DEFAULT '',
  shared_tail TEXT NOT NULL DEFAULT '',
  output_dir TEXT,
  image_model TEXT NOT NULL,
  text_model TEXT NOT NULL,
  quality TEXT NOT NULL DEFAULT 'medium',
  size TEXT NOT NULL DEFAULT '1024x1024',
  background TEXT NOT NULL DEFAULT 'transparent',
  gate TEXT NOT NULL DEFAULT 'none',
  concurrency INTEGER NOT NULL DEFAULT 3,
  budget_usd REAL NOT NULL DEFAULT 0,
  export_sizes TEXT NOT NULL DEFAULT '[1024]',
  webp_sizes TEXT NOT NULL DEFAULT '[]',
  filename_pattern TEXT NOT NULL DEFAULT '{slug}',
  use_references INTEGER NOT NULL DEFAULT 1,
  spec_locked INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE items (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  slug TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  subject TEXT NOT NULL,
  prompt_sha TEXT,
  status TEXT NOT NULL DEFAULT 'draft',
  priority INTEGER NOT NULL DEFAULT 0,
  redo_note TEXT,
  redo_from TEXT,
  auto_retries INTEGER NOT NULL DEFAULT 0,
  current_attempt TEXT,
  approved_attempt TEXT,
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX items_project ON items(project_id, status, priority, ord);
CREATE TABLE attempts (
  id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  n INTEGER NOT NULL,
  kind TEXT NOT NULL,
  model TEXT NOT NULL,
  quality TEXT NOT NULL,
  prompt TEXT NOT NULL,
  note TEXT,
  file TEXT,
  cost REAL NOT NULL DEFAULT 0,
  usage TEXT,
  gate TEXT,
  verdict TEXT,
  error TEXT,
  ms INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE INDEX attempts_item ON attempts(item_id, n);
CREATE INDEX attempts_project ON attempts(project_id);
CREATE TABLE refs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  file TEXT NOT NULL,
  attempt_id TEXT,
  created_at INTEGER NOT NULL
);
CREATE TABLE plan_calls (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  model TEXT NOT NULL,
  cost REAL NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
  item_id TEXT,
  kind TEXT NOT NULL,
  message TEXT NOT NULL,
  data TEXT,
  ts INTEGER NOT NULL
);
CREATE INDEX events_project ON events(project_id, id);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#,
    r#"
ALTER TABLE projects ADD COLUMN provider TEXT NOT NULL DEFAULT 'openai';
ALTER TABLE projects ADD COLUMN text_provider TEXT NOT NULL DEFAULT 'openai';
"#,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub idea: String,
    pub style_brief: String,
    pub shared_tail: String,
    pub output_dir: Option<String>,
    pub image_model: String,
    pub text_model: String,
    pub quality: String,
    pub size: String,
    pub background: String,
    pub gate: String,
    pub concurrency: i64,
    pub budget_usd: f64,
    pub export_sizes: Vec<u32>,
    pub webp_sizes: Vec<u32>,
    pub filename_pattern: String,
    pub use_references: bool,
    pub spec_locked: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Image provider id (see `providers::Kind`).
    pub provider: String,
    /// Provider used for planning.
    pub text_provider: String,
}

/// Fields the UI may change. `None` leaves a field as it is.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub idea: Option<String>,
    pub style_brief: Option<String>,
    pub shared_tail: Option<String>,
    pub output_dir: Option<String>,
    pub image_model: Option<String>,
    pub text_model: Option<String>,
    pub quality: Option<String>,
    pub size: Option<String>,
    pub background: Option<String>,
    pub gate: Option<String>,
    pub concurrency: Option<i64>,
    pub budget_usd: Option<f64>,
    pub export_sizes: Option<Vec<u32>>,
    pub webp_sizes: Option<Vec<u32>>,
    pub filename_pattern: Option<String>,
    pub use_references: Option<bool>,
    pub provider: Option<String>,
    pub text_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProjectStats {
    pub total: i64,
    pub draft: i64,
    pub queued: i64,
    pub drawing: i64,
    pub done: i64,
    pub flagged: i64,
    pub failed: i64,
    pub approved: i64,
    pub rejected: i64,
    pub spent_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    #[serde(flatten)]
    pub project: Project,
    pub stats: ProjectStats,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub project_id: String,
    pub ord: i64,
    pub slug: String,
    pub title: String,
    pub subject: String,
    pub prompt_sha: Option<String>,
    pub status: String,
    pub priority: i64,
    pub redo_note: Option<String>,
    pub redo_from: Option<String>,
    pub auto_retries: i64,
    pub current_attempt: Option<String>,
    pub approved_attempt: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: i64,
    /// Filled by queries that join attempts.
    pub file: Option<String>,
    pub attempts: i64,
    pub cost: f64,
    pub verdict: Option<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub id: String,
    pub item_id: String,
    pub project_id: String,
    pub n: i64,
    pub kind: String,
    pub model: String,
    pub quality: String,
    pub prompt: String,
    pub note: Option<String>,
    pub file: Option<String>,
    pub cost: f64,
    pub usage: Option<Value>,
    pub gate: Option<Value>,
    pub verdict: Option<String>,
    pub error: Option<String>,
    pub ms: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reference {
    pub id: String,
    pub project_id: String,
    pub file: String,
    pub attempt_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: i64,
    pub project_id: Option<String>,
    pub item_id: Option<String>,
    pub kind: String,
    pub message: String,
    pub data: Option<Value>,
    pub ts: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpendSummary {
    pub today_usd: f64,
    pub total_usd: f64,
}

/// Prompt sent for an item: its own subject, then the project's shared tail.
pub fn full_prompt(project: &Project, item: &Item) -> String {
    let tail = project.shared_tail.trim();
    if tail.is_empty() {
        item.subject.trim().to_string()
    } else {
        format!("{}\n\n{}", item.subject.trim(), tail)
    }
}

pub struct Store {
    conn: Mutex<Connection>,
    pub data_dir: PathBuf,
}

const ITEM_COLS: &str = "i.id, i.project_id, i.ord, i.slug, i.title, i.subject, i.prompt_sha, i.status, i.priority, \
  i.redo_note, i.redo_from, i.auto_retries, i.current_attempt, i.approved_attempt, i.last_error, i.updated_at, \
  (SELECT file FROM attempts WHERE id = COALESCE(i.approved_attempt, i.current_attempt)), \
  (SELECT COUNT(*) FROM attempts WHERE item_id = i.id), \
  (SELECT COALESCE(SUM(cost), 0) FROM attempts WHERE item_id = i.id), \
  (SELECT verdict FROM attempts WHERE id = COALESCE(i.approved_attempt, i.current_attempt)), \
  (SELECT gate FROM attempts WHERE id = COALESCE(i.approved_attempt, i.current_attempt))";

fn item_from_row(r: &Row) -> rusqlite::Result<Item> {
    let gate: Option<String> = r.get(20)?;
    let reasons = gate
        .and_then(|g| serde_json::from_str::<Value>(&g).ok())
        .and_then(|g| g["reasons"].as_array().cloned())
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(Item {
        id: r.get(0)?,
        project_id: r.get(1)?,
        ord: r.get(2)?,
        slug: r.get(3)?,
        title: r.get(4)?,
        subject: r.get(5)?,
        prompt_sha: r.get(6)?,
        status: r.get(7)?,
        priority: r.get(8)?,
        redo_note: r.get(9)?,
        redo_from: r.get(10)?,
        auto_retries: r.get(11)?,
        current_attempt: r.get(12)?,
        approved_attempt: r.get(13)?,
        last_error: r.get(14)?,
        updated_at: r.get(15)?,
        file: r.get(16)?,
        attempts: r.get(17)?,
        cost: r.get(18)?,
        verdict: r.get(19)?,
        reasons,
    })
}

const PROJECT_COLS: &str = "id, name, idea, style_brief, shared_tail, output_dir, image_model, text_model, quality, size, \
  background, gate, concurrency, budget_usd, export_sizes, webp_sizes, filename_pattern, use_references, spec_locked, \
  created_at, updated_at, provider, text_provider";

fn project_from_row(r: &Row) -> rusqlite::Result<Project> {
    let sizes = |s: String| serde_json::from_str::<Vec<u32>>(&s).unwrap_or_default();
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        idea: r.get(2)?,
        style_brief: r.get(3)?,
        shared_tail: r.get(4)?,
        output_dir: r.get(5)?,
        image_model: r.get(6)?,
        text_model: r.get(7)?,
        quality: r.get(8)?,
        size: r.get(9)?,
        background: r.get(10)?,
        gate: r.get(11)?,
        concurrency: r.get(12)?,
        budget_usd: r.get(13)?,
        export_sizes: sizes(r.get(14)?),
        webp_sizes: sizes(r.get(15)?),
        filename_pattern: r.get(16)?,
        use_references: r.get::<_, i64>(17)? != 0,
        spec_locked: r.get::<_, i64>(18)? != 0,
        created_at: r.get(19)?,
        updated_at: r.get(20)?,
        provider: r.get(21)?,
        text_provider: r.get(22)?,
    })
}

const ATTEMPT_COLS: &str =
    "id, item_id, project_id, n, kind, model, quality, prompt, note, file, cost, usage, gate, verdict, error, ms, created_at";

fn attempt_from_row(r: &Row) -> rusqlite::Result<Attempt> {
    let json = |s: Option<String>| s.and_then(|s| serde_json::from_str(&s).ok());
    Ok(Attempt {
        id: r.get(0)?,
        item_id: r.get(1)?,
        project_id: r.get(2)?,
        n: r.get(3)?,
        kind: r.get(4)?,
        model: r.get(5)?,
        quality: r.get(6)?,
        prompt: r.get(7)?,
        note: r.get(8)?,
        file: r.get(9)?,
        cost: r.get(10)?,
        usage: json(r.get(11)?),
        gate: json(r.get(12)?),
        verdict: r.get(13)?,
        error: r.get(14)?,
        ms: r.get(15)?,
        created_at: r.get(16)?,
    })
}

impl Store {
    pub fn open(data_dir: &Path) -> AppResult<Store> {
        std::fs::create_dir_all(data_dir)?;
        let conn = Connection::open(data_dir.join("eitri.sqlite"))?;
        Store::init(conn, data_dir.to_path_buf())
    }

    pub fn open_in_memory(data_dir: &Path) -> AppResult<Store> {
        Store::init(Connection::open_in_memory()?, data_dir.to_path_buf())
    }

    fn init(conn: Connection, data_dir: PathBuf) -> AppResult<Store> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            conn.execute_batch(sql)?;
            conn.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
        }
        // A crash or quit mid-draw leaves items marked drawing; their request is gone.
        conn.execute(
            "UPDATE items SET status = 'queued' WHERE status = 'drawing'",
            [],
        )?;
        Ok(Store {
            conn: Mutex::new(conn),
            data_dir,
        })
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn project_dir(&self, project_id: &str) -> PathBuf {
        self.data_dir.join("projects").join(project_id)
    }

    // ---------- settings ----------

    pub fn setting(&self, key: &str) -> AppResult<Option<String>> {
        Ok(self
            .db()
            .query_row("SELECT value FROM settings WHERE key = ?", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        self.db().execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    // ---------- projects ----------

    pub fn create_project(
        &self,
        name: &str,
        image_model: &str,
        text_model: &str,
        concurrency: i64,
        budget: f64,
    ) -> AppResult<Project> {
        self.create_project_with(
            name,
            "openai",
            image_model,
            "openai",
            text_model,
            concurrency,
            budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_project_with(
        &self,
        name: &str,
        provider: &str,
        image_model: &str,
        text_provider: &str,
        text_model: &str,
        concurrency: i64,
        budget: f64,
    ) -> AppResult<Project> {
        let id = new_id();
        let t = now_ms();
        self.db().execute(
            "INSERT INTO projects(id, name, provider, image_model, text_provider, text_model, concurrency, budget_usd, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![id, name.trim(), provider, image_model, text_provider, text_model, concurrency, budget, t],
        )?;
        std::fs::create_dir_all(self.project_dir(&id))?;
        self.event(
            Some(&id),
            None,
            "project",
            &format!("Created project {}", name.trim()),
            None,
        )?;
        self.project(&id)
    }

    pub fn project(&self, id: &str) -> AppResult<Project> {
        self.db()
            .query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?"),
                [id],
                project_from_row,
            )
            .optional()?
            .ok_or_else(|| msg("Project not found."))
    }

    pub fn projects(&self) -> AppResult<Vec<ProjectSummary>> {
        let projects: Vec<Project> = {
            let db = self.db();
            let mut st = db.prepare(&format!(
                "SELECT {PROJECT_COLS} FROM projects ORDER BY updated_at DESC"
            ))?;
            let rows = st.query_map([], project_from_row)?;
            rows.collect::<Result<_, _>>()?
        };
        projects
            .into_iter()
            .map(|p| {
                let stats = self.stats(&p.id)?;
                let cover = self.cover(&p.id)?;
                Ok(ProjectSummary {
                    project: p,
                    stats,
                    cover,
                })
            })
            .collect()
    }

    fn cover(&self, project_id: &str) -> AppResult<Option<String>> {
        Ok(self
            .db()
            .query_row(
                "SELECT a.file FROM items i JOIN attempts a ON a.id = COALESCE(i.approved_attempt, i.current_attempt) \
                 WHERE i.project_id = ? AND a.file IS NOT NULL ORDER BY (i.approved_attempt IS NULL), i.ord LIMIT 1",
                [project_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn stats(&self, project_id: &str) -> AppResult<ProjectStats> {
        let db = self.db();
        let mut s = ProjectStats::default();
        let mut st =
            db.prepare("SELECT status, COUNT(*) FROM items WHERE project_id = ? GROUP BY status")?;
        let rows = st.query_map([project_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (status, n) = row?;
            s.total += n;
            match status.as_str() {
                "draft" => s.draft = n,
                "queued" => s.queued = n,
                "drawing" => s.drawing = n,
                "done" => s.done = n,
                "flagged" => s.flagged = n,
                "failed" | "refused" => s.failed += n,
                "approved" => s.approved = n,
                "rejected" => s.rejected = n,
                _ => {}
            }
        }
        s.spent_usd = db.query_row(
            "SELECT COALESCE((SELECT SUM(cost) FROM attempts WHERE project_id = ?1), 0) + \
                    COALESCE((SELECT SUM(cost) FROM plan_calls WHERE project_id = ?1), 0)",
            [project_id],
            |r| r.get(0),
        )?;
        Ok(s)
    }

    pub fn update_project(&self, id: &str, p: ProjectPatch) -> AppResult<Project> {
        let cur = self.project(id)?;
        let locked_tail = cur.spec_locked.then(|| cur.shared_tail.clone());
        let n = Project {
            name: p.name.unwrap_or(cur.name),
            idea: p.idea.unwrap_or(cur.idea),
            style_brief: p.style_brief.unwrap_or(cur.style_brief),
            shared_tail: p.shared_tail.unwrap_or(cur.shared_tail),
            output_dir: p.output_dir.or(cur.output_dir),
            image_model: p.image_model.unwrap_or(cur.image_model),
            text_model: p.text_model.unwrap_or(cur.text_model),
            quality: p.quality.unwrap_or(cur.quality),
            size: p.size.unwrap_or(cur.size),
            background: p.background.unwrap_or(cur.background),
            gate: p.gate.unwrap_or(cur.gate),
            concurrency: p.concurrency.unwrap_or(cur.concurrency).clamp(1, 8),
            budget_usd: p.budget_usd.unwrap_or(cur.budget_usd).max(0.0),
            export_sizes: p.export_sizes.unwrap_or(cur.export_sizes),
            webp_sizes: p.webp_sizes.unwrap_or(cur.webp_sizes),
            filename_pattern: p.filename_pattern.unwrap_or(cur.filename_pattern),
            use_references: p.use_references.unwrap_or(cur.use_references),
            provider: p.provider.unwrap_or(cur.provider),
            text_provider: p.text_provider.unwrap_or(cur.text_provider),
            ..cur
        };
        if locked_tail.is_some_and(|t| t != n.shared_tail) {
            return Err(msg(
                "The spec is approved, so its shared prompt is frozen. Unlock it first.",
            ));
        }
        self.db().execute(
            "UPDATE projects SET name=?2, idea=?3, style_brief=?4, shared_tail=?5, output_dir=?6, image_model=?7, \
             text_model=?8, quality=?9, size=?10, background=?11, gate=?12, concurrency=?13, budget_usd=?14, \
             export_sizes=?15, webp_sizes=?16, filename_pattern=?17, use_references=?18, updated_at=?19, \
             provider=?20, text_provider=?21 WHERE id=?1",
            params![
                id, n.name, n.idea, n.style_brief, n.shared_tail, n.output_dir, n.image_model, n.text_model, n.quality,
                n.size, n.background, n.gate, n.concurrency, n.budget_usd,
                serde_json::to_string(&n.export_sizes)?, serde_json::to_string(&n.webp_sizes)?,
                n.filename_pattern, n.use_references as i64, now_ms(), n.provider, n.text_provider
            ],
        )?;
        self.project(id)
    }

    /// Copies a project's settings, plan, prompts and style references into a new project.
    /// Drawn images are not copied; every item starts as a draft.
    pub fn duplicate_project(&self, id: &str) -> AppResult<Project> {
        let src = self.project(id)?;
        let copy = self.create_project_with(
            &format!("{} copy", src.name),
            &src.provider,
            &src.image_model,
            &src.text_provider,
            &src.text_model,
            src.concurrency,
            src.budget_usd,
        )?;
        self.update_project(
            &copy.id,
            ProjectPatch {
                idea: Some(src.idea.clone()),
                style_brief: Some(src.style_brief.clone()),
                shared_tail: Some(src.shared_tail.clone()),
                output_dir: src.output_dir.clone(),
                quality: Some(src.quality.clone()),
                size: Some(src.size.clone()),
                background: Some(src.background.clone()),
                gate: Some(src.gate.clone()),
                export_sizes: Some(src.export_sizes.clone()),
                webp_sizes: Some(src.webp_sizes.clone()),
                filename_pattern: Some(src.filename_pattern.clone()),
                use_references: Some(src.use_references),
                ..Default::default()
            },
        )?;
        let items: Vec<(String, String, String)> = self
            .items(id)?
            .into_iter()
            .map(|i| (i.slug, i.title, i.subject))
            .collect();
        self.replace_plan(&copy.id, &items)?;
        let refs_dir = self.project_dir(&copy.id).join("refs");
        for r in self.references(id)? {
            std::fs::create_dir_all(&refs_dir)?;
            let dest = refs_dir.join(format!("{}.png", new_id()));
            if std::fs::copy(&r.file, &dest).is_ok() {
                self.add_reference(&copy.id, &dest.to_string_lossy(), None)?;
            }
        }
        self.event(
            Some(&copy.id),
            None,
            "project",
            &format!("Duplicated from {}", src.name),
            None,
        )?;
        self.project(&copy.id)
    }

    pub fn delete_project(&self, id: &str) -> AppResult<()> {
        self.db()
            .execute("DELETE FROM projects WHERE id = ?", [id])?;
        let dir = self.project_dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    /// Freezes every prompt: each item records the SHA-256 of the exact prompt it will send.
    pub fn lock_spec(&self, project_id: &str, locked: bool) -> AppResult<Project> {
        let project = self.project(project_id)?;
        if locked {
            for item in self.items(project_id)? {
                let sha = sha256(&full_prompt(&project, &item));
                self.db().execute(
                    "UPDATE items SET prompt_sha = ? WHERE id = ?",
                    params![sha, item.id],
                )?;
            }
        }
        self.db().execute(
            "UPDATE projects SET spec_locked = ?2, updated_at = ?3 WHERE id = ?1",
            params![project_id, locked as i64, now_ms()],
        )?;
        let what = if locked {
            "Spec approved; prompts frozen"
        } else {
            "Spec unlocked for editing"
        };
        self.event(Some(project_id), None, "spec", what, None)?;
        self.project(project_id)
    }

    // ---------- items ----------

    pub fn items(&self, project_id: &str) -> AppResult<Vec<Item>> {
        let db = self.db();
        let mut st = db.prepare(&format!(
            "SELECT {ITEM_COLS} FROM items i WHERE i.project_id = ? ORDER BY i.ord"
        ))?;
        let rows = st.query_map([project_id], item_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn item(&self, id: &str) -> AppResult<Item> {
        self.db()
            .query_row(
                &format!("SELECT {ITEM_COLS} FROM items i WHERE i.id = ?"),
                [id],
                item_from_row,
            )
            .optional()?
            .ok_or_else(|| msg("Image not found."))
    }

    /// Replaces the project's unstarted items with a new plan. Items that already have
    /// attempts are kept, so a re-plan never throws away paid work.
    pub fn replace_plan(
        &self,
        project_id: &str,
        items: &[(String, String, String)],
    ) -> AppResult<()> {
        let db = self.db();
        db.execute(
            "DELETE FROM items WHERE project_id = ? AND NOT EXISTS (SELECT 1 FROM attempts a WHERE a.item_id = items.id)",
            [project_id],
        )?;
        let start: i64 = db.query_row(
            "SELECT COALESCE(MAX(ord), -1) + 1 FROM items WHERE project_id = ?",
            [project_id],
            |r| r.get(0),
        )?;
        let t = now_ms();
        let mut used: std::collections::HashSet<String> = {
            let mut st = db.prepare("SELECT slug FROM items WHERE project_id = ?")?;
            let rows = st.query_map([project_id], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        for (i, (slug, title, subject)) in items.iter().enumerate() {
            let slug = unique_slug(&slugify(slug), &mut used);
            db.execute(
                "INSERT INTO items(id, project_id, ord, slug, title, subject, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
                params![new_id(), project_id, start + i as i64, slug, title, subject, t],
            )?;
        }
        Ok(())
    }

    pub fn add_item(&self, project_id: &str, title: &str, subject: &str) -> AppResult<Item> {
        let id = new_id();
        {
            let db = self.db();
            let ord: i64 = db.query_row(
                "SELECT COALESCE(MAX(ord), -1) + 1 FROM items WHERE project_id = ?",
                [project_id],
                |r| r.get(0),
            )?;
            let mut used: std::collections::HashSet<String> = {
                let mut st = db.prepare("SELECT slug FROM items WHERE project_id = ?")?;
                let rows = st.query_map([project_id], |r| r.get::<_, String>(0))?;
                rows.collect::<Result<_, _>>()?
            };
            let base = if title.trim().is_empty() {
                format!("image-{}", ord + 1)
            } else {
                slugify(title)
            };
            let slug = unique_slug(&base, &mut used);
            db.execute(
                "INSERT INTO items(id, project_id, ord, slug, title, subject, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
                params![id, project_id, ord, slug, title.trim(), subject.trim(), now_ms()],
            )?;
        }
        self.item(&id)
    }

    pub fn edit_item(
        &self,
        id: &str,
        title: Option<String>,
        subject: Option<String>,
    ) -> AppResult<Item> {
        let item = self.item(id)?;
        let project = self.project(&item.project_id)?;
        if project.spec_locked && subject.as_ref().is_some_and(|s| s != &item.subject) {
            return Err(msg("The spec is approved, so prompts are frozen. Use Redo with a note, or unlock the spec."));
        }
        self.db().execute(
            "UPDATE items SET title = ?2, subject = ?3, updated_at = ?4 WHERE id = ?1",
            params![
                id,
                title.unwrap_or(item.title),
                subject.unwrap_or(item.subject),
                now_ms()
            ],
        )?;
        self.item(id)
    }

    pub fn delete_item(&self, id: &str) -> AppResult<()> {
        self.db().execute(
            "DELETE FROM items WHERE id = ? AND status NOT IN ('drawing')",
            [id],
        )?;
        Ok(())
    }

    pub fn set_status(&self, id: &str, status: &str, error: Option<&str>) -> AppResult<()> {
        self.db().execute(
            "UPDATE items SET status = ?2, last_error = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, status, error, now_ms()],
        )?;
        Ok(())
    }

    /// Puts items in the queue. `priority` jumps them ahead (used for previews).
    pub fn enqueue(
        &self,
        project_id: &str,
        ids: Option<&[String]>,
        priority: i64,
    ) -> AppResult<usize> {
        let db = self.db();
        let n = match ids {
            Some(ids) => {
                let mut n = 0;
                for id in ids {
                    n += db.execute(
                        "UPDATE items SET status = 'queued', priority = ?3, updated_at = ?4 \
                         WHERE id = ?1 AND project_id = ?2 AND status NOT IN ('drawing', 'queued')",
                        params![id, project_id, priority, now_ms()],
                    )?;
                }
                n
            }
            None => db.execute(
                "UPDATE items SET status = 'queued', priority = ?2, updated_at = ?3 WHERE project_id = ?1 AND status = 'draft'",
                params![project_id, priority, now_ms()],
            )?,
        };
        Ok(n)
    }

    /// Atomically takes the next queued item for a worker.
    pub fn claim_next(&self, project_id: &str) -> AppResult<Option<Item>> {
        let id: Option<String> = self
            .db()
            .query_row(
                "UPDATE items SET status = 'drawing', updated_at = ?2 WHERE id = (SELECT id FROM items \
                 WHERE project_id = ?1 AND status = 'queued' ORDER BY priority DESC, ord LIMIT 1) RETURNING id",
                params![project_id, now_ms()],
                |r| r.get(0),
            )
            .optional()?;
        id.map(|id| self.item(&id)).transpose()
    }

    pub fn count_status(&self, project_id: &str, status: &str) -> AppResult<i64> {
        Ok(self.db().query_row(
            "SELECT COUNT(*) FROM items WHERE project_id = ? AND status = ?",
            params![project_id, status],
            |r| r.get(0),
        )?)
    }

    pub fn set_redo(&self, id: &str, note: Option<&str>, from: Option<&str>) -> AppResult<()> {
        self.db().execute(
            "UPDATE items SET redo_note = ?2, redo_from = ?3, auto_retries = 0, approved_attempt = NULL, \
             status = 'queued', priority = 5, last_error = NULL, updated_at = ?4 WHERE id = ?1",
            params![id, note, from, now_ms()],
        )?;
        Ok(())
    }

    /// Automatic requeue after a failed check; unlike `set_redo` it keeps the retry count.
    pub fn requeue_with(&self, id: &str, note: Option<&str>, from: Option<&str>) -> AppResult<()> {
        self.db().execute(
            "UPDATE items SET redo_note = ?2, redo_from = ?3, status = 'queued', priority = 5, updated_at = ?4 WHERE id = ?1",
            params![id, note, from, now_ms()],
        )?;
        Ok(())
    }

    pub fn clear_redo(&self, id: &str) -> AppResult<()> {
        self.db().execute(
            "UPDATE items SET redo_note = NULL, redo_from = NULL WHERE id = ?",
            [id],
        )?;
        Ok(())
    }

    pub fn bump_auto_retry(&self, id: &str) -> AppResult<()> {
        self.db().execute(
            "UPDATE items SET auto_retries = auto_retries + 1 WHERE id = ?",
            [id],
        )?;
        Ok(())
    }

    pub fn approve(&self, id: &str, attempt: Option<&str>) -> AppResult<Item> {
        let item = self.item(id)?;
        let attempt = attempt
            .map(String::from)
            .or(item.current_attempt.clone())
            .ok_or_else(|| msg("Nothing to approve yet."))?;
        self.db().execute(
            "UPDATE items SET status = 'approved', approved_attempt = ?2, current_attempt = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, attempt, now_ms()],
        )?;
        self.event(
            Some(&item.project_id),
            Some(id),
            "review",
            &format!("Approved {}", item.slug),
            None,
        )?;
        self.item(id)
    }

    pub fn reject(&self, id: &str) -> AppResult<Item> {
        let item = self.item(id)?;
        self.db().execute(
            "UPDATE items SET status = 'rejected', approved_attempt = NULL, updated_at = ?2 WHERE id = ?1",
            params![id, now_ms()],
        )?;
        self.event(
            Some(&item.project_id),
            Some(id),
            "review",
            &format!("Rejected {}", item.slug),
            None,
        )?;
        self.item(id)
    }

    // ---------- attempts ----------

    #[allow(clippy::too_many_arguments)]
    pub fn insert_attempt(
        &self,
        item: &Item,
        kind: &str,
        model: &str,
        quality: &str,
        prompt: &str,
        note: Option<&str>,
        file: Option<&str>,
        cost: f64,
        usage: Option<&Value>,
        gate: Option<&Value>,
        verdict: Option<&str>,
        error: Option<&str>,
        ms: i64,
    ) -> AppResult<Attempt> {
        let id = new_id();
        {
            let db = self.db();
            let n: i64 = db.query_row(
                "SELECT COALESCE(MAX(n), 0) + 1 FROM attempts WHERE item_id = ?",
                [&item.id],
                |r| r.get(0),
            )?;
            db.execute(
                &format!("INSERT INTO attempts({ATTEMPT_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)"),
                params![
                    id, item.id, item.project_id, n, kind, model, quality, prompt, note, file, cost,
                    usage.map(|u| u.to_string()), gate.map(|g| g.to_string()), verdict, error, ms, now_ms()
                ],
            )?;
            if file.is_some() {
                db.execute(
                    "UPDATE items SET current_attempt = ?2 WHERE id = ?1",
                    params![item.id, id],
                )?;
            }
        }
        self.attempt(&id)
    }

    pub fn attempt(&self, id: &str) -> AppResult<Attempt> {
        self.db()
            .query_row(
                &format!("SELECT {ATTEMPT_COLS} FROM attempts WHERE id = ?"),
                [id],
                attempt_from_row,
            )
            .optional()?
            .ok_or_else(|| msg("Attempt not found."))
    }

    pub fn attempts(&self, item_id: &str) -> AppResult<Vec<Attempt>> {
        let db = self.db();
        let mut st = db.prepare(&format!(
            "SELECT {ATTEMPT_COLS} FROM attempts WHERE item_id = ? ORDER BY n DESC"
        ))?;
        let rows = st.query_map([item_id], attempt_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn spent(&self, project_id: &str) -> AppResult<f64> {
        Ok(self.stats(project_id)?.spent_usd)
    }

    pub fn spend_summary(&self) -> AppResult<SpendSummary> {
        let db = self.db();
        let day_start = now_ms() - (now_ms() % 86_400_000);
        let sum = |since: i64| -> rusqlite::Result<f64> {
            db.query_row(
                "SELECT COALESCE((SELECT SUM(cost) FROM attempts WHERE created_at >= ?1), 0) + \
                        COALESCE((SELECT SUM(cost) FROM plan_calls WHERE created_at >= ?1), 0)",
                [since],
                |r| r.get(0),
            )
        };
        Ok(SpendSummary {
            today_usd: sum(day_start)?,
            total_usd: sum(0)?,
        })
    }

    pub fn record_plan_call(&self, project_id: &str, model: &str, cost: f64) -> AppResult<()> {
        self.db().execute(
            "INSERT INTO plan_calls(id, project_id, model, cost, created_at) VALUES (?1,?2,?3,?4,?5)",
            params![new_id(), project_id, model, cost, now_ms()],
        )?;
        Ok(())
    }

    // ---------- references ----------

    pub fn add_reference(
        &self,
        project_id: &str,
        file: &str,
        attempt_id: Option<&str>,
    ) -> AppResult<Reference> {
        let id = new_id();
        self.db().execute(
            "INSERT INTO refs(id, project_id, file, attempt_id, created_at) VALUES (?1,?2,?3,?4,?5)",
            params![id, project_id, file, attempt_id, now_ms()],
        )?;
        self.event(
            Some(project_id),
            None,
            "reference",
            "Added a style reference",
            None,
        )?;
        Ok(Reference {
            id,
            project_id: project_id.into(),
            file: file.into(),
            attempt_id: attempt_id.map(String::from),
            created_at: now_ms(),
        })
    }

    pub fn references(&self, project_id: &str) -> AppResult<Vec<Reference>> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, project_id, file, attempt_id, created_at FROM refs WHERE project_id = ? ORDER BY created_at")?;
        let rows = st.query_map([project_id], |r| {
            Ok(Reference {
                id: r.get(0)?,
                project_id: r.get(1)?,
                file: r.get(2)?,
                attempt_id: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn remove_reference(&self, id: &str) -> AppResult<()> {
        self.db().execute("DELETE FROM refs WHERE id = ?", [id])?;
        Ok(())
    }

    // ---------- events ----------

    pub fn event(
        &self,
        project_id: Option<&str>,
        item_id: Option<&str>,
        kind: &str,
        message: &str,
        data: Option<&Value>,
    ) -> AppResult<()> {
        self.db().execute(
            "INSERT INTO events(project_id, item_id, kind, message, data, ts) VALUES (?1,?2,?3,?4,?5,?6)",
            params![project_id, item_id, kind, crate::error::redact(message), data.map(|d| d.to_string()), now_ms()],
        )?;
        Ok(())
    }

    pub fn events(&self, project_id: &str, limit: i64) -> AppResult<Vec<Event>> {
        let db = self.db();
        let mut st = db.prepare(
            "SELECT id, project_id, item_id, kind, message, data, ts FROM events WHERE project_id = ? ORDER BY id DESC LIMIT ?",
        )?;
        let rows = st.query_map(params![project_id, limit], |r| {
            let data: Option<String> = r.get(5)?;
            Ok(Event {
                id: r.get(0)?,
                project_id: r.get(1)?,
                item_id: r.get(2)?,
                kind: r.get(3)?,
                message: r.get(4)?,
                data: data.and_then(|d| serde_json::from_str(&d).ok()),
                ts: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.trim().chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out
        .trim_end_matches('-')
        .chars()
        .take(60)
        .collect::<String>();
    if out.is_empty() {
        "image".into()
    } else {
        out
    }
}

fn unique_slug(base: &str, used: &mut std::collections::HashSet<String>) -> String {
    let mut slug = base.to_string();
    let mut n = 2;
    while used.contains(&slug) {
        slug = format!("{base}-{n}");
        n += 1;
    }
    used.insert(slug.clone());
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (Store::open_in_memory(dir.path()).unwrap(), dir)
    }

    #[test]
    fn plan_claim_and_resume() {
        let (s, _d) = store();
        let p = s
            .create_project("Potions", "gpt-image-1", "gpt-4.1-mini", 3, 0.0)
            .unwrap();
        s.replace_plan(
            &p.id,
            &[
                ("red potion".into(), "Red".into(), "A red potion".into()),
                (
                    "red potion".into(),
                    "Red 2".into(),
                    "Another red potion".into(),
                ),
                ("blue".into(), "Blue".into(), "A blue potion".into()),
            ],
        )
        .unwrap();
        let items = s.items(&p.id).unwrap();
        assert_eq!(
            items.iter().map(|i| i.slug.as_str()).collect::<Vec<_>>(),
            ["red-potion", "red-potion-2", "blue"]
        );
        assert_eq!(s.enqueue(&p.id, None, 0).unwrap(), 3);
        s.enqueue(&p.id, Some(&[items[2].id.clone()]), 10).unwrap(); // already queued: unchanged
        let first = s.claim_next(&p.id).unwrap().unwrap();
        assert_eq!(first.status, "drawing");
        assert_eq!(s.count_status(&p.id, "drawing").unwrap(), 1);
        // Simulate a restart: drawing goes back to queued.
        let conn = std::mem::replace(&mut *s.db(), Connection::open_in_memory().unwrap());
        let s2 = Store::init(conn, s.data_dir.clone()).unwrap();
        assert_eq!(s2.count_status(&p.id, "drawing").unwrap(), 0);
        assert_eq!(s2.count_status(&p.id, "queued").unwrap(), 3);
    }

    #[test]
    fn locked_spec_freezes_prompts_and_tail() {
        let (s, _d) = store();
        let p = s
            .create_project("Icons", "gpt-image-1", "gpt-4.1-mini", 3, 0.0)
            .unwrap();
        s.update_project(
            &p.id,
            ProjectPatch {
                shared_tail: Some("flat icon".into()),
                ..Default::default()
            },
        )
        .unwrap();
        s.replace_plan(&p.id, &[("sword".into(), "Sword".into(), "A sword".into())])
            .unwrap();
        s.lock_spec(&p.id, true).unwrap();
        let item = &s.items(&p.id).unwrap()[0];
        assert_eq!(
            item.prompt_sha.as_deref(),
            Some(sha256("A sword\n\nflat icon").as_str())
        );
        assert!(s.edit_item(&item.id, None, Some("A spear".into())).is_err());
        assert!(s
            .update_project(
                &p.id,
                ProjectPatch {
                    shared_tail: Some("other".into()),
                    ..Default::default()
                }
            )
            .is_err());
    }

    #[test]
    fn duplicate_copies_plan_not_images() {
        let (s, _d) = store();
        let p = s
            .create_project_with(
                "Set",
                "fal",
                "fal-ai/flux/dev",
                "google",
                "gemini-2.5-flash",
                2,
                3.0,
            )
            .unwrap();
        s.replace_plan(&p.id, &[("a".into(), "A".into(), "a".into())])
            .unwrap();
        let a = s.items(&p.id).unwrap()[0].clone();
        s.insert_attempt(
            &a,
            "generate",
            "m",
            "medium",
            "a",
            None,
            Some("x.png"),
            0.05,
            None,
            None,
            Some("pass"),
            None,
            10,
        )
        .unwrap();
        let c = s.duplicate_project(&p.id).unwrap();
        assert_eq!(
            (c.provider.as_str(), c.text_provider.as_str()),
            ("fal", "google")
        );
        let items = s.items(&c.id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].status.as_str(), items[0].attempts), ("draft", 0));
    }

    #[test]
    fn replan_keeps_paid_items() {
        let (s, _d) = store();
        let p = s
            .create_project("Set", "gpt-image-1", "gpt-4.1-mini", 3, 0.0)
            .unwrap();
        s.replace_plan(
            &p.id,
            &[
                ("a".into(), "A".into(), "a".into()),
                ("b".into(), "B".into(), "b".into()),
            ],
        )
        .unwrap();
        let a = s.items(&p.id).unwrap()[0].clone();
        s.insert_attempt(
            &a,
            "generate",
            "gpt-image-1",
            "medium",
            "a",
            None,
            Some("x.png"),
            0.05,
            None,
            None,
            Some("pass"),
            None,
            10,
        )
        .unwrap();
        s.replace_plan(&p.id, &[("c".into(), "C".into(), "c".into())])
            .unwrap();
        let slugs: Vec<String> = s
            .items(&p.id)
            .unwrap()
            .into_iter()
            .map(|i| i.slug)
            .collect();
        assert_eq!(slugs, ["a", "c"]);
        assert!((s.spent(&p.id).unwrap() - 0.05).abs() < 1e-9);
    }
}
