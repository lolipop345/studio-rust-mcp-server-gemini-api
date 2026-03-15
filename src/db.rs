use rusqlite::{params, Connection, Result, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub updated_at: i64,
    pub snippet: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatSessionFull {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub updated_at: i64,
    pub active_node_id: Option<String>,
    pub tree_data: Option<String>,
    pub project_id: Option<String>,
}

// ── Projects ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub details: Option<String>,
    pub context: Option<String>,
    pub chat_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub details: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────

pub struct DbClient {
    conn: Connection,
}

impl DbClient {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Chats table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL,
                active_node_id TEXT,
                tree_data TEXT
            )",
            [],
        )?;

        // Add project_id column if it doesn't exist (migration)
        let _ = conn.execute("ALTER TABLE chats ADD COLUMN project_id TEXT", []);

        // Projects table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                details TEXT,
                context TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    // ── Chats (global — no project) ───────────────────────────────────────────

    pub fn list_chats(&self) -> Result<Vec<ChatSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, pinned, updated_at FROM chats
             WHERE project_id IS NULL
             ORDER BY pinned DESC, updated_at DESC"
        )?;
        let chat_iter = stmt.query_map([], |row| {
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                pinned: row.get(2).map(|v: i32| v > 0)?,
                updated_at: row.get(3)?,
                snippet: None,
            })
        })?;
        let mut chats = Vec::new();
        for chat in chat_iter { chats.push(chat?); }
        Ok(chats)
    }

    // ── Chats (by project) ────────────────────────────────────────────────────

    pub fn list_chats_by_project(&self, project_id: &str) -> Result<Vec<ChatSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, pinned, updated_at FROM chats
             WHERE project_id = ?1
             ORDER BY pinned DESC, updated_at DESC"
        )?;
        let chat_iter = stmt.query_map(params![project_id], |row| {
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                pinned: row.get(2).map(|v: i32| v > 0)?,
                updated_at: row.get(3)?,
                snippet: None,
            })
        })?;
        let mut chats = Vec::new();
        for chat in chat_iter { chats.push(chat?); }
        Ok(chats)
    }

    pub fn load_chat(&self, id: &str) -> Result<Option<ChatSessionFull>> {
        self.conn.query_row(
            "SELECT id, title, pinned, updated_at, active_node_id, tree_data, project_id
             FROM chats WHERE id = ?1",
            params![id],
            |row| {
                Ok(ChatSessionFull {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    pinned: row.get(2).map(|v: i32| v > 0)?,
                    updated_at: row.get(3)?,
                    active_node_id: row.get(4)?,
                    tree_data: row.get(5)?,
                    project_id: row.get(6)?,
                })
            },
        ).optional()
    }

    pub fn search_chats(&self, query: &str, project_id: Option<&str>) -> Result<Vec<ChatSession>> {
        let mut clean_query_parts = Vec::new();
        let mut only_model = false;
        let mut only_user = false;
        let mut only_title = false;
        let mut min_timestamp = None;

        for part in query.split_whitespace() {
            let lower = part.to_lowercase();
            if lower == "onlymodel:true" {
                only_model = true;
            } else if lower == "onlyuser:true" {
                only_user = true;
            } else if lower == "onlytitle:true" {
                only_title = true;
            } else if lower.starts_with("last:") && lower.ends_with("d") {
                if let Ok(days) = lower[5..lower.len()-1].parse::<i64>() {
                    if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                        min_timestamp = Some((now.as_millis() as i64) - (days * 24 * 60 * 60 * 1000));
                    }
                }
            } else {
                clean_query_parts.push(part.to_string());
            }
        }

        let clean_query = clean_query_parts.join(" ");
        // Escape SQL LIKE special characters to prevent LIKE injection
        let escaped_query = clean_query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let like_query = if escaped_query.is_empty() {
            "%%".to_string()
        } else {
            format!("%{}%", escaped_query)
        };

        let mut sql = String::from(
            "SELECT id, title, pinned, updated_at, tree_data FROM chats WHERE 1=1"
        );

        // Project filter
        match project_id {
            Some(_) => sql.push_str(" AND project_id = ?2"),
            None    => sql.push_str(" AND project_id IS NULL"),
        }

        if only_title {
            sql.push_str(" AND title LIKE ?1 ESCAPE '\\'");
        } else {
            sql.push_str(" AND (title LIKE ?1 ESCAPE '\\' OR tree_data LIKE ?1 ESCAPE '\\')");
        }

        if only_model {
            sql.push_str(" AND tree_data LIKE '%\"role\":\"model\"%'");
        }
        if only_user {
            sql.push_str(" AND tree_data LIKE '%\"role\":\"user\"%'");
        }
        if let Some(ts) = min_timestamp {
            // Use parameterized query — ts is i64 so safe, but best practice
            sql.push_str(&format!(" AND updated_at >= {}", ts as i64));
        }

        sql.push_str(" ORDER BY updated_at DESC LIMIT 50");

        let mut stmt = self.conn.prepare(&sql)?;

        let map_row = |row: &rusqlite::Row| -> rusqlite::Result<(String, String, i32, i64, Option<String>)> {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        };

        let rows: Vec<_> = if let Some(pid) = project_id {
            stmt.query_map(params![like_query, pid], map_row)?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map(params![like_query], map_row)?
                .filter_map(|r| r.ok())
                .collect()
        };

        let mut chats = Vec::new();
        for (id, title, pinned, updated_at, tree_data) in rows {
            let mut snippet = None;
            if !clean_query.is_empty() {
                if let Some(td) = tree_data {
                    let mut full_text = String::new();
                    match serde_json::from_str::<serde_json::Value>(&td) {
                        Ok(value) => {
                            fn gather_text(v: &serde_json::Value, t: &mut String) {
                                match v {
                                    serde_json::Value::Object(map) => {
                                        if let Some(serde_json::Value::String(c)) = map.get("content") {
                                            if !c.trim().is_empty() {
                                                t.push_str(c.trim());
                                                t.push(' ');
                                            }
                                        }
                                        for (_, val) in map { gather_text(val, t); }
                                    }
                                    serde_json::Value::Array(arr) => {
                                        for val in arr { gather_text(val, t); }
                                    }
                                    _ => {}
                                }
                            }
                            gather_text(&value, &mut full_text);
                        }
                        Err(e) => {
                            println!("JSON Parse ERROR for chat {}: {}", id, e);
                        }
                    }

                    let lower_full = full_text.to_lowercase();
                    let lower_q   = clean_query.to_lowercase();

                    if !full_text.is_empty() && lower_full.find(&lower_q).is_some() {
                        let idx = lower_full.find(&lower_q).unwrap();
                        let start = lower_full[..idx].rfind(". ").map(|i| i + 2)
                            .or_else(|| lower_full[..idx].rfind('\n').map(|i| i + 1))
                            .unwrap_or(0);
                        let end = lower_full[idx..].find(". ")
                            .map(|i| idx + i + 1)
                            .or_else(|| lower_full[idx..].find('\n').map(|i| idx + i))
                            .unwrap_or(lower_full.len());

                        let start = if idx.saturating_sub(start) > 60 { idx.saturating_sub(60) } else { start };
                        let end   = if end.saturating_sub(idx) > 60 { (idx + clean_query.len() + 60).min(full_text.len()) } else { end };

                        let mut cs = start.min(full_text.len());
                        while cs > 0 && !full_text.is_char_boundary(cs) { cs -= 1; }
                        let mut ce = end.min(full_text.len());
                        while ce < full_text.len() && !full_text.is_char_boundary(ce) { ce += 1; }

                        snippet = Some(format!("...{}...", full_text[cs..ce].trim()));
                    } else {
                        let lower_td = td.to_lowercase();
                        if let Some(idx) = lower_td.find(&lower_q) {
                            let start = idx.saturating_sub(60);
                            let end   = (idx + clean_query.len() + 60).min(lower_td.len());
                            let mut cs = start;
                            while cs > 0 && !lower_td.is_char_boundary(cs) { cs -= 1; }
                            let mut ce = end;
                            while ce < lower_td.len() && !lower_td.is_char_boundary(ce) { ce += 1; }
                            snippet = Some(format!("...{}...", lower_td[cs..ce].replace('\n', " ").replace('\t', " ")));
                        }
                    }
                }
            }
            chats.push(ChatSession { id, title, pinned: pinned != 0, updated_at, snippet });
        }
        Ok(chats)
    }

    pub fn save_chat(&self, chat: &ChatSessionFull) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chats (id, title, pinned, updated_at, active_node_id, tree_data, project_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                pinned = excluded.pinned,
                updated_at = excluded.updated_at,
                active_node_id = excluded.active_node_id,
                tree_data = excluded.tree_data,
                project_id = excluded.project_id",
            params![
                chat.id,
                chat.title,
                chat.pinned as i32,
                chat.updated_at,
                chat.active_node_id,
                chat.tree_data,
                chat.project_id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_chat(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM chats WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM chats WHERE project_id IS NULL", [])?;
        Ok(())
    }

    pub fn update_title(&self, id: &str, new_title: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE chats SET title = ?1 WHERE id = ?2",
            params![new_title, id],
        )?;
        Ok(())
    }

    pub fn toggle_pin(&self, id: &str, pinned: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE chats SET pinned = ?1 WHERE id = ?2",
            params![pinned as i32, id],
        )?;
        Ok(())
    }

    // ── Projects ──────────────────────────────────────────────────────────────

    pub fn create_project(&self, id: &str, name: &str, details: Option<&str>, now: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO projects (id, name, details, context, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, ?4)",
            params![id, name, details, now],
        )?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, p.details, p.context, p.created_at, p.updated_at,
                    (SELECT COUNT(*) FROM chats c WHERE c.project_id = p.id) AS chat_count
             FROM projects p
             ORDER BY p.updated_at DESC"
        )?;
        let iter = stmt.query_map([], |row| {
            Ok(ProjectRow {
                id:         row.get(0)?,
                name:       row.get(1)?,
                details:    row.get(2)?,
                context:    row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                chat_count: row.get(6)?,
            })
        })?;
        let mut projects = Vec::new();
        for p in iter { projects.push(p?); }
        Ok(projects)
    }

    pub fn delete_project(&self, id: &str) -> Result<()> {
        // Delete all chats belonging to this project
        self.conn.execute(
            "DELETE FROM chats WHERE project_id = ?1",
            params![id],
        )?;
        self.conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_project_context(&self, id: &str, context: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE projects SET context = ?1, updated_at = ?2 WHERE id = ?3",
            params![context, chrono_now_ms(), id],
        )?;
        Ok(())
    }

    /// Load all tree_data from project chats updated in last 7 days, extract model_keypoints
    pub fn get_project_memory(&self, project_id: &str) -> Result<Vec<String>> {
        let seven_days_ago = chrono_now_ms() - 7 * 24 * 60 * 60 * 1000;
        let mut stmt = self.conn.prepare(
            "SELECT tree_data FROM chats
             WHERE project_id = ?1 AND updated_at >= ?2
             ORDER BY updated_at DESC"
        )?;
        let rows: Vec<Option<String>> = stmt.query_map(params![project_id, seven_days_ago], |row| {
            row.get::<_, Option<String>>(0)
        })?.filter_map(|r| r.ok()).collect();

        let mut memories = Vec::new();
        for tree_data_opt in rows {
            if let Some(tree_data) = tree_data_opt {
                // Parse the tree_data JSON and extract model_keypoints from assistant nodes
                if let Ok(tree) = serde_json::from_str::<serde_json::Value>(&tree_data) {
                    if let Some(obj) = tree.as_object() {
                        for (_node_id, node) in obj {
                            if node.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                                if let Some(kp) = node.get("model_keypoints").and_then(|v| v.as_str()) {
                                    let trimmed = kp.trim();
                                    if !trimmed.is_empty() {
                                        memories.push(trimmed.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(memories)
    }
}

fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
