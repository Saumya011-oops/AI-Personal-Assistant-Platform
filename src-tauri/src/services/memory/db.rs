use anyhow::Result;
use rusqlite::{params, Connection, Row};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbChat {
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
    pub last_memory_sync: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub token_count: i64,
    pub retrieved_document_ids: Option<String>,
    pub retrieved_memory_ids: Option<String>,
    pub citations: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbMemory {
    pub id: String,
    pub r#type: String, // PROFILE, PREFERENCE, etc.
    pub content: String,
    pub embedding_model: String,
    pub importance: i64,
    pub confidence: f64,
    pub access_count: i64,
    pub last_used: String,
    pub created_at: String,
    pub updated_at: String,
    pub source_conversation: Option<String>,
    pub status: String,
    pub deleted_at: Option<String>,
}

pub struct MemoryDb {
    conn: Arc<Mutex<Connection>>,
}

impl MemoryDb {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    // --- CHATS ---

    pub fn create_chat(&self, id: &str, title: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO chats (id, title, created_at, updated_at, message_count)
             VALUES (?1, ?2, datetime('now'), datetime('now'), 0)",
            params![id, title],
        )?;
        Ok(())
    }

    pub fn get_chat(&self, id: &str) -> Result<Option<DbChat>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, summary, created_at, updated_at, message_count, last_memory_sync FROM chats WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_chat(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_chats(&self) -> Result<Vec<DbChat>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, summary, created_at, updated_at, message_count, last_memory_sync
             FROM chats
             ORDER BY updated_at DESC"
        )?;
        let chat_iter = stmt.query_map([], |row| Self::row_to_chat(row))?;
        let mut chats = Vec::new();
        for chat in chat_iter {
            chats.push(chat?);
        }
        Ok(chats)
    }

    pub fn search_chats(&self, query_text: &str) -> Result<Vec<DbChat>> {
        let conn = self.conn.lock().unwrap();
        let like_query = format!("%{}%", query_text);
        let mut stmt = conn.prepare(
            "SELECT DISTINCT c.id, c.title, c.summary, c.created_at, c.updated_at, c.message_count, c.last_memory_sync
             FROM chats c
             LEFT JOIN chat_messages m ON m.conversation_id = c.id
             WHERE c.title LIKE ?1 OR c.summary LIKE ?1 OR m.content LIKE ?1
             ORDER BY c.updated_at DESC"
        )?;
        let chat_iter = stmt.query_map(params![like_query], |row| Self::row_to_chat(row))?;
        let mut chats = Vec::new();
        for chat in chat_iter {
            chats.push(chat?);
        }
        Ok(chats)
    }

    pub fn rename_chat(&self, id: &str, title: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE chats SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![title, id],
        )?;
        Ok(())
    }

    pub fn update_chat_sync_timestamp(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE chats SET last_memory_sync = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn delete_chat(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM chats WHERE id = ?1", params![id])?;
        conn.execute("DELETE FROM chat_messages WHERE conversation_id = ?1", params![id])?;
        conn.execute("DELETE FROM conversation_summaries WHERE conversation_id = ?1", params![id])?;
        Ok(())
    }

    // --- MESSAGES ---

    pub fn save_message(&self, msg: &DbMessage) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO chat_messages (id, conversation_id, role, content, token_count, retrieved_document_ids, retrieved_memory_ids, citations, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                msg.id,
                msg.conversation_id,
                msg.role,
                msg.content,
                msg.token_count,
                msg.retrieved_document_ids,
                msg.retrieved_memory_ids,
                msg.citations,
                msg.created_at
            ],
        )?;

        // Update message_count and updated_at in chats
        conn.execute(
            "UPDATE chats 
             SET message_count = (SELECT COUNT(*) FROM chat_messages WHERE conversation_id = ?1),
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![msg.conversation_id],
        )?;

        Ok(())
    }

    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<DbMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, role, content, token_count, retrieved_document_ids, retrieved_memory_ids, citations, created_at
             FROM chat_messages
             WHERE conversation_id = ?1
             ORDER BY created_at ASC"
        )?;
        let msg_iter = stmt.query_map(params![conversation_id], |row| {
            Ok(DbMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                token_count: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                retrieved_document_ids: row.get(5)?,
                retrieved_memory_ids: row.get(6)?,
                citations: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        let mut msgs = Vec::new();
        for msg in msg_iter {
            msgs.push(msg?);
        }
        Ok(msgs)
    }

    // --- SUMMARIES ---

    pub fn get_summary(&self, conversation_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT summary FROM conversation_summaries WHERE conversation_id = ?1"
        )?;
        let mut rows = stmt.query(params![conversation_id])?;
        if let Some(row) = rows.next()? {
            let summary: String = row.get(0)?;
            Ok(Some(summary))
        } else {
            Ok(None)
        }
    }

    pub fn save_summary(&self, conversation_id: &str, summary: &str, last_message_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO conversation_summaries (id, conversation_id, summary, last_message_id, created_at, updated_at)
             VALUES (
                coalesce((SELECT id FROM conversation_summaries WHERE conversation_id = ?1), lower(hex(randomblob(16)))),
                ?1, ?2, ?3,
                coalesce((SELECT created_at FROM conversation_summaries WHERE conversation_id = ?1), datetime('now')),
                datetime('now')
             )",
            params![conversation_id, summary, last_message_id],
        )?;

        // Update chats table too
        conn.execute(
            "UPDATE chats SET summary = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![summary, conversation_id],
        )?;

        Ok(())
    }

    // --- UNIFIED MEMORIES ---

    pub fn save_memory(&self, mem: &DbMemory) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO memories (id, type, content, embedding_model, importance, confidence, access_count, last_used, created_at, updated_at, source_conversation, status, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                mem.id,
                mem.r#type,
                mem.content,
                mem.embedding_model,
                mem.importance,
                mem.confidence,
                mem.access_count,
                mem.last_used,
                mem.created_at,
                mem.updated_at,
                mem.source_conversation,
                mem.status,
                mem.deleted_at
            ],
        )?;
        Ok(())
    }

    pub fn get_memory(&self, id: &str) -> Result<Option<DbMemory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, content, embedding_model, importance, confidence, access_count, last_used, created_at, updated_at, source_conversation, status, deleted_at
             FROM memories
             WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_memory(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_memories(&self) -> Result<Vec<DbMemory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, content, embedding_model, importance, confidence, access_count, last_used, created_at, updated_at, source_conversation, status, deleted_at
             FROM memories
             WHERE status != 'deleted' AND deleted_at IS NULL
             ORDER BY updated_at DESC"
        )?;
        let mem_iter = stmt.query_map([], |row| Self::row_to_memory(row))?;
        let mut mems = Vec::new();
        for mem in mem_iter {
            mems.push(mem?);
        }
        Ok(mems)
    }

    pub fn list_memories_by_ids(&self, ids: &[String]) -> Result<Vec<DbMemory>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().unwrap();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, type, content, embedding_model, importance, confidence, access_count, last_used, created_at, updated_at, source_conversation, status, deleted_at
             FROM memories
             WHERE id IN ({}) AND status != 'deleted' AND deleted_at IS NULL",
            placeholders
        );
        let mut stmt = conn.prepare(&query)?;
        
        let params_val: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let mem_iter = stmt.query_map(&*params_val, |row| Self::row_to_memory(row))?;
        let mut mems = Vec::new();
        for mem in mem_iter {
            mems.push(mem?);
        }
        Ok(mems)
    }

    pub fn soft_delete_memory(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memories SET status = 'deleted', deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn increment_memory_access(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memories 
             SET access_count = access_count + 1, 
                 last_used = datetime('now') 
             WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn clear_all_memories(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM memories", [])?;
        Ok(())
    }

    // --- HELPERS ---

    fn row_to_chat(row: &Row) -> Result<DbChat, rusqlite::Error> {
        Ok(DbChat {
            id: row.get(0)?,
            title: row.get(1)?,
            summary: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
            message_count: row.get(5)?,
            last_memory_sync: row.get(6)?,
        })
    }

    fn row_to_memory(row: &Row) -> Result<DbMemory, rusqlite::Error> {
        Ok(DbMemory {
            id: row.get(0)?,
            r#type: row.get(1)?,
            content: row.get(2)?,
            embedding_model: row.get(3)?,
            importance: row.get(4)?,
            confidence: row.get(5)?,
            access_count: row.get(6)?,
            last_used: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            source_conversation: row.get(10)?,
            status: row.get(11)?,
            deleted_at: row.get(12)?,
        })
    }
}
