pub mod db;
pub mod qdrant;
pub mod queue;
pub mod extraction;
pub mod ranking;

use std::sync::Arc;
use anyhow::Result;
use serde_json::Value;
use uuid::Uuid;

use crate::db::Database;
use crate::services::ollama::OllamaService;
use crate::services::groq::GroqService;

pub use self::db::{DbChat, DbMessage, DbMemory};
use self::db::MemoryDb;
use self::qdrant::MemoryQdrant;
use self::queue::{MemoryQueue, MemoryJob};
pub use self::ranking::RankedMemory;

#[derive(Clone)]
pub struct MemoryService {
    db: Arc<MemoryDb>,
    ollama_service: OllamaService,
    groq_service: GroqService,
    qdrant: Arc<MemoryQdrant>,
    queue: Arc<MemoryQueue>,
}

impl MemoryService {
    pub fn new(
        _database: Database,
        ollama_service: OllamaService,
        groq_service: GroqService,
        qdrant_url: &str,
    ) -> Self {
        let qdrant = Arc::new(MemoryQdrant::new(qdrant_url));
        let queue = Arc::new(MemoryQueue::new());
        let db = Arc::new(MemoryDb::new(_database.get_connection().clone()));

        Self {
            db,
            ollama_service,
            groq_service,
            qdrant,
            queue,
        }
    }
    pub fn qdrant(&self) -> &Arc<MemoryQdrant> {
        &self.qdrant
    }
    /// Initializes Qdrant collections.
    pub async fn initialize(&self) -> Result<()> {
        let _ = self.qdrant.initialize().await;

        // Spawn queue worker loop under the active Tokio runtime context
        let mut rx_guard = self.queue.receiver.lock().await;
        if let Some(receiver) = rx_guard.take() {
            let db = self.db.clone();
            let ollama = self.ollama_service.clone();
            let groq = self.groq_service.clone();
            let qdrant = self.qdrant.clone();

            tokio::spawn(async move {
                MemoryQueue::worker_loop(receiver, db, ollama, groq, qdrant).await;
            });
        }

        Ok(())
    }

    // --- CHATS & CONVERSATIONS ---

