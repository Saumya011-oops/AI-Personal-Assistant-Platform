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
use assistant_core::domain::{QueryAnalysis, RetrievalStrategy, QueryComplexity, MetadataFilters, RetrievalMode};
use anyhow::Result;

struct QueryCase {
    query: &'static str,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== RUNNING DETAILED RETRIEVAL LATENCY BENCHMARK ===");

    let config = AppConfig::load()?;
    let database = Database::connect(&config.database_path)?;
    
    let ollama_service = OllamaService::new(config.ollama_url.clone(), config.embedding_model.clone());
    let qdrant_service = QdrantService::new(config.qdrant_url.clone(), config.qdrant_collection.clone());
    let sparse_service = SparseRetrievalService::new(
        config.sparse_helper_port,
        config.sparse_helper_script_path(),
        config.node_binary.clone(),
    );
    let cred_service = std::sync::Arc::new(assistant_core::services::CredentialService::new_no_handle()?);
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
        ollama_service.clone(),
        qdrant_service.clone(),
        sparse_service.clone(),
        groq_service,
        query_analyzer_service,
        reranker_service.clone(),
        context_builder,
    );

    retrieval_service.rebuild_topic_graph(&database).await?;

    let test_cases = [
        QueryCase { query: "Explain oauth" },
        QueryCase { query: "SSO integration" },
        QueryCase { query: "token management" },
        QueryCase { query: "How does OAuth authentication work" },
        QueryCase { query: "SSO configuration for enterprise" },
        QueryCase { query: "JWT access tokens" },
        QueryCase { query: "Set up Prometheus and Grafana" },
        QueryCase { query: "Grafana dashboard setup" },
        QueryCase { query: "System telemetry monitoring metrics" },
        QueryCase { query: "How to scale Qdrant cluster" },
        QueryCase { query: "Qdrant backup and recovery" },
        QueryCase { query: "Qdrant production setup" },
        QueryCase { query: "RAG hybrid search pipeline" },
        QueryCase { query: "recursive retrieval strategy" },
        QueryCase { query: "contextual retrieval strategy" },
        QueryCase { query: "RAG architecture overview" },
        QueryCase { query: "SQLite embedded storage" },
        QueryCase { query: "SQLite performance tuning" },
        QueryCase { query: "SQLite migration guide" },
        QueryCase { query: "Data Privacy and Trust strategy" },
        QueryCase { query: "Sub-50ms Query Milestones" },
        QueryCase { query: "Remote Work guidelines" },
        QueryCase { query: "Employee Handbook" },
        QueryCase { query: "Conflict Resolution Standard Operating Procedure" },
        QueryCase { query: "Compensation and Promotion Review Cycle" },
        QueryCase { query: "Annual Budget Allocation" },
        QueryCase { query: "Corporate Expense Reimbursement Policy" },
        QueryCase { query: "Desktop Client Native Sync" },
        QueryCase { query: "Threat Modeling security" },
        QueryCase { query: "Data encryption at rest" },
    ];

    // Warm up
    let _ = retrieval_service
        .retrieve_documents_with_mode(&database, "warmup", RetrievalMode::Evaluation)
        .await?;

    println!("\nStarting latency logging run...");
    for case in &test_cases {
        let _ = retrieval_service
            .retrieve_documents_with_mode(&database, case.query, RetrievalMode::Evaluation)
            .await?;
    }

    Ok(())
}
