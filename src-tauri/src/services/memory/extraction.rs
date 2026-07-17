use std::sync::Arc;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services::ollama::OllamaService;
use crate::services::groq::GroqService;
use super::db::{MemoryDb, DbMemory};
use super::qdrant::MemoryQdrant;

#[derive(Debug, Deserialize, Serialize)]
pub struct ExtractedMemoryItem {
    pub r#type: String, // PROFILE, PREFERENCE, PROJECT, TASK, GOAL, TECHNOLOGY, SKILL, FACT, CONSTRAINT, RELATIONSHIP, EPISODE
    pub content: String,
    pub importance: i64, // 1-10
    pub action: String, // ADD | UPDATE | MERGE | IGNORE
    pub target_memory_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExtractedMemoriesResponse {
    pub memories: Vec<ExtractedMemoryItem>,
}

pub struct MemoryExtractor {
    db: Arc<MemoryDb>,
    ollama_service: OllamaService,
    groq_service: GroqService,
    qdrant: Arc<MemoryQdrant>,
}

impl MemoryExtractor {
    pub fn new(
        db: Arc<MemoryDb>,
        ollama_service: OllamaService,
        groq_service: GroqService,
        qdrant: Arc<MemoryQdrant>,
    ) -> Self {
        Self {
            db,
            ollama_service,
            groq_service,
            qdrant,
        }
    }

    /// Check if a user message is trivial (greetings, simple yes/no, short acknowledgments).
    pub fn is_trivial_message(&self, message: &str) -> bool {
        let trimmed = message.trim().to_lowercase();
        if trimmed.len() < 4 {
            return true;
        }

        // Trivial word list
        let trivial_words = [
            "hi", "hello", "hey", "yo", "thanks", "thank you", "ok", "okay",
            "yes", "no", "bye", "goodbye", "cool", "sure", "indeed", "awesome",
            "great", "fine", "test", "testing"
        ];

        for word in &trivial_words {
            if trimmed == *word || trimmed.starts_with(&format!("{} ", word)) || trimmed.ends_with(&format!(" {}", word)) {
                return true;
            }
        }

        false
    }

