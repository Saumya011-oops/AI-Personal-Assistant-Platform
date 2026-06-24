use anyhow::{Context, Result};
use assistant_core::config::AppConfig;
use assistant_core::db::Database;
use assistant_core::services::ollama::OllamaService;
use assistant_core::services::qdrant::QdrantService;
use assistant_core::services::sparse::SparseRetrievalService;
use assistant_core::services::groq::GroqService;
use assistant_core::services::query_analyzer::QueryAnalyzerService;
use assistant_core::services::reranker::RerankerService;
use assistant_core::services::context_builder::ContextBuilder;
use assistant_core::services::retrieval::RetrievalService;
use assistant_core::services::pipeline::PipelineService;
use assistant_core::services::SyncService;
use assistant_core::integrations::obsidian::ObsidianIntegration;
use assistant_core::integrations::notion::NotionIntegration;
use assistant_core::domain::{QueryAnalysis, QueryComplexity, RetrievalStrategy, MetadataFilters};

async fn reindex_database(
    config: &AppConfig,
    database: &Database,
    pipeline_service: &PipelineService,
    sync_service: &SyncService,
    retrieval_service: &RetrievalService,
    qdrant_service: &QdrantService,
) -> Result<()> {
    println!("\n=== Starting Reindexing Stage ===");
    println!("Clearing old database tables and vector collections...");
    database.document_repository().clear_all_documents().context("Failed to clear SQLite documents")?;
    qdrant_service.clear_collection().await.context("Failed to clear Qdrant collection")?;
    retrieval_service.clear_sparse_index().await.context("Failed to clear sparse index")?;
    println!("Database and collections cleared successfully.");

    let settings = database.settings_repository().get_settings().context("Failed to retrieve settings")?;
    let vault_path = settings.obsidian_vault_path.unwrap_or_else(|| {
        "/Users/saumyathacker/Documents/rag_sys/rag_sys".to_string()
    });
    println!("\nScanning and indexing Obsidian vault at: {}", vault_path);
    let obsidian = ObsidianIntegration::new(vault_path);
    
    // Sync metadata to SQLite
    let sync_run = sync_service.run_obsidian_sync(database, &obsidian).await.context("Failed to sync Obsidian metadata")?;
    println!("Obsidian Sync discovered {} documents.", sync_run.documents_discovered);

    // Sync Notion documents
    if config.notion_token.is_some() {
        println!("\nScanning and indexing Notion documents...");
        let notion = NotionIntegration::new(config.clone());
        let notion_sync_run = sync_service.run_notion_sync(database, &notion, config.notion_token.as_deref().unwrap_or("")).await.context("Failed to sync Notion documents")?;
        println!("Notion Sync discovered {} documents.", notion_sync_run.documents_discovered);
    } else {
        println!("\nNotion token not configured, skipping Notion sync.");
    }

    // Process and upload embeddings to Qdrant & SQLite chunks
    println!("\nGenerating embeddings and indexing points for all synced documents...");
    let processed = pipeline_service.process_all_pending_documents(database).await.context("Failed to process document embeddings")?;
    println!("Successfully processed and uploaded embeddings for {} documents.", processed);
    println!("=== Reindexing Stage Completed ===\n");

    Ok(())
}

