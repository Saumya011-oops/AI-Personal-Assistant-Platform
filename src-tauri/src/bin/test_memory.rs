use tokio::time::{sleep, Duration};
use assistant_core::services::memory::{MemoryService, DbMemory};
use assistant_core::db::Database;
use assistant_core::config::AppConfig;
use assistant_core::services::ollama::OllamaService;
use assistant_core::services::groq::GroqService;
use assistant_core::services::memory::ranking::rank_memories;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    tracing::info!("Initializing Test Database and Config...");
    let config = AppConfig::load()?;
    let database = Database::connect(&config.database_path)?;
    database.run_migrations()?;

    let ollama_service = OllamaService::new(
        config.ollama_url.clone(),
        config.embedding_model.clone(),
    );
    let groq_service = GroqService::new(
        config.groq_api_key.clone(),
        Some(database.clone()),
        None,
        config.groq_base_url.clone(),
        config.groq_model_primary.clone(),
        config.groq_model_fallback.clone(),
    );

    let memory_service = MemoryService::new(
        database.clone(),
        ollama_service.clone(),
        groq_service.clone(),
        &config.qdrant_url,
    );

    tracing::info!("Initializing Qdrant Memories collection...");
    memory_service.initialize().await?;

    // --- TEST 1: Chat and Message Persistence ---
    tracing::info!("Running TEST 1: Chat and Message Persistence...");
    let chat_id = memory_service.create_chat("Test Chat Title")?;
    tracing::info!("Created Chat ID: {}", chat_id);

    memory_service.save_message(&chat_id, "user", "Hello, assistant. I prefer writing code in Rust.", 10, None, None, None)?;
    memory_service.save_message(&chat_id, "assistant", "I will remember that you prefer Rust.", 10, None, None, None)?;

    let messages = memory_service.list_messages(&chat_id)?;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    tracing::info!("TEST 1 Passed!");

    // --- TEST 2: Memory Ranking and Normalization ---
    tracing::info!("Running TEST 2: Memory Ranking & Score Normalization...");
    let mem1 = DbMemory {
        id: "mem1".to_string(),
        r#type: "PREFERENCE".to_string(),
        content: "User prefers Rust".to_string(),
        embedding_model: "test-model".to_string(),
        importance: 8,
        confidence: 1.0,
        access_count: 5,
        last_used: "2026-07-02 12:00:00".to_string(),
        created_at: "2026-07-02 12:00:00".to_string(),
        updated_at: "2026-07-02 12:00:00".to_string(),
        source_conversation: Some(chat_id.clone()),
        status: "active".to_string(),
        deleted_at: None,
    };
    let mem2 = DbMemory {
        id: "mem2".to_string(),
        r#type: "TECHNOLOGY".to_string(),
        content: "User works with Tauri".to_string(),
        embedding_model: "test-model".to_string(),
        importance: 4,
        confidence: 1.0,
        access_count: 1,
        last_used: "2026-07-01 12:00:00".to_string(),
        created_at: "2026-07-01 12:00:00".to_string(),
        updated_at: "2026-07-01 12:00:00".to_string(),
        source_conversation: Some(chat_id.clone()),
        status: "active".to_string(),
        deleted_at: None,
    };

    let ranked = rank_memories(vec![(mem1, 0.9f32), (mem2, 0.6f32)]);
    assert_eq!(ranked.len(), 2);
    assert!(ranked[0].final_score > ranked[1].final_score);
    assert_eq!(ranked[0].memory.id, "mem1");
    tracing::info!("TEST 2 Passed! Normalized ranking works correctly.");

    // --- TEST 3: Memory Queue Reliability & Qdrant Synchronization ---
    tracing::info!("Running TEST 3: Memory Queue & Extraction...");
    if groq_service.is_configured() {
        memory_service.queue_memory_extraction(&chat_id, "I am saumya. I love programming in Rust, and my current project is a desktop assistant.", "Got it. You are saumya, you love Rust, and your current project is a desktop assistant.")?;
        
        tracing::info!("Waiting for background extraction queue to process...");
        sleep(Duration::from_secs(8)).await;

        let memories = memory_service.list_memories()?;
        tracing::info!("Extracted memories count: {}", memories.len());
        for m in &memories {
            tracing::info!("- Memory: \"{}\" [{}] Category: {}", m.content, m.id, m.r#type);
        }
        assert!(memories.len() > 0);
        tracing::info!("TEST 3 Passed!");
    } else {
        tracing::warn!("Groq is not configured. Skipping LLM-based extraction test.");
    }

    // --- TEST 4: Cleanup & Memory Reset ---
    tracing::info!("Running TEST 4: Clear all memories...");
    memory_service.clear_all_memories()?;
    let remaining_mems = memory_service.list_memories()?;
    assert_eq!(remaining_mems.len(), 0);
    tracing::info!("TEST 4 Passed!");

    tracing::info!("All backend memory verification tests completed successfully!");
    Ok(())
}