    /// Process conversation pair to extract new or update existing memories.
    pub async fn extract_and_consolidate(
        &self,
        conversation_id: &str,
        user_message: &str,
        assistant_response: &str,
    ) -> Result<()> {
        if self.is_trivial_message(user_message) {
            tracing::info!("Skipping memory extraction for trivial conversation segment");
            return Ok(());
        }

        let query_prefixed = format!("search_query: {}", user_message);
        let query_embeddings = self
            .ollama_service
            .generate_embeddings(&[query_prefixed])
            .await?;
        
        let Some(query_vector) = query_embeddings.into_iter().next() else {
            return Err(anyhow::anyhow!("No embedding returned for user message"));
        };

        // 2. Fetch similar memories from Qdrant
        let similar_points = match self.qdrant.search_similar_memories(query_vector, 8).await {
            Ok(pts) => pts,
            Err(e) => {
                tracing::warn!("Qdrant search failed, processing extraction without existing memory context: {:?}", e);
                Vec::new()
            }
        };

        let similar_ids: Vec<String> = similar_points.into_iter().map(|(id, _)| id).collect();
        let existing_memories = self.db.list_memories_by_ids(&similar_ids)?;

        // 3. Assemble existing memories block for prompt
        let mut existing_context = String::new();
        if existing_memories.is_empty() {
            existing_context.push_str("No existing similar memories found.\n");
        } else {
            existing_context.push_str("Existing similar memories:\n");
            for m in &existing_memories {
                existing_context.push_str(&format!(
                    "- [{}] Type: {}, Content: \"{}\", Importance: {}\n",
                    m.id, m.r#type, m.content, m.importance
                ));
            }
        }

        // 4. Construct System Prompt
        let system_prompt = r#"You are a Memory Extraction Engine for a local AI Personal Assistant.
Your task is to analyze the recent exchange and decide if there is any information that should be stored in the long-term memory system.

Supported memory types:
- PROFILE (user profile details, personal name, location, birthday, etc.)
- PREFERENCE (coding/text/UI preferences, preferred styles, tone)
- PROJECT (active projects the user is working on)
- TASK (specific tasks, to-dos, ongoing works)
- GOAL (user's long term aspirations or metrics)
- TECHNOLOGY (programming languages, libraries, frameworks used)
- SKILL (user's expertise or things they are learning)
- FACT (general persistent knowledge, custom rules, configurations)
- CONSTRAINT (specific limitations or absolute rules they want you to follow)
- RELATIONSHIP (connections between people or technologies)
- EPISODE (notable completed interactions or actions: e.g. "The user completed memory implementation.")

For each piece of memory found, you must compare it with the "Existing similar memories" provided and output one of these actions:
1. "ADD": For completely new details or facts.
2. "UPDATE": If it updates or modifies an existing memory (you MUST provide the target_memory_id of the memory being updated). E.g. "User prefers Rust" updates "User prefers Python".
3. "MERGE": If it is a duplicate or slight variation of an existing memory (you MUST provide the target_memory_id of the memory being merged).
4. "IGNORE": If the fact is already fully and accurately captured in an existing memory, or it is a trivial statement (greetings, temporary questions, random unhelpful facts).

Format your output STRICTLY as a JSON object:
{
  "memories": [
    {
      "type": "PREFERENCE",
      "content": "User prefers Rust.",
      "importance": 8, // Integer 1-10
      "action": "ADD", // ADD | UPDATE | MERGE | IGNORE
      "target_memory_id": null // String if action is UPDATE or MERGE, else null
    }
  ]
}

DO NOT include any markdown code fence surrounding the JSON. Output only the pure JSON structure."#;

        // 5. Construct User Prompt
        let user_prompt = format!(
            "Exchange:\n[User]: {}\n[Assistant]: {}\n\n{}\n\nExtract and consolidate memories based on the above exchange.",
            user_message, assistant_response, existing_context
        );

        // 6. Call Groq Service
        let response_value = self.groq_service.chat_json(system_prompt, &user_prompt).await?;
        let extraction: ExtractedMemoriesResponse = serde_json::from_value(response_value)
            .context("Failed to parse LLM response into ExtractedMemoriesResponse")?;

        // 7. Process extraction actions
        for item in extraction.memories {
            if item.action == "IGNORE" {
                continue;
            }

            match item.action.as_str() {
                "ADD" => {
                    let new_id = Uuid::new_v4().to_string();
                    let embedding_model = self.ollama_service.model().to_string();
                    
                    let db_mem = DbMemory {
                        id: new_id.clone(),
                        r#type: item.r#type.clone(),
                        content: item.content.clone(),
                        embedding_model: embedding_model.clone(),
                        importance: item.importance,
                        confidence: 1.0,
                        access_count: 0,
                        last_used: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        updated_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        source_conversation: Some(conversation_id.to_string()),
                        status: "active".to_string(),
                        deleted_at: None,
                    };

                    self.db.save_memory(&db_mem)?;

                    let content_prefixed = format!("search_document: {}", item.content);
                    if let Ok(embeddings) = self.ollama_service.generate_embeddings(&[content_prefixed]).await {
                        if let Some(vector) = embeddings.into_iter().next() {
                            let _ = self.qdrant.upsert_memory(&new_id, vector, &item.r#type, &item.content, item.importance).await;
                        }
                    }
                }
                "UPDATE" | "MERGE" => {
                    if let Some(ref target_id) = item.target_memory_id {
                        if let Some(mut existing_mem) = self.db.get_memory(target_id)? {
                            existing_mem.content = item.content.clone();
                            existing_mem.r#type = item.r#type.clone();
                            existing_mem.importance = item.importance;
                            existing_mem.confidence = (existing_mem.confidence + 1.0).min(5.0); // Increment confidence score on reinforcements
                            existing_mem.updated_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                            existing_mem.last_used = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

                            self.db.save_memory(&existing_mem)?;

                            let content_prefixed = format!("search_document: {}", item.content);
                            if let Ok(embeddings) = self.ollama_service.generate_embeddings(&[content_prefixed]).await {
                                if let Some(vector) = embeddings.into_iter().next() {
                                    let _ = self.qdrant.upsert_memory(target_id, vector, &item.r#type, &item.content, item.importance).await;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}
