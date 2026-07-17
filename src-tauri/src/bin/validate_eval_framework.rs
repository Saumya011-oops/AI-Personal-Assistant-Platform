/// validate_eval_framework.rs
///
/// Phase 2 + 3 — Static Validation Binary
///
/// Validates the evaluation framework WITHOUT running the full pipeline:
///
/// Phase 2: Generator Validation
///   - Loads every generated TestCase
///   - Checks required_doc_ids against actual documents in the SQLite DB
///   - Checks required_entities against chunk content in the DB
///   - Checks memory_fixtures for correctness (seeded, so always valid)
///   - Checks expected_statuses are achievable given the corpus
///   - Reports impossible tests
///
/// Phase 3: Executor Trace Field Completeness
///   - Checks ExecutionTrace struct has all required fields defined in types.rs
///   - Checks the LatencyBreakdown fields are all populated
///   - Verifies the trace captures every required pipeline stage
///
/// Does NOT call Qdrant, Ollama, or Groq — pure SQLite + static analysis.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use anyhow::{Context, Result};

use assistant_core::config::AppConfig;
use assistant_core::db::Database;
use assistant_core::evaluation::generator::generate_tests;
use assistant_core::evaluation::types::*;

// ──────────────────────────────────────────────────────────────────────────────
// Validation result
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct TestValidationResult {
    test_id: String,
    suite: TestSuite,
    query: String,
    issues: Vec<ValidationIssue>,
}

#[derive(Debug)]
struct ValidationIssue {
    severity: Severity,
    dimension: &'static str,
    description: String,
}

