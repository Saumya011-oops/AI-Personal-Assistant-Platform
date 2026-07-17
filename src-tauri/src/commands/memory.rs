use tauri::State;

use crate::domain::CommandEnvelope;
use crate::services::AppState;
use crate::services::memory::{DbChat, DbMemory, DbMessage};

#[tauri::command]
pub async fn create_chat(
    state: State<'_, AppState>,
    title: String,
) -> Result<CommandEnvelope<String>, String> {
    match state.memory_service.create_chat(&title) {
        Ok(id) => Ok(CommandEnvelope::success(id)),
        Err(e) => Ok(CommandEnvelope::error("CREATE_CHAT_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn list_chats(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<Vec<DbChat>>, String> {
    match state.memory_service.list_chats() {
        Ok(chats) => Ok(CommandEnvelope::success(chats)),
        Err(e) => Ok(CommandEnvelope::error("LIST_CHATS_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn search_chats(
    state: State<'_, AppState>,
    query: String,
) -> Result<CommandEnvelope<Vec<DbChat>>, String> {
    match state.memory_service.search_chats(&query) {
        Ok(chats) => Ok(CommandEnvelope::success(chats)),
        Err(e) => Ok(CommandEnvelope::error("SEARCH_CHATS_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn rename_chat(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<CommandEnvelope<()>, String> {
    match state.memory_service.rename_chat(&id, &title) {
        Ok(_) => Ok(CommandEnvelope::success(())),
        Err(e) => Ok(CommandEnvelope::error("RENAME_CHAT_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn delete_chat(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandEnvelope<()>, String> {
    match state.memory_service.delete_chat(&id) {
        Ok(_) => Ok(CommandEnvelope::success(())),
        Err(e) => Ok(CommandEnvelope::error("DELETE_CHAT_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn get_conversation_summary(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<CommandEnvelope<Option<String>>, String> {
    match state.memory_service.get_summary(&conversation_id) {
        Ok(summary) => Ok(CommandEnvelope::success(summary)),
        Err(e) => Ok(CommandEnvelope::error("GET_SUMMARY_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn load_chat_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<CommandEnvelope<Vec<DbMessage>>, String> {
    match state.memory_service.list_messages(&conversation_id) {
        Ok(messages) => Ok(CommandEnvelope::success(messages)),
        Err(e) => Ok(CommandEnvelope::error("LOAD_MESSAGES_FAILED", e.to_string())),
    }
}

// --- MEMORIES CONTROLS ---

#[tauri::command]
pub async fn list_memories(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<Vec<DbMemory>>, String> {
    match state.memory_service.list_memories() {
        Ok(memories) => Ok(CommandEnvelope::success(memories)),
        Err(e) => Ok(CommandEnvelope::error("LIST_MEMORIES_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn delete_memory(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandEnvelope<()>, String> {
    match state.memory_service.delete_memory(&id) {
        Ok(_) => Ok(CommandEnvelope::success(())),
        Err(e) => Ok(CommandEnvelope::error("DELETE_MEMORY_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn update_memory(
    state: State<'_, AppState>,
    id: String,
    content: String,
    importance: i64,
) -> Result<CommandEnvelope<()>, String> {
    match state.memory_service.update_memory(&id, &content, importance) {
        Ok(_) => Ok(CommandEnvelope::success(())),
        Err(e) => Ok(CommandEnvelope::error("UPDATE_MEMORY_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn clear_all_memories(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<()>, String> {
    match state.memory_service.clear_all_memories().await {
        Ok(_) => Ok(CommandEnvelope::success(())),
        Err(e) => Ok(CommandEnvelope::error("CLEAR_ALL_MEMORIES_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn export_memories(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<String>, String> {
    match state.memory_service.export_memories() {
        Ok(json) => Ok(CommandEnvelope::success(json)),
        Err(e) => Ok(CommandEnvelope::error("EXPORT_MEMORIES_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn import_memories(
    state: State<'_, AppState>,
    json_str: String,
) -> Result<CommandEnvelope<()>, String> {
    match state.memory_service.import_memories(&json_str) {
        Ok(_) => Ok(CommandEnvelope::success(())),
        Err(e) => Ok(CommandEnvelope::error("IMPORT_MEMORIES_FAILED", e.to_string())),
    }
}

#[tauri::command]
pub async fn reset_assistant_data(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<()>, String> {
    match state.database.reset_assistant_data() {
        Ok(_) => {
            // Also clear the Qdrant memory collection
            let _ = state.memory_service.clear_all_memories().await;
            // Clear document vector collections as well
            let _ = state.pipeline_service.qdrant_service().clear_collection().await;
            let _ = state.retrieval_service.clear_sparse_index().await;
            
            Ok(CommandEnvelope::success(()))
        }
        Err(e) => Ok(CommandEnvelope::error("RESET_ASSISTANT_FAILED", e.to_string())),
    }
}