async fn test_strategy(
    retrieval_service: &RetrievalService,
    database: &Database,
    query: &str,
    strategy: RetrievalStrategy,
    filters: MetadataFilters,
) -> Result<()> {
    println!("\n========================================================================");
    println!("TESTING STRATEGY: {:?}", strategy);
    println!("Query: \"{}\"", query);
    if filters.tags.is_some() || filters.category.is_some() || filters.source.is_some() {
        println!("Metadata Filters: {:?}", filters);
    }

    let analysis = QueryAnalysis {
        intent: "search".to_string(),
        entities: Vec::new(),
        metadata_filters: filters,
        temporal: matches!(strategy, RetrievalStrategy::Contextual),
        complexity: if matches!(strategy, RetrievalStrategy::Recursive) { QueryComplexity::Complex } else { QueryComplexity::Simple },
        strategy: strategy.clone(),
    };

    let start_time = std::time::Instant::now();
    let results = retrieval_service
        .retrieve_with_strategy(database, query, &strategy, &analysis, 0)
        .await?;
    let duration = start_time.elapsed();

    println!("Results retrieved: {} (took {:?})", results.len(), duration);
    for (i, chunk) in results.iter().take(3).enumerate() {
        println!("  {}. [{}] Title: \"{}\" | Score: {:.4} | Tags: {:?}", 
            i + 1, 
            chunk.source, 
            chunk.document_title, 
            chunk.score,
            chunk.tags
        );
        let clean_content = chunk.content.replace('\n', " ");
        let snippet = if clean_content.len() > 150 {
            format!("{}...", &clean_content[..150].trim())
        } else {
            clean_content.trim().to_string()
        };
        println!("     Snippet: \"{}\"", snippet);
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    println!("=== RAG Strategies Verification and Performance Testing ===");

    // Load configuration
    let config = AppConfig::load().context("Failed to load AppConfig")?;
    
    // Connect to Database
    let database = Database::connect(&config.database_path).context("Failed to connect to database")?;
    
    // Initialize services
    let ollama_service = OllamaService::new(
        config.ollama_url.clone(),
        config.embedding_model.clone(),
    );
    let qdrant_service = QdrantService::new(
        config.qdrant_url.clone(),
        config.qdrant_collection.clone(),
    );
    let sparse_service = SparseRetrievalService::new(
        config.sparse_helper_port,
        config.sparse_helper_script_path(),
        config.node_binary.clone(),
    );
    let groq_service = GroqService::new(
        config.groq_api_key.clone(),
        None,
        None,
        config.groq_base_url.clone(),
        config.groq_model_primary.clone(),
        config.groq_model_fallback.clone(),
    );
    let query_analyzer_service = QueryAnalyzerService::new(groq_service.clone());
    let reranker_service = RerankerService::new(
        config.reranker_helper_port,
        config.reranker_worker_script_path(),
        config.reranker_python_path(),
        config.reranker_model.clone(),
        config.reranker_model_cache_dir.clone(),
    );
    let context_builder = ContextBuilder::new();

    let pipeline_service = PipelineService::new(
        ollama_service.clone(),
        qdrant_service.clone(),
        sparse_service.clone(),
    );

    let retrieval_service = RetrievalService::new(
        ollama_service,
        qdrant_service.clone(),
        sparse_service,
        groq_service,
        query_analyzer_service,
        reranker_service,
        context_builder,
    );

    println!("Initializing Retrieval Service indices and helper processes...");
    retrieval_service.initialize(&database).await.context("Failed to initialize retrieval service")?;
    println!("All systems operational.");

    // Perform reindexing
    let sync_service = SyncService::new();
    reindex_database(&config, &database, &pipeline_service, &sync_service, &retrieval_service, &qdrant_service).await?;

    println!("Starting verification tests...\n");

    // 1. Dense Strategy
    test_strategy(
        &retrieval_service,
        &database,
        "Explain our authentication flow and PKCE architecture",
        RetrievalStrategy::Dense,
        MetadataFilters::default(),
    ).await?;

    // 2. Sparse Strategy
    test_strategy(
        &retrieval_service,
        &database,
        "JWT oauth token endpoint",
        RetrievalStrategy::Sparse,
        MetadataFilters::default(),
    ).await?;

    // 3. Hybrid Strategy (dense + sparse fusion)
    test_strategy(
        &retrieval_service,
        &database,
        "What is our policy and response targets for P1 severity incidents?",
        RetrievalStrategy::Hybrid,
        MetadataFilters::default(),
    ).await?;

    // 4. Faceted Strategy (filtering on category/tags)
    test_strategy(
        &retrieval_service,
        &database,
        "List high memory usage issues in the Support category",
        RetrievalStrategy::Faceted,
        MetadataFilters {
            source: None,
            author: None,
            tags: Some(vec!["troubleshooting".to_string()]),
            category: Some(vec!["support".to_string()]),
            date_range: None,
            document_type: None,
            topic: None,
        },
    ).await?;

    // 5. Contextual Strategy (time-decay or event boosting)
    test_strategy(
        &retrieval_service,
        &database,
        "outage reports and downtime timeline",
        RetrievalStrategy::Contextual,
        MetadataFilters::default(),
    ).await?;

    // 6. Recursive Strategy (multi-hop parent context synthesis)
    test_strategy(
        &retrieval_service,
        &database,
        "How is Tauri integrated with SQLite and the Rust background watchers?",
        RetrievalStrategy::Recursive,
        MetadataFilters::default(),
    ).await?;

    println!("\n========================================================================");
    println!("All retrieval strategies executed successfully!");
    Ok(())
}
