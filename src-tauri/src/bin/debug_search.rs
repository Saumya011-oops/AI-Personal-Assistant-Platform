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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = AppConfig::load().context("Failed to load AppConfig")?;
    let database = Database::connect(&config.database_path).context("Failed to connect to database")?;
    
    let ollama_service = OllamaService::new(config.ollama_url.clone(), config.embedding_model.clone());
    let qdrant_service = QdrantService::new(config.qdrant_url.clone(), config.qdrant_collection.clone());
    let sparse_service = SparseRetrievalService::new(config.sparse_helper_port, config.sparse_helper_script_path(), config.node_binary.clone());
    let cred_service = std::sync::Arc::new(assistant_core::services::CredentialService::new_no_handle().context("Failed to create credential service")?);
    let groq_service = GroqService::new(config.groq_api_key.clone(), Some(database.clone()), Some(cred_service), config.groq_base_url.clone(), config.groq_model_primary.clone(), config.groq_model_fallback.clone());
    let query_analyzer_service = QueryAnalyzerService::new(groq_service.clone());
    let reranker_service = RerankerService::new(config.reranker_helper_port, config.reranker_worker_script_path(), config.reranker_python_path(), config.reranker_model.clone(), config.reranker_model_cache_dir.clone());
    let context_builder = ContextBuilder::new();

    let retrieval_service = RetrievalService::new(
        ollama_service.clone(),
        qdrant_service.clone(),
        sparse_service.clone(),
        groq_service,
        query_analyzer_service,
        reranker_service.clone(),
        context_builder,
    );

    retrieval_service.initialize(&database).await?;

    let query = "How does authentication interact with Qdrant access control?";
    println!("=== Retrieval for '{}' ===", query);
    let res = retrieval_service.retrieve_documents(&database, query).await?;
    for (i, c) in res.results.iter().enumerate() {
        println!("  {}. Title='{}', score={}, source={}", i + 1, c.document_title, c.score, c.source);
    }
    
    Ok(())
}
