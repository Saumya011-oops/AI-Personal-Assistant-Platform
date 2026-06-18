use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use serde_json::json;
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

#[derive(Debug, Deserialize, Clone)]
struct EvalQuery {
    id: String,
    category: String,
    query: String,
    #[serde(rename = "expected_keywords")]
    expected_keywords: Vec<String>,
    #[serde(rename = "expected_status")]
    expected_status: Vec<String>,
    #[serde(rename = "expected_citation_min")]
    expected_citation_min: usize,
    required_documents: Option<Vec<String>>,
    required_entities: Option<Vec<String>>,
    required_topics: Option<Vec<String>>,
    required_answer_terms: Option<Vec<String>>,
    forbidden_answer_terms: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct EvalResult {
    id: String,
    category: String,
    query: String,
    status: String,
    confidence_score: u32,
    latency_ms: u64,
    citations_count: usize,
    passed: bool,
    reasons: Vec<String>,
    answer: String,
    entity_recall: f32,
    document_recall: f32,
    topic_recall: f32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    println!("=== RAG Retrieval Evaluation Suite ===");

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
    let cred_service = std::sync::Arc::new(
        assistant_core::services::CredentialService::new_no_handle()
            .context("Failed to create credential service")?
    );
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
    println!("System ready. Loading evaluation queries.");

    // Load queries
    let queries_file = File::open("src-tauri/eval_queries.json").context("Failed to open eval_queries.json")?;
    let queries: Vec<EvalQuery> = serde_json::from_reader(queries_file).context("Failed to parse eval_queries.json")?;

    let mut results = Vec::new();

    for (idx, q) in queries.iter().enumerate() {
        println!("\n[{}/{}] Running ID: {} Category: {} | Query: \"{}\"", idx + 1, queries.len(), q.id, q.category, q.query);
        
        let start = Instant::now();
        let response_res = retrieval_service.ask_assistant(&database, &q.query).await;
        let latency = start.elapsed().as_millis() as u64;

        match response_res {
            Ok(response) => {
                let report = response.confidence.as_ref().expect("No confidence report returned");
                let citations_count = response.citations.len();
                
                let mut passed = true;
                let mut reasons = Vec::new();

                // 1. Status check
                if !q.expected_status.contains(&report.status) {
                    passed = false;
                    reasons.push(format!("Status mismatch: expected {:?}, got {}", q.expected_status, report.status));
                }

                // 2. Citation count check
                if citations_count < q.expected_citation_min {
                    passed = false;
                    reasons.push(format!("Citation count insufficient: expected >= {}, got {}", q.expected_citation_min, citations_count));
                }

                // 3. Keyword check
                let answer_lower = response.answer.to_lowercase();
                for kw in &q.expected_keywords {
                    if !answer_lower.contains(&kw.to_lowercase()) {
                        passed = false;
                        reasons.push(format!("Missing expected keyword: \"{}\"", kw));
                    }
                }

                // 4. Fetch full chunk details from SQLite for exact recall check
                let chunk_ids: Vec<String> = response.citations.iter().map(|c| c.chunk_id.clone()).collect();
                let search_docs = database.document_repository().get_chunk_search_documents_by_ids(&chunk_ids).unwrap_or_default();

                // Calculate Document Recall
                let required_docs = q.required_documents.clone().unwrap_or_default();
                let document_recall = if required_docs.is_empty() {
                    1.0
                } else {
                    let mut matched = 0;
                    for req_doc in &required_docs {
                        let req_doc_lower = req_doc.to_lowercase();
                        let req_doc_spaces = req_doc_lower.replace('_', " ");
                        let found = search_docs.iter().any(|doc| {
                            let doc_title = doc.title.to_lowercase();
                            let doc_id = doc.document_id.to_lowercase();
                            doc_title.contains(&req_doc_lower) || doc_title.contains(&req_doc_spaces) ||
                            doc_id.contains(&req_doc_lower) || doc_id.contains(&req_doc_spaces)
                        });
                        if found {
                            matched += 1;
                        }
                    }
                    matched as f32 / required_docs.len() as f32
                };

                // Extract full text from chunks for entity/topic recall
                let all_chunks_text = {
                    let mut text = String::new();
                    for doc in &search_docs {
                        text.push_str(&doc.content.to_lowercase());
                        text.push_str(" ");
                        text.push_str(&doc.title.to_lowercase());
                        text.push_str(" ");
                    }
                    text
                };

                // 5. Entity recall check
                let required_entities = q.required_entities.clone().unwrap_or_default();
                let entity_recall = if required_entities.is_empty() {
                    1.0
                } else {
                    let mut matched = 0;
                    for req_entity in &required_entities {
                        if all_chunks_text.contains(&req_entity.to_lowercase()) {
                            matched += 1;
                        }
                    }
                    matched as f32 / required_entities.len() as f32
                };

                // 6. Topic recall check
                let required_topics = q.required_topics.clone().unwrap_or_default();
                let topic_recall = if required_topics.is_empty() {
                    1.0
                } else {
                    let mut matched = 0;
                    for req_topic in &required_topics {
                        if all_chunks_text.contains(&req_topic.to_lowercase()) {
                            matched += 1;
                        }
                    }
                    matched as f32 / required_topics.len() as f32
                };

                if document_recall < 1.0 {
                    reasons.push(format!("Document recall below 100% (info only): expected all of {:?}, got recall {:.2}", required_docs, document_recall));
                }
                if entity_recall < 1.0 {
                    passed = false;
                    reasons.push(format!("Entity recall below 100%: expected all of {:?}, got recall {:.2}", required_entities, entity_recall));
                }
                if topic_recall < 1.0 {
                    passed = false;
                    reasons.push(format!("Topic recall below 100%: expected all of {:?}, got recall {:.2}", required_topics, topic_recall));
                }

                // 7. Required answer terms check
                let required_answer_terms = q.required_answer_terms.clone().unwrap_or_default();
                for term in &required_answer_terms {
                    if !answer_lower.contains(&term.to_lowercase()) {
                        passed = false;
                        reasons.push(format!("Missing required answer term: \"{}\"", term));
                    }
                }

                // 8. Forbidden answer terms check
                let forbidden_answer_terms = q.forbidden_answer_terms.clone().unwrap_or_default();
                for term in &forbidden_answer_terms {
                    if answer_lower.contains(&term.to_lowercase()) {
                        passed = false;
                        reasons.push(format!("Contains forbidden answer term: \"{}\"", term));
                    }
                }

                // 9. Category J Custom Verification Assertions
                if q.category == "J" {
                    let q_lower = q.query.to_lowercase();
                    if q_lower.contains("compare") || q_lower.contains("difference") {
                        let mut targets = Vec::new();
                        for t in &["notion", "obsidian", "prometheus", "grafana", "oauth", "token"] {
                            if q_lower.contains(t) {
                                targets.push(*t);
                            }
                        }
                        for target in &targets {
                            if !all_chunks_text.contains(target) {
                                passed = false;
                                reasons.push(format!("Verification failed: Missing citations for comparison target \"{}\"", target));
                            }
                        }
                    } else if q_lower.contains("connect") || q_lower.contains("interact") || q_lower.contains("relationship") || q_lower.contains("relate") {
                        let mut targets = Vec::new();
                        for t in &["onboarding", "notion", "authentication", "qdrant"] {
                            if q_lower.contains(t) {
                                targets.push(*t);
                            }
                        }
                        for target in &targets {
                            if !all_chunks_text.contains(target) {
                                passed = false;
                                reasons.push(format!("Verification failed: Missing citations for relationship target \"{}\"", target));
                            }
                        }

                        let has_bridge = all_chunks_text.contains("permission") || all_chunks_text.contains("access") || all_chunks_text.contains("token") || all_chunks_text.contains("setup");
                        if !has_bridge {
                            passed = false;
                            reasons.push("Verification failed: Missing bridge evidence citation (setup/permission/access/control/token)".to_string());
                        }
                    }
                }

                if passed {
                    println!("  Result: PASS | Status: {} | Citations: {} | Latency: {}ms | Doc Recall: {:.1}% | Entity Recall: {:.1}% | Topic Recall: {:.1}%", report.status, citations_count, latency, document_recall * 100.0, entity_recall * 100.0, topic_recall * 100.0);
                } else {
                    println!("  Result: FAIL | Status: {} | Citations: {} | Latency: {}ms | Doc Recall: {:.1}% | Entity Recall: {:.1}% | Topic Recall: {:.1}%", report.status, citations_count, latency, document_recall * 100.0, entity_recall * 100.0, topic_recall * 100.0);
                    for r in &reasons {
                        println!("    - {}", r);
                    }
                }

                results.push(EvalResult {
                    id: q.id.clone(),
                    category: q.category.clone(),
                    query: q.query.clone(),
                    status: report.status.clone(),
                    confidence_score: report.confidence_score,
                    latency_ms: latency,
                    citations_count,
                    passed,
                    reasons,
                    answer: response.answer,
                    entity_recall,
                    document_recall,
                    topic_recall,
                });
            }
            Err(e) => {
                println!("  Result: ERROR | Error: {}", e);
                results.push(EvalResult {
                    id: q.id.clone(),
                    category: q.category.clone(),
                    query: q.query.clone(),
                    status: "ERROR".to_string(),
                    confidence_score: 0,
                    latency_ms: latency,
                    citations_count: 0,
                    passed: false,
                    reasons: vec![format!("Execution error: {}", e)],
                    answer: String::new(),
                    entity_recall: 0.0,
                    document_recall: 0.0,
                    topic_recall: 0.0,
                });
            }
        }

        // Rate limit avoidance sleep
        if idx + 1 < queries.len() {
            println!("Sleeping 10s to respect API rate limits...");
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    }

    // Compute metrics
    let total = results.len();
    let passed_count = results.iter().filter(|r| r.passed).count();
    let overall_score = (passed_count as f32 / total as f32) * 100.0;

    let categories = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];
    let mut category_scores = serde_json::Map::new();

    for cat in &categories {
        let cat_results: Vec<&EvalResult> = results.iter().filter(|r| &r.category == cat).collect();
        if !cat_results.is_empty() {
            let cat_passed = cat_results.iter().filter(|r| r.passed).count();
            let cat_score = (cat_passed as f32 / cat_results.len() as f32) * 100.0;
            category_scores.insert(cat.to_string(), json!(cat_score));
        } else {
            category_scores.insert(cat.to_string(), json!(0.0));
        }
    }

    // Write scorecard
    let scorecard = json!({
        "overall_score": overall_score,
        "total_queries": total,
        "passed_queries": passed_count,
        "failed_queries": total - passed_count,
        "category_scores": category_scores,
    });
    let scorecard_path = "src-tauri/evaluation_scorecard.json";
    let mut scorecard_file = File::create(scorecard_path)?;
    serde_json::to_writer_pretty(&mut scorecard_file, &scorecard)?;
    println!("\nSaved scorecard to: {}", scorecard_path);

    // Generate markdown report
    let mut report_md = String::new();
    report_md.push_str("# RAG Retrieval Evaluation Report\n\n");
    report_md.push_str(&format!("**Overall Score**: {:.1}%\n", overall_score));
    report_md.push_str(&format!("**Total Queries**: {}\n", total));
    report_md.push_str(&format!("**Passed**: {}\n", passed_count));
    report_md.push_str(&format!("**Failed**: {}\n\n", total - passed_count));

    report_md.push_str("## Category Performance\n\n");
    report_md.push_str("| Category | Description | Score |\n");
    report_md.push_str("|---|---|---|\n");
    for cat in &categories {
        let score = category_scores.get(*cat).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let desc = match *cat {
            "A" => "Direct Keyword Matching",
            "B" => "Dense Vector Semantic Search",
            "C" => "Hybrid Search",
            "D" => "Metadata-Faceted Filtering",
            "E" => "Contextual Temporal Search",
            "F" => "Recursive Document Multi-Hop Retrieval",
            "G" => "Confidence Gating Canary",
            "H" => "Ambiguity Routing",
            "I" => "Citation & Source Integrity",
            "J" => "Multi-Hop & Comparison Retrieval Precision",
            _ => "Unknown",
        };
        report_md.push_str(&format!("| Category {} | {} | {:.1}% |\n", cat, desc, score));
    }
    report_md.push_str("\n## Query Execution Details\n\n");
    report_md.push_str("| ID | Category | Query | Status | Citations | Passed | Latency | Doc Recall | Entity Recall | Topic Recall | Reasons / Mismatch |\n");
    report_md.push_str("|---|---|---|---|---|---|---|---|---|---|---|\n");
    for r in &results {
        let reasons_str = if r.reasons.is_empty() {
            "None".to_string()
        } else {
            r.reasons.join("; ")
        };
        report_md.push_str(&format!(
            "| {} | {} | \"{}\" | {} | {} | {} | {}ms | {:.1}% | {:.1}% | {:.1}% | {} |\n",
            r.id, r.category, r.query, r.status, r.citations_count,
            if r.passed { "✅" } else { "❌" },
            r.latency_ms,
            r.document_recall * 100.0,
            r.entity_recall * 100.0,
            r.topic_recall * 100.0,
            reasons_str
        ));
    }

    let report_path = "src-tauri/evaluation_report.md";
    let mut report_file = File::create(report_path)?;
    report_file.write_all(report_md.as_bytes())?;
    println!("Saved evaluation report to: {}", report_path);

    // Also write to the artifacts directory so the user gets it
    let artifact_dir = "/Users/saumyathacker/.gemini/antigravity/brain/ab9a6414-1b35-4165-99f9-f18c04a4acb7";
    let artifact_report_path = Path::new(artifact_dir).join("evaluation_report.md");
    let mut artifact_file = File::create(artifact_report_path)?;
    artifact_file.write_all(report_md.as_bytes())?;
    println!("Saved evaluation report to artifacts directory.");

    println!("\nEvaluation run completed.");
    Ok(())
}
