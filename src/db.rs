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
}

pub struct DbClient {
    conn: Connection,
}

impl DbClient {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Initialize schema
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
        
        Ok(Self { conn })
    }

    pub fn list_chats(&self) -> Result<Vec<ChatSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, pinned, updated_at FROM chats ORDER BY pinned DESC, updated_at DESC"
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
        for chat in chat_iter {
            chats.push(chat?);
        }
        Ok(chats)
    }

    pub fn load_chat(&self, id: &str) -> Result<Option<ChatSessionFull>> {
        self.conn.query_row(
            "SELECT id, title, pinned, updated_at, active_node_id, tree_data FROM chats WHERE id = ?1",
            params![id],
            |row| {
                Ok(ChatSessionFull {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    pinned: row.get(2).map(|v: i32| v > 0)?,
                    updated_at: row.get(3)?,
                    active_node_id: row.get(4)?,
                    tree_data: row.get(5)?,
                })
            },
        ).optional()
    }

    pub fn search_chats(&self, query: &str) -> Result<Vec<ChatSession>> {
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
        let like_query = if clean_query.is_empty() {
            "%%".to_string()
        } else {
            format!("%{}%", clean_query)
        };

        let mut sql = String::from("SELECT id, title, pinned, updated_at, tree_data FROM chats WHERE 1=1");

        if only_title {
            sql.push_str(" AND title LIKE ?1");
        } else {
            sql.push_str(" AND (title LIKE ?1 OR tree_data LIKE ?1)");
        }

        if only_model {
            sql.push_str(" AND tree_data LIKE '%\"role\":\"model\"%'");
        }
        if only_user {
            sql.push_str(" AND tree_data LIKE '%\"role\":\"user\"%'");
        }
        if let Some(ts) = min_timestamp {
            sql.push_str(&format!(" AND updated_at >= {}", ts));
        }

        sql.push_str(" ORDER BY updated_at DESC LIMIT 50");

        let mut stmt = self.conn.prepare(&sql)?;

        let chat_iter = stmt.query_map(params![like_query], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let pinned: i32 = row.get(2)?;
            let updated_at: i64 = row.get(3)?;
            let tree_data: Option<String> = row.get(4)?;

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
                                                t.push_str(" ");
                                            }
                                        }
                                        for (_, val) in map {
                                            gather_text(val, t);
                                        }
                                    }
                                    serde_json::Value::Array(arr) => {
                                        for val in arr {
                                            gather_text(val, t);
                                        }
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

                    // Always try to use full_text first, if empty or not found, fallback
                    let lower_full = full_text.to_lowercase();
                    let lower_q = clean_query.to_lowercase();
                    
                    if !full_text.is_empty() && lower_full.find(&lower_q).is_some() {
                        let idx = lower_full.find(&lower_q).unwrap();
                        let mut start = lower_full[..idx].rfind(". ").map(|i| i + 2)
                            .or_else(|| lower_full[..idx].rfind("\n").map(|i| i + 1))
                            .unwrap_or(0);
                        
                        let mut end = lower_full[idx..].find(". ")
                            .map(|i| idx + i + 1)
                            .or_else(|| lower_full[idx..].find("\n").map(|i| idx + i))
                            .unwrap_or(lower_full.len());

                        if idx.saturating_sub(start) > 60 {
                            start = idx.saturating_sub(60);
                        }
                        if end.saturating_sub(idx) > 60 {
                            end = idx + clean_query.len() + 60;
                        }
                        
                        let safe_start = start.min(full_text.len());
                        let safe_end = end.min(full_text.len());

                        let mut char_start = safe_start;
                        while char_start > 0 && !full_text.is_char_boundary(char_start) {
                            char_start -= 1;
                        }
                        let mut char_end = safe_end;
                        while char_end < full_text.len() && !full_text.is_char_boundary(char_end) {
                            char_end += 1;
                        }

                        let snip = full_text[char_start..char_end].trim();
                        snippet = Some(format!("...{}...", snip));
                    } else {
                        // Fallback behavior if not parseable or not found in text
                        let lower_td = td.to_lowercase();
                        if let Some(idx) = lower_td.find(&lower_q) {
                            let mut start = idx.saturating_sub(60);
                            let mut end = (idx + clean_query.len() + 60).min(lower_td.len());

                            let mut char_start = start;
                            while char_start > 0 && !lower_td.is_char_boundary(char_start) {
                                char_start -= 1;
                            }
                            let mut char_end = end;
                            while char_end < lower_td.len() && !lower_td.is_char_boundary(char_end) {
                                char_end += 1;
                            }

                            let snip = &lower_td[char_start..char_end];
                            // Don't strip quotes, just return raw string part safely
                            snippet = Some(format!("...{}...", snip.replace("\n", " ").replace("\t", " ")));
                        }
                    }
                }
            }

            Ok(ChatSession {
                id,
                title,
                pinned: pinned != 0,
                updated_at,
                snippet,
            })
        })?;

        let mut chats = Vec::new();
        for chat in chat_iter {
            chats.push(chat?);
        }
        Ok(chats)
    }

    pub fn save_chat(&self, chat: &ChatSessionFull) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chats (id, title, pinned, updated_at, active_node_id, tree_data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                pinned = excluded.pinned,
                updated_at = excluded.updated_at,
                active_node_id = excluded.active_node_id,
                tree_data = excluded.tree_data",
            params![
                chat.id,
                chat.title,
                chat.pinned as i32,
                chat.updated_at,
                chat.active_node_id,
                chat.tree_data,
            ],
        )?;
        Ok(())
    }

    pub fn delete_chat(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM chats WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM chats", [])?;
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
}