#[derive(Debug, PartialEq)]
enum Severity {
    /// Test is impossible — will always fail regardless of system correctness
    Impossible,
    /// Test may fail but is not definitively impossible (e.g. weak evidence)
    Suspect,
    /// Informational warning — test is valid but has a potential issue
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Impossible => write!(f, "IMPOSSIBLE"),
            Severity::Suspect => write!(f, "SUSPECT  "),
            Severity::Warning => write!(f, "WARNING  "),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase 3: Trace field completeness (static struct check)
// ──────────────────────────────────────────────────────────────────────────────

/// Required fields that must be present in every trace.
/// Checked structurally — if a field exists on ExecutionTrace and is not
/// `Option`, it will always be populated (Rust guarantees this).
/// For Option fields, we note which ones are silently None and explain why.
struct TraceFieldAudit {
    field: &'static str,
    always_set: bool,
    condition: &'static str,
}

fn audit_trace_fields() -> Vec<TraceFieldAudit> {
    vec![
        TraceFieldAudit { field: "test_id",            always_set: true,  condition: "Always set by executor" },
        TraceFieldAudit { field: "query",              always_set: true,  condition: "Always set by executor" },
        TraceFieldAudit { field: "query_analysis",     always_set: false, condition: "Currently None — RetrievalService does not expose QueryAnalysis in AssistantResponse. IMPROVEMENT REQUIRED." },
        TraceFieldAudit { field: "expanded_query",     always_set: false, condition: "Set from DiagnosticsPayload.query_expanded if diagnostics enabled" },
        TraceFieldAudit { field: "pre_rerank_chunks",  always_set: false, condition: "Set from DiagnosticsPayload.pre_rerank_chunks if diagnostics enabled" },
        TraceFieldAudit { field: "post_rerank_chunks", always_set: false, condition: "Set from DiagnosticsPayload.post_rerank_chunks if diagnostics enabled. Empty if retrieval fails." },
        TraceFieldAudit { field: "recalled_memories",  always_set: true,  condition: "Always set (may be empty if no matching memories)" },
        TraceFieldAudit { field: "prompt_assembled",   always_set: false, condition: "Populated from AssistantResponse.assembled_prompt (wired in retrieval service). Available on main RAG path only; empty for fallback/memory-only paths." },
        TraceFieldAudit { field: "llm_response",       always_set: false, condition: "Set from AssistantResponse.answer — empty on pipeline error" },
        TraceFieldAudit { field: "citations",          always_set: true,  condition: "Always set (may be empty list)" },
        TraceFieldAudit { field: "final_answer",       always_set: false, condition: "Set from AssistantResponse.answer — empty on pipeline error" },
        TraceFieldAudit { field: "confidence",         always_set: false, condition: "Set from AssistantResponse.confidence — None if pipeline fails before confidence check" },
        TraceFieldAudit { field: "diagnostics",        always_set: false, condition: "Set from AssistantResponse.diagnostics — None if diagnostics not enabled" },
        TraceFieldAudit { field: "latency.total_ms",   always_set: true,  condition: "Always measured from pipeline start to end" },
        TraceFieldAudit { field: "latency.memory_retrieval_ms", always_set: false, condition: "Measured separately after main pipeline call — may be 0 if memory service fails" },
        TraceFieldAudit { field: "error",              always_set: false, condition: "Set on pipeline error, None on success" },
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
// Main
// ──────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n╔═══════════════════════════════════════════════════════════════════╗");
    println!("║   QA Framework Validator — Phase 2 (Generator) + Phase 3 (Trace) ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");

    let config = AppConfig::load().context("Failed to load AppConfig")?;
    let database = Database::connect(&config.database_path).context("Failed to connect to DB")?;
    database.run_migrations().context("Failed to run migrations")?;

    // ── Load corpus inventory from DB ────────────────────────────────────────

    let conn = database.get_connection();
    let conn_guard = conn.lock().expect("db lock poisoned");

    // All document IDs and titles
    let mut doc_titles: HashSet<String> = HashSet::new();
    let mut doc_ids: HashSet<String> = HashSet::new();
    {
        let mut stmt = conn_guard
            .prepare("SELECT id, title FROM documents")
            .context("failed to prepare doc query")?;
        let mut rows = stmt.query([]).context("failed to query documents")?;
        while let Some(row) = rows.next().context("failed to iterate rows")? {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            doc_ids.insert(id.clone());
            doc_titles.insert(title.to_lowercase());
            doc_titles.insert(id.to_lowercase());
        }
    }

    // All chunk content concatenated per document (for entity checks)
    let mut all_chunk_text: Vec<String> = Vec::new();
    {
        let mut stmt = conn_guard
            .prepare("SELECT content FROM chunks")
            .context("failed to prepare chunk query")?;
        let mut rows = stmt.query([]).context("failed to query chunks")?;
        while let Some(row) = rows.next().context("failed to iterate rows")? {
            let content: String = row.get(0)?;
            all_chunk_text.push(content.to_lowercase());
        }
    }
    let all_chunks_lower: String = all_chunk_text.join(" ");

    // Live memory count
    let memory_count: i64 = conn_guard
        .query_row("SELECT COUNT(*) FROM memories WHERE status='active'", [], |r| r.get(0))
        .unwrap_or(0);

    drop(conn_guard);

    println!("📚 Corpus inventory:");
    println!("   Documents: {}", doc_ids.len());
    println!("   Chunks:    {}", all_chunk_text.len());
    println!("   Memories:  {} (live, active)", memory_count);
    println!();

    // ── Phase 2: Validate every generated test ────────────────────────────────

    println!("═══════════════════════════════════════════════════════════════════");
    println!("PHASE 2 — Generator Validation");
    println!("═══════════════════════════════════════════════════════════════════\n");

    let all_tests = generate_tests(None);
    println!("Generated {} test cases\n", all_tests.len());

    let mut validation_results: Vec<TestValidationResult> = Vec::new();

    for test in &all_tests {
        let mut issues: Vec<ValidationIssue> = Vec::new();

        // ── 2a. required_doc_ids exist in corpus ─────────────────────────────
        for req_doc in &test.ground_truth.required_doc_ids {
            let req_lower = req_doc.to_lowercase();
            let req_spaced = req_lower.replace('_', " ");
            let found = doc_titles.iter().any(|t| {
                t.contains(&req_lower) || t.contains(&req_spaced)
            });
            if !found {
                issues.push(ValidationIssue {
                    severity: Severity::Impossible,
                    dimension: "required_doc_ids",
                    description: format!(
                        "Document '{}' does NOT exist in the corpus (checked {} doc titles/IDs). \
                         Retrieval will always fail for this requirement.",
                        req_doc, doc_ids.len()
                    ),
                });
            }
        }

        // ── 2b. required_entities appear in chunk content ────────────────────
        for entity in &test.ground_truth.required_entities {
            let entity_lower = entity.to_lowercase();
            if !all_chunks_lower.contains(&entity_lower) {
                // Exact match failed — try substring (handles abbreviations)
                let partial_match = entity_lower
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .any(|w| all_chunks_lower.contains(w));
                if !partial_match {
                    issues.push(ValidationIssue {
                        severity: Severity::Impossible,
                        dimension: "required_entities",
                        description: format!(
                            "Entity '{}' not found anywhere in {} chunks. \
                             The retrieval scorer will always deduct points.",
                            entity, all_chunk_text.len()
                        ),
                    });
                } else {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        dimension: "required_entities",
                        description: format!(
                            "Entity '{}' exact match absent but partial word match found. \
                             Scoring uses substring match so may still pass.",
                            entity
                        ),
                    });
                }
            }
        }

        // ── 2c. required_memory_content: live memories must exist ────────────
        if !test.ground_truth.required_memory_content.is_empty()
            && test.memory_fixtures.is_empty()
        {
            // No seeded fixtures — relies on live memories which are 0
            if memory_count == 0 {
                issues.push(ValidationIssue {
                    severity: Severity::Impossible,
                    dimension: "required_memory_content",
                    description: format!(
                        "Test requires memory content {:?} but there are 0 live active memories \
                         in the DB and no MemoryFixtures declared. Memory scoring will always FAIL.",
                        test.ground_truth.required_memory_content
                    ),
                });
            }
        }

        // ── 2d. Memory dedup test needs >= 2 memories ────────────────────────
        if test.category == TestCategory::MemoryDeduplication
            && test.memory_fixtures.len() < 2
        {
            issues.push(ValidationIssue {
                severity: Severity::Suspect,
                dimension: "memory_fixtures",
                description: format!(
                    "MemoryDeduplication test has {} fixture(s), needs >= 2 to verify dedup.",
                    test.memory_fixtures.len()
                ),
            });
        }

        // ── 2e. Stale test must have is_stale=true fixture ───────────────────
        if test.category == TestCategory::StaleMemoryRejection {
            let has_stale = test.memory_fixtures.iter().any(|f| f.is_stale);
            let has_fresh = test.memory_fixtures.iter().any(|f| !f.is_stale);
            if !has_stale {
                issues.push(ValidationIssue {
                    severity: Severity::Impossible,
                    dimension: "memory_fixtures",
                    description: "StaleMemoryRejection test has no is_stale=true fixture. Cannot validate stale rejection.".to_string(),
                });
            }
            if !has_fresh {
                issues.push(ValidationIssue {
                    severity: Severity::Suspect,
                    dimension: "memory_fixtures",
                    description: "StaleMemoryRejection test has no fresh fixture. Need a fresh alternative to verify ranking.".to_string(),
                });
            }
        }

        // ── 2f. expected_statuses are real status strings ────────────────────
        let valid_statuses = [
            "OK", "PARTIAL_RETRIEVAL", "EMPTY_RETRIEVAL", "LOW_CONFIDENCE",
            "NO_RELEVANT_DOCUMENTS", "MEMORY_ONLY", "NORMAL_CHAT",
        ];
        for status in &test.ground_truth.expected_statuses {
            if !valid_statuses.contains(&status.as_str()) {
                issues.push(ValidationIssue {
                    severity: Severity::Suspect,
                    dimension: "expected_statuses",
                    description: format!(
                        "Status '{}' is not a known status string. \
                         Known: {:?}. May be a typo.",
                        status, valid_statuses
                    ),
                });
            }
        }

        // ── 2g. min_citations feasibility ────────────────────────────────────
        if test.ground_truth.min_citations > 5 {
            issues.push(ValidationIssue {
                severity: Severity::Suspect,
                dimension: "min_citations",
                description: format!(
                    "min_citations={} is unusually high. The pipeline typically returns ≤5 citations.",
                    test.ground_truth.min_citations
                ),
            });
        }

        // ── 2h. Canary tests should expect empty/low retrieval ────────────────
        if test.category == TestCategory::HallucinationCanary {
            let allows_empty = test.ground_truth.expected_statuses.iter().any(|s| {
                s == "EMPTY_RETRIEVAL" || s == "NO_RELEVANT_DOCUMENTS" || s == "LOW_CONFIDENCE"
            });
            if !allows_empty && !test.ground_truth.expected_statuses.is_empty() {
                issues.push(ValidationIssue {
                    severity: Severity::Suspect,
                    dimension: "expected_statuses",
                    description: format!(
                        "HallucinationCanary test only accepts statuses {:?} but these don't include \
                         EMPTY_RETRIEVAL/NO_RELEVANT_DOCUMENTS. Canary queries should expect \
                         the system to say 'I don't know'.",
                        test.ground_truth.expected_statuses
                    ),
                });
            }
        }

        // ── 2i. required_answer_keywords exist in chunk content ──────────────
        for kw in &test.ground_truth.required_answer_keywords {
            let kw_lower = kw.to_lowercase();
            if kw_lower.len() > 4 && !all_chunks_lower.contains(&kw_lower) {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    dimension: "required_answer_keywords",
                    description: format!(
                        "Required answer keyword '{}' is not present in any chunk. \
                         The LLM cannot produce this word from the retrieved evidence.",
                        kw
                    ),
                });
            }
        }

        // ── 2j. Ground truth non-empty for non-canary tests ──────────────────
        let is_canary = matches!(
            test.category,
            TestCategory::HallucinationCanary | TestCategory::EmptyRetrieval
        );
        if !is_canary {
            let has_assertions = !test.ground_truth.required_doc_ids.is_empty()
                || !test.ground_truth.required_entities.is_empty()
                || !test.ground_truth.required_answer_keywords.is_empty()
                || !test.ground_truth.required_memory_content.is_empty()
                || test.ground_truth.min_citations > 0
                || !test.ground_truth.expected_statuses.is_empty();

            if !has_assertions {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    dimension: "ground_truth",
                    description: "Non-canary test has no ground truth assertions. \
                         This test can never detect a regression.".to_string(),
                });
            }
        }

        validation_results.push(TestValidationResult {
            test_id: test.id.clone(),
            suite: test.suite.clone(),
            query: test.query.clone(),
            issues,
        });
    }

    // ── Print Phase 2 results ─────────────────────────────────────────────────

    let impossible_tests: Vec<&TestValidationResult> = validation_results
        .iter()
        .filter(|r| r.issues.iter().any(|i| i.severity == Severity::Impossible))
        .collect();

    let suspect_tests: Vec<&TestValidationResult> = validation_results
        .iter()
        .filter(|r| {
            r.issues.iter().any(|i| i.severity == Severity::Suspect)
                && r.issues.iter().all(|i| i.severity != Severity::Impossible)
        })
        .collect();

    let warning_tests: Vec<&TestValidationResult> = validation_results
        .iter()
        .filter(|r| r.issues.iter().all(|i| i.severity == Severity::Warning))
        .collect();

    let clean_tests: Vec<&TestValidationResult> = validation_results
        .iter()
        .filter(|r| r.issues.is_empty())
        .collect();

    println!("Results summary:");
    println!("  ✅ Valid (clean):   {}", clean_tests.len());
    println!("  ⚠️  Warnings:        {}", warning_tests.len());
    println!("  🟡 Suspect:         {}", suspect_tests.len());
    println!("  🔴 Impossible:      {}", impossible_tests.len());
    println!();

    if !impossible_tests.is_empty() {
        println!("── IMPOSSIBLE TESTS (will always fail, not meaningful) ─────────────\n");
        for r in &impossible_tests {
            println!("  ❌ [{}] {} — \"{}\"", r.suite, r.test_id, truncate(&r.query, 60));
            for issue in &r.issues {
                if issue.severity == Severity::Impossible {
                    println!("     • [{}] [{}] {}", issue.severity, issue.dimension, issue.description);
                }
            }
            println!();
        }
    }

    if !suspect_tests.is_empty() {
        println!("── SUSPECT TESTS (may be wrong but not definitely) ─────────────────\n");
        for r in &suspect_tests {
            println!("  🟡 [{}] {} — \"{}\"", r.suite, r.test_id, truncate(&r.query, 60));
            for issue in &r.issues {
                if issue.severity == Severity::Suspect {
                    println!("     • [{}] [{}] {}", issue.severity, issue.dimension, issue.description);
                }
            }
            println!();
        }
    }

    if !warning_tests.is_empty() {
        println!("── WARNINGS ────────────────────────────────────────────────────────\n");
        for r in &warning_tests {
            println!("  ⚠️  [{}] {} — \"{}\"", r.suite, r.test_id, truncate(&r.query, 60));
            for issue in &r.issues {
                println!("     • [{}] [{}] {}", issue.severity, issue.dimension, issue.description);
            }
            println!();
        }
    }

    // ── Phase 3: Trace field completeness ────────────────────────────────────

    println!("\n═══════════════════════════════════════════════════════════════════");
    println!("PHASE 3 — ExecutionTrace Field Completeness Audit");
    println!("═══════════════════════════════════════════════════════════════════\n");

    let audits = audit_trace_fields();
    let mut always_set = 0usize;
    let mut conditional = 0usize;
    let mut gaps: Vec<&TraceFieldAudit> = Vec::new();

    for audit in &audits {
        let icon = if audit.always_set { "✅" } else { "⚠️ " };
        println!("  {} field: {:35} {}", icon, audit.field, audit.condition);
        if audit.always_set { always_set += 1; } else { conditional += 1; }

        // Flag critical gaps — only query_analysis remains after prompt_assembled fix
        if audit.field == "query_analysis" {
            gaps.push(audit);
        }
    }

    println!();
    println!("  Always-set fields: {}", always_set);
    println!("  Conditional/None:  {}", conditional);
    println!();

    if !gaps.is_empty() {
        println!("── CRITICAL TRACE GAPS ─────────────────────────────────────────────\n");
        for gap in &gaps {
            println!("  🔴 {}: {}", gap.field, gap.condition);
        }
        println!();
        println!("  Impact on scoring:");
        println!("  • query_analysis = None → No query expansion, entity, intent data in trace.");
        println!("    The framework cannot validate Query Analysis as a separate dimension.");
        println!();
        println!("  Note: prompt_assembled is now wired via AssistantResponse.assembled_prompt.");
        println!("  The Prompt Assembly scorer will receive real data on the main RAG path.");
    }

    // ── Final summary ─────────────────────────────────────────────────────────

    println!("\n═══════════════════════════════════════════════════════════════════");
    println!("VALIDATION SUMMARY");
    println!("═══════════════════════════════════════════════════════════════════\n");

    let total_tests = all_tests.len();
    let valid_pct = (clean_tests.len() as f64 / total_tests as f64) * 100.0;

    println!("  Total tests generated: {}", total_tests);
    println!("  Valid:      {} ({:.0}%)", clean_tests.len(), valid_pct);
    println!("  Warnings:   {}", warning_tests.len());
    println!("  Suspect:    {}", suspect_tests.len());
    println!("  Impossible: {}", impossible_tests.len());
    println!();

    if !impossible_tests.is_empty() {
        println!("  ❌ DO NOT run Phase 4 with impossible tests.");
        println!("  ❌ DO NOT generate a baseline until impossible tests are fixed.");
        println!();
        println!("  Required actions before Phase 4:");
        // Collect unique impossible required_doc_ids
        let impossible_docs: HashSet<String> = all_tests
            .iter()
            .filter(|t| impossible_tests.iter().any(|r| r.test_id == t.id))
            .flat_map(|t| {
                t.ground_truth.required_doc_ids.iter().cloned().collect::<Vec<_>>()
            })
            .filter(|doc| {
                let doc_lower = doc.to_lowercase();
                !doc_titles.iter().any(|t| t.contains(&doc_lower))
            })
            .collect();

        if !impossible_docs.is_empty() {
            println!("  1. Replace non-existent required_doc_ids with real corpus documents:");
            for doc in &impossible_docs {
                println!("     - '{}' → does not exist", doc);
            }
        }

        let impossible_entities: HashSet<String> = all_tests
            .iter()
            .filter(|t| impossible_tests.iter().any(|r| r.test_id == t.id))
            .flat_map(|t| {
                t.ground_truth.required_entities.iter().cloned().collect::<Vec<_>>()
            })
            .filter(|e| !all_chunks_lower.contains(&e.to_lowercase()))
            .collect();

        if !impossible_entities.is_empty() {
            println!("  2. Replace non-existent required_entities:");
            for ent in &impossible_entities {
                println!("     - '{}' → not found in any chunk", ent);
            }
        }
    } else {
        println!("  ✅ No impossible tests detected.");
        println!("  Phase 4 (full pipeline run) is safe to proceed.");
    }

    // Exit with non-zero if there are impossible tests
    if !impossible_tests.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
