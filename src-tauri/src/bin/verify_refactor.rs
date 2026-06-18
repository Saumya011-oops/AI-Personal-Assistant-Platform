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

async fn run_and_verify(retrieval_service: &RetrievalService, database: &Database, query: &str) -> Result<()> {
    println!("\n========================================================================");
    println!("RUNNING QUERY: \"{}\"", query);
    
    // Debug raw retrieval
    let raw_retrieval = retrieval_service.retrieve_documents(database, query).await?;
    println!("Query Analysis: Strategy={:?}, Filters={:?}, Complexity={:?}", 
        raw_retrieval.strategy_used, raw_retrieval.analysis.metadata_filters, raw_retrieval.analysis.complexity
    );
    println!("Raw Chunks Retrieved: {}", raw_retrieval.results.len());
    for (i, chunk) in raw_retrieval.results.iter().take(3).enumerate() {
        println!("  Raw Chunk {}: ID={}, Title=\"{}\", Score={:.4}, Content preview: \"{}\"",
            i + 1, chunk.chunk_id, chunk.document_title, chunk.score,
            chunk.content.replace('\n', " ").chars().take(80).collect::<String>()
        );
    }

    let start_time = std::time::Instant::now();
    let response = retrieval_service.ask_assistant(database, query).await?;
    let duration = start_time.elapsed();
    
    let report = response.confidence.as_ref().unwrap();
    println!("Response Status: {}", report.status);
    println!("Confidence Score: {}/100 ({})", report.confidence_score, report.confidence);
    println!("Ambiguity Score: {:?}", report.ambiguity_score);
    println!("Latency: {:?}", duration);
    println!("\n--- Answer ---");
    println!("{}", response.answer);
    println!("--------------");
    
    println!("\nCitations returned: {}", response.citations.len());
    for (i, cit) in response.citations.iter().enumerate() {
        println!("  {}. Source Doc: \"{}\" | Type: {} | Chunk ID: {} | Rerank Score: {:.4} | Retrieval Score: {:?}",
            i + 1,
            cit.source_document,
            cit.source_type,
            cit.chunk_id,
            cit.rerank_score,
            cit.retrieval_score
        );
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    println!("=== RAG Refactored Retrieval Verification ===");

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
    let cred_service = std::sync::Arc::new(assistant_core::services::CredentialService::new_no_handle().context("Failed to create credential service")?);
    let groq_service = GroqService::new(
        config.groq_api_key.clone(),
        Some(database.clone()),
        Some(cred_service),
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

    let retrieval_service = RetrievalService::new(
        ollama_service,
        qdrant_service,
        sparse_service,
        groq_service,
        query_analyzer_service,
        reranker_service,
        context_builder,
    );

    println!("Initializing Retrieval Service...");
    retrieval_service.initialize(&database).await.context("Failed to initialize retrieval service")?;
    println!("System ready. Starting verification queries.");

    let queries = vec![
        "Compare Notion and Obsidian integrations",
        "Compare Prometheus and Grafana",
        "How does onboarding connect to Notion setup?",
        "How does authentication interact with Qdrant access control?",
        "Difference between OAuth and token management",
        "Explain setup",
    ];

    for (i, query) in queries.iter().enumerate() {
        if i > 0 {
            println!("Sleeping 15 seconds to avoid Groq TPM rate limits...");
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
        run_and_verify(&retrieval_service, &database, query).await?;
    }

    println!("\n========================================================================");
    println!("All verification queries executed successfully!");
    Ok(())
}