    pub fn create_chat(&self, title: &str) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        self.db.create_chat(&id, title)?;
        Ok(id)
    }

    pub fn get_chat(&self, id: &str) -> Result<Option<DbChat>> {
        self.db.get_chat(id)
    }

    pub fn list_chats(&self) -> Result<Vec<DbChat>> {
        self.db.list_chats()
    }

    pub fn search_chats(&self, query_text: &str) -> Result<Vec<DbChat>> {
        self.db.search_chats(query_text)
    }

    pub fn rename_chat(&self, id: &str, title: &str) -> Result<()> {
        self.db.rename_chat(id, title)
    }

    pub fn delete_chat(&self, id: &str) -> Result<()> {
        self.db.delete_chat(id)
    }

    pub fn get_summary(&self, conversation_id: &str) -> Result<Option<String>> {
        self.db.get_summary(conversation_id)
    }

    // --- MESSAGES ---

    pub fn save_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        token_count: i64,
        retrieved_document_ids: Option<Vec<String>>,
        retrieved_memory_ids: Option<Vec<String>>,
        citations_json: Option<Value>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let doc_ids_str = retrieved_document_ids.map(|ids| ids.join(","));
        let mem_ids_str = retrieved_memory_ids.map(|ids| ids.join(","));
        let citations_str = citations_json.map(|v| v.to_string());

        let msg = DbMessage {
            id: id.clone(),
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            token_count,
            retrieved_document_ids: doc_ids_str,
            retrieved_memory_ids: mem_ids_str,
            citations: citations_str,
            created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };

        self.db.save_message(&msg)?;
        Ok(id)
    }

    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<DbMessage>> {
        self.db.list_messages(conversation_id)
    }

    // --- SHORT TERM MEMORY & TRIMMING ---

    pub async fn get_chat_history_for_prompt(
        &self,
        conversation_id: &str,
        _limit_tokens: usize, // budget token limit parameter
    ) -> Result<(Option<String>, Vec<DbMessage>)> {
        let messages = self.db.list_messages(conversation_id)?;
        if messages.is_empty() {
            return Ok((None, Vec::new()));
        }

        let summary = self.db.get_summary(conversation_id)?;

        // Automatically trigger summarization in background if total messages > 40
        if messages.len() > 40 {
            let db_clone = self.db.clone();
            let groq_clone = self.groq_service.clone();
            let cid = conversation_id.to_string();
            let msgs_to_summarize = messages[..messages.len() - 10].to_vec();
            let last_msg_id = msgs_to_summarize.last().map(|m| m.id.clone()).unwrap_or_default();

            tokio::spawn(async move {
                if let Err(e) = Self::generate_and_save_summary(db_clone, groq_clone, &cid, &msgs_to_summarize, &last_msg_id).await {
                    tracing::error!("Failed to generate conversation summary: {:?}", e);
                }
            });
        }

        // Return recent 10 messages along with the active summary
        let recent_count = 10;
        let start_idx = if messages.len() > recent_count {
            messages.len() - recent_count
        } else {
            0
        };

        let recent_messages = messages[start_idx..].to_vec();
        Ok((summary, recent_messages))
    }

    async fn generate_and_save_summary(
        db: Arc<MemoryDb>,
        groq_service: GroqService,
        conversation_id: &str,
        messages: &[DbMessage],
        last_message_id: &str,
    ) -> Result<()> {
        let mut transcript = String::new();
        for m in messages {
            transcript.push_str(&format!("{}: {}\n", m.role, m.content));
        }

        let system_prompt = "You are a helpful AI assistant. Summarize the conversation history concisely, focusing on core facts, ongoing topics, and user preferences.";
        let user_prompt = format!("Summarize this conversation transcript:\n\n{}", transcript);

        let summary = groq_service.chat_text(system_prompt, &user_prompt).await?;
        db.save_summary(conversation_id, &summary, last_message_id)?;
        Ok(())
    }

    // --- LONG TERM MEMORY RETRIEVAL & RANKING ---

    pub async fn retrieve_memories_for_query(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<RankedMemory>> {
        // Generate embedding for query
        let query_prefixed = format!("search_query: {}", query);
        let query_embeddings = match self
            .ollama_service
            .generate_embeddings(&[query_prefixed])
            .await {
                Ok(embeddings) => embeddings,
                Err(e) => {
                    tracing::warn!("Ollama embeddings failed, falling back to database keyword-based memory retrieval: {:?}", e);
                    // FALLBACK: Query DB directly for memories matching keywords
                    let keywords: Vec<String> = query.split_whitespace()
                        .map(|s| s.to_lowercase().replace(&['?', '!', '.', ',', '"', '\''][..], ""))
                        .filter(|s| s.len() > 2)
                        .collect();
                    if keywords.is_empty() {
                        return Ok(Vec::new());
                    }
                    let all_memories = self.db.list_memories()?;
                    let mut matched = Vec::new();
                    for mem in all_memories {
                        if mem.status != "active" { continue; }
                        let mut score = 0.0;
                        let content_lower = mem.content.to_lowercase();
                        for kw in &keywords {
                            if content_lower.contains(kw) {
                                score += 0.3;
                            }
                        }
                        if score > 0.0 {
                            matched.push((mem, score as f32));
                        }
                    }
                    let mut ranked = ranking::rank_memories(matched);
                    ranked.truncate(limit);
                    return Ok(ranked);
                }
            };
        
        let Some(query_vector) = query_embeddings.into_iter().next() else {
            return Ok(Vec::new());
        };

        // Query Qdrant for semantic similar memories (fetch extra candidates for ranking)
        let similar_points = match self.qdrant.search_similar_memories(query_vector, limit * 3).await {
            Ok(pts) => pts,
            Err(e) => {
                tracing::warn!("Failed to query memory vectors from Qdrant: {:?}", e);
                return Ok(Vec::new());
            }
        };

        if similar_points.is_empty() {
            return Ok(Vec::new());
        }

        let similar_ids: Vec<String> = similar_points.iter().map(|(id, _)| id.clone()).collect();
        let db_memories = self.db.list_memories_by_ids(&similar_ids)?;

        let mut candidates = Vec::new();
        for mem in db_memories {
            if let Some(pos) = similar_ids.iter().position(|id| *id == mem.id) {
                let mut score = similar_points[pos].1;
                
                // Add token boundary exact match boost
                let query_lower = query.to_lowercase();
                let content_lower = mem.content.to_lowercase();
                let mut is_match = false;
                if let Some(idx) = query_lower.find(&content_lower) {
                    let char_before = if idx > 0 { query_lower.chars().nth(idx - 1) } else { None };
                    let char_after = query_lower.chars().nth(idx + content_lower.len());
                    
                    let before_boundary = char_before.map_or(true, |c| !c.is_alphanumeric());
                    let after_boundary = char_after.map_or(true, |c| !c.is_alphanumeric());
                    
                    if before_boundary && after_boundary {
                        is_match = true;
                    }
                }
                
                if is_match {
                    score += 0.15;
                }
                
                candidates.push((mem, score));
            }
        }

        // Apply ranking formula and sort
        let mut ranked = ranking::rank_memories(candidates);
        ranked.truncate(limit);

        // Increment access count for retrieved memories in background
        let db_clone = self.db.clone();
        let accessed_ids: Vec<String> = ranked.iter().map(|rm| rm.memory.id.clone()).collect();
        tokio::spawn(async move {
            for id in accessed_ids {
                let _ = db_clone.increment_memory_access(&id);
            }
        });

        Ok(ranked)
    }

    // --- BACKGROUND JOBS ---

    pub fn queue_memory_extraction(
        &self,
        conversation_id: &str,
        user_message: &str,
        assistant_response: &str,
    ) -> Result<()> {
        self.queue.enqueue(MemoryJob::ExtractMemory {
            conversation_id: conversation_id.to_string(),
            user_message: user_message.to_string(),
            assistant_response: assistant_response.to_string(),
        })
    }

    pub async fn extract_memories_synchronously(
        &self,
        conversation_id: &str,
        user_message: &str,
        assistant_response: &str,
    ) -> Result<()> {
        let extractor = self::extraction::MemoryExtractor::new(
            self.db.clone(),
            self.ollama_service.clone(),
            self.groq_service.clone(),
            self.qdrant.clone(),
        );
        extractor.extract_and_consolidate(conversation_id, user_message, assistant_response).await
    }

    // --- MEMORIES CRUD ---

    pub fn list_memories(&self) -> Result<Vec<DbMemory>> {
        self.db.list_memories()
    }

    pub fn delete_memory(&self, id: &str) -> Result<()> {
        self.db.soft_delete_memory(id)?;
        let qdrant_clone = self.qdrant.clone();
        let id_str = id.to_string();
        tokio::spawn(async move {
            let _ = qdrant_clone.delete_memory(&id_str).await;
        });
        Ok(())
    }

    pub fn update_memory(&self, id: &str, content: &str, importance: i64) -> Result<()> {
        if let Some(mut mem) = self.db.get_memory(id)? {
            mem.content = content.to_string();
            mem.importance = importance;
            mem.updated_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            self.db.save_memory(&mem)?;

            // Re-generate vector embedding and upsert to Qdrant
            let ollama_clone = self.ollama_service.clone();
            let qdrant_clone = self.qdrant.clone();
            let id_str = id.to_string();
            let content_str = content.to_string();
            let type_str = mem.r#type.clone();
            
            tokio::spawn(async move {
                let content_prefixed = format!("search_document: {}", content_str);
                if let Ok(embeddings) = ollama_clone.generate_embeddings(&[content_prefixed]).await {
                    if let Some(vector) = embeddings.into_iter().next() {
                        let _ = qdrant_clone.upsert_memory(&id_str, vector, &type_str, &content_str, importance).await;
                    }
                }
            });
        }
        Ok(())
    }

    pub async fn clear_all_memories(&self) -> Result<()> {
        self.db.clear_all_memories()?;
        self.qdrant.clear_collection().await?;
        Ok(())
    }

    pub fn export_memories(&self) -> Result<String> {
        let memories = self.db.list_memories()?;
        let json_str = serde_json::to_string_pretty(&memories)?;
        Ok(json_str)
    }

    pub fn import_memories(&self, json_str: &str) -> Result<()> {
        let memories: Vec<DbMemory> = serde_json::from_str(json_str)?;
        for mut mem in memories {
            mem.updated_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            self.db.save_memory(&mem)?;

            let ollama_clone = self.ollama_service.clone();
            let qdrant_clone = self.qdrant.clone();
            let id_str = mem.id.clone();
            let content_str = mem.content.clone();
            let type_str = mem.r#type.clone();
            let importance = mem.importance;

            tokio::spawn(async move {
                let content_prefixed = format!("search_document: {}", content_str);
                if let Ok(embeddings) = ollama_clone.generate_embeddings(&[content_prefixed]).await {
                    if let Some(vector) = embeddings.into_iter().next() {
                        let _ = qdrant_clone.upsert_memory(&id_str, vector, &type_str, &content_str, importance).await;
                    }
                }
            });
        }
        Ok(())
    }
}
