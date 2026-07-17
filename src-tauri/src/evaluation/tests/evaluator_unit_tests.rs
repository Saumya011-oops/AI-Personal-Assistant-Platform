/// evaluation/tests/evaluator_unit_tests.rs
///
/// Phase 1 — Evaluator Validation
///
/// Deterministic unit tests for every scoring dimension.
/// No I/O, no LLM calls, no database access.
///
/// Every dimension has:
///   - At least one PASS case
///   - At least one FAIL case
///   - Explicit assertion messages explaining what the bug would be

#[cfg(test)]
pub mod evaluator_unit_tests {
    use crate::domain::{Citation, ConfidenceReport, RetrievedChunk};
    use crate::evaluation::evaluator::{cosine_similarity, tokenize_sentences, Evaluator};
    use crate::evaluation::regression::RegressionRunner;
    use crate::evaluation::types::*;
    use serde_json::Value;

    // ─────────────────────────────────────────────────────────────────────────
    // Test helpers
    // ─────────────────────────────────────────────────────────────────────────

    fn make_evaluator() -> Evaluator {
        Evaluator::new_for_test()
    }

    fn make_gt() -> GroundTruth {
        GroundTruth::default()
    }

    fn make_test(gt: GroundTruth) -> TestCase {
        TestCase {
            id: "unit-test".to_string(),
            suite: TestSuite::Retrieval,
            category: TestCategory::FactualLookup,
            description: "unit test".to_string(),
            query: "test query".to_string(),
            ground_truth: gt,
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        }
    }

    fn make_trace() -> ExecutionTrace {
        ExecutionTrace {
            test_id: "unit-test".to_string(),
            query: "test query".to_string(),
            query_analysis: None,
            expanded_query: None,
            pre_rerank_chunks: vec![],
            post_rerank_chunks: vec![],
            recalled_memories: vec![],
            prompt_assembled: String::new(),
            llm_response: String::new(),
            citations: vec![],
            final_answer: String::new(),
            confidence: None,
            diagnostics: None,
            latency: LatencyBreakdown::default(),
            error: None,
        }
    }

    fn make_chunk(chunk_id: &str, doc_title: &str, content: &str) -> RetrievedChunk {
        RetrievedChunk {
            chunk_id: chunk_id.to_string(),
            document_id: "doc-001".to_string(),
            source: "test".to_string(),
            document_title: doc_title.to_string(),
            content: content.to_string(),
            score: 0.85,
            retrieval_score: Some(0.85),
            dense_score: Some(0.85),
            sparse_score: None,
            fused_score: None,
            reranker_score: Some(0.9),
            final_score: Some(0.9),
            ordinal: 0,
            path_or_url: None,
            tags: vec![],
            author: None,
            category: None,
            created_at: None,
            modified_at: None,
            metadata: Value::Null,
        }
    }

    fn make_citation(chunk_id: &str, doc_title: &str) -> Citation {
        Citation {
            chunk_id: chunk_id.to_string(),
            source_document: doc_title.to_string(),
            source_type: "document".to_string(),
            retrieval_score: Some(0.85),
            rerank_score: 0.9,
            section: None,
            evidence: None,
            evidence_level: None,
            document_title: doc_title.to_string(),
            evidence_snippet: None,
            source_connector: "test".to_string(),
            source: doc_title.to_string(),
            document_id: "doc-001".to_string(),
            score: 0.9,
        }
    }

    fn make_confidence(status: &str) -> ConfidenceReport {
        ConfidenceReport {
            confidence: "high".to_string(),
            confidence_score: 85,
            reasons: vec![],
            status: status.to_string(),
            ambiguity_score: None,
        }
    }

    fn make_claim(support: ClaimSupport) -> ClaimVerification {
        ClaimVerification {
            claim: "The system uses vector search to retrieve relevant documents from the corpus.".to_string(),
            support,
            supporting_evidence: Some("Vector search is used.".to_string()),
            evidence_source: Some("chunk-001".to_string()),
            determined_by: "exact_match".to_string(),
            similarity_score: Some(0.85),
        }
    }

    fn make_memory_snap(id: &str, content: &str, score: f64) -> RankedMemorySnapshot {
        use crate::services::memory::DbMemory;
        RankedMemorySnapshot {
            memory: DbMemory {
                id: id.to_string(),
                r#type: "PREFERENCE".to_string(),
                content: content.to_string(),
                embedding_model: "nomic".to_string(),
                importance: 7,
                confidence: 0.9,
                access_count: 1,
                last_used: "2026-07-15 12:00:00".to_string(),
                created_at: "2026-07-15 12:00:00".to_string(),
                updated_at: "2026-07-15 12:00:00".to_string(),
                source_conversation: None,
                status: "active".to_string(),
                deleted_at: None,
            },
            final_score: score,
            similarity: 0.8,
            importance_score: 0.7,
            recency_score: 0.9,
            access_freq_score: 0.1,
        }
    }

    fn dim_pass(s: f32, t: f32) -> DimensionScore {
        DimensionScore::new(s, t, vec![])
    }

    fn make_full_result(id: &str, overall: f32, hal: f32, passed: bool) -> EvalResult {
        EvalResult {
            test_id: id.to_string(),
            suite: TestSuite::Retrieval,
            category: TestCategory::FactualLookup,
            query: "test".to_string(),
            passed,
            scorecard: EvalScorecard {
                retrieval: dim_pass(overall, 95.0),
                memory: dim_pass(overall, 95.0),
                prompt_assembly: dim_pass(overall, 95.0),
                answer_quality: dim_pass(overall, 95.0),
                hallucination: dim_pass(hal, 100.0),
                citation_accuracy: dim_pass(100.0, 100.0),
                grounding: dim_pass(overall, 100.0),
            },
            claim_verifications: vec![],
            root_causes: vec![],
            fix_proposals: vec![],
            trace: make_trace(),
            is_regression: false,
            is_improvement: false,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DIM 1 — Retrieval
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn ret_pass_correct_doc_entity_status_citation() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            required_doc_ids: vec!["authentication".to_string()],
            required_entities: vec!["OAuth".to_string()],
            min_citations: 1,
            expected_statuses: vec!["OK".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.confidence = Some(make_confidence("OK"));
        trace.post_rerank_chunks = vec![make_chunk(
            "c-001",
            "authentication_flow_oauth2",
            "OAuth is an authorization framework used for token-based authentication.",
        )];
        trace.citations = vec![make_citation("c-001", "authentication_flow_oauth2")];

        let score = ev.score_retrieval(&test, &trace);
        assert!(
            score.passed,
            "[FAIL] retrieval must PASS with correct doc/entity/status/citation. Details: {:?}",
            score.details
        );
        assert_eq!(score.score, 100.0);
    }

    #[test]
    fn ret_fail_required_doc_absent() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            required_doc_ids: vec!["notion".to_string()],
            expected_statuses: vec!["OK".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.confidence = Some(make_confidence("OK"));
        trace.post_rerank_chunks = vec![make_chunk("c-001", "qdrant_cluster_scaling", "Qdrant vector db")];

        let score = ev.score_retrieval(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] retrieval must FAIL when required doc 'notion' is absent from retrieved set. Score={}",
            score.score
        );
    }

    #[test]
    fn ret_fail_entity_not_in_chunks() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            required_entities: vec!["Kubernetes".to_string()],
            expected_statuses: vec!["OK".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.confidence = Some(make_confidence("OK"));
        trace.post_rerank_chunks = vec![make_chunk("c-001", "sla_policy_overview", "SLA report for Q1.")];

        let score = ev.score_retrieval(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] retrieval must FAIL when entity 'Kubernetes' is absent from chunk content. Score={}",
            score.score
        );
    }

    #[test]
    fn ret_fail_status_mismatch() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            expected_statuses: vec!["OK".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.confidence = Some(make_confidence("EMPTY_RETRIEVAL"));

        let score = ev.score_retrieval(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] retrieval must FAIL when status='EMPTY_RETRIEVAL' but expected 'OK'. Score={}",
            score.score
        );
    }

    #[test]
    fn ret_fail_citation_count_below_minimum() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            min_citations: 3,
            expected_statuses: vec!["OK".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.confidence = Some(make_confidence("OK"));
        trace.citations = vec![make_citation("c-001", "doc-a"), make_citation("c-002", "doc-b")];

        let score = ev.score_retrieval(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] retrieval must FAIL when citation count (2) is below minimum (3). Score={}",
            score.score
        );
    }

    #[test]
    fn ret_fail_duplicate_chunk_ids() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            expected_statuses: vec!["OK".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.confidence = Some(make_confidence("OK"));
        trace.post_rerank_chunks = vec![
            make_chunk("c-001", "doc-a", "content a"),
            make_chunk("c-001", "doc-a", "content a"), // same ID twice
        ];

        let score = ev.score_retrieval(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] retrieval must FAIL when duplicate chunk_ids are present. Score={}",
            score.score
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DIM 2 — Memory
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn mem_pass_no_assertions_required() {
        let ev = make_evaluator();
        let test = make_test(make_gt());
        let trace = make_trace();
        let score = ev.score_memory(&test, &trace);
        assert!(score.passed, "[FAIL] memory must PASS (100%) when no assertions exist. Score={}", score.score);
        assert_eq!(score.score, 100.0);
    }

    #[test]
    fn mem_pass_required_content_recalled() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            required_memory_content: vec!["User prefers Rust".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.recalled_memories = vec![make_memory_snap(
            "m-001",
            "User prefers Rust as their primary programming language",
            0.92,
        )];

        let score = ev.score_memory(&test, &trace);
        assert!(score.passed, "[FAIL] memory must PASS when required content is recalled. Details: {:?}", score.details);
    }

    #[test]
    fn mem_fail_required_content_not_recalled() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            required_memory_content: vec!["preferred_name_TestUser_LTR_unique_token".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let trace = make_trace();

        let score = ev.score_memory(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] memory must FAIL when required_memory_content is absent from recalled_memories. Score={}",
            score.score
        );
    }

    #[test]
    fn mem_fail_stale_memory_ranked_first() {
        let ev = make_evaluator();
        let gt = make_gt();
        let mut test = make_test(gt);
        test.category = TestCategory::StaleMemoryRejection;
        test.memory_fixtures = vec![MemoryFixture {
            id: "eval-mem-stale".to_string(),
            memory_type: "PREFERENCE".to_string(),
            content: "User prefers MySQL".to_string(),
            importance: 7,
            simulated_age_days: 180.0,
            is_stale: true,
        }];

        let mut trace = make_trace();
        // The stale memory is ranked first — this is wrong
        let mut stale = make_memory_snap("eval-mem-stale", "User prefers MySQL", 0.9);
        stale.recency_score = 0.02; // correctly low but ranking hasn't respected it
        trace.recalled_memories = vec![stale];

        let score = ev.score_memory(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] memory must FAIL when the stale fixture is ranked #1. Score={}",
            score.score
        );
    }

    #[test]
    fn mem_fail_duplicate_memories_recalled() {
        let ev = make_evaluator();
        let gt = make_gt();
        let mut test = make_test(gt);
        test.memory_fixtures = vec![MemoryFixture {
            id: "m-dup".to_string(),
            memory_type: "PREFERENCE".to_string(),
            content: "User likes Rust".to_string(),
            importance: 5,
            simulated_age_days: 1.0,
            is_stale: false,
        }];

        let mut trace = make_trace();
        let snap = make_memory_snap("m-dup", "User likes Rust", 0.85);
        trace.recalled_memories = vec![snap.clone(), snap]; // duplicate

        let score = ev.score_memory(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] memory must FAIL when the same memory appears twice in recalled list. Score={}",
            score.score
        );
    }

    #[test]
    fn mem_fail_ranking_not_descending() {
        let ev = make_evaluator();
        let gt = make_gt();
        let mut test = make_test(gt);
        test.memory_fixtures = vec![
            MemoryFixture { id: "m-a".to_string(), memory_type: "PREF".to_string(), content: "A".to_string(), importance: 5, simulated_age_days: 1.0, is_stale: false },
            MemoryFixture { id: "m-b".to_string(), memory_type: "PREF".to_string(), content: "B".to_string(), importance: 5, simulated_age_days: 1.0, is_stale: false },
        ];

        let mut trace = make_trace();
        // Scores go UP instead of DOWN — ranking is wrong
        trace.recalled_memories = vec![
            make_memory_snap("m-a", "Memory A", 0.70), // lower score listed first
            make_memory_snap("m-b", "Memory B", 0.90), // higher score listed second ← wrong
        ];

        let score = ev.score_memory(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] memory must FAIL when recalled_memories are not in descending score order. Score={}",
            score.score
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DIM 3 — Prompt Assembly
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn prompt_pass_empty_captured_gives_95() {
        let ev = make_evaluator();
        let trace = make_trace(); // prompt_assembled = ""
        let score = ev.score_prompt_assembly(&trace);
        // 100 - 5 (observability gap) = 95, exactly on threshold → PASS
        assert!(score.passed, "[FAIL] empty prompt should give score=95 (on threshold). Score={}", score.score);
        assert_eq!(score.score, 95.0);
    }

    #[test]
    fn prompt_pass_correct_section_order() {
        let ev = make_evaluator();
        let mut trace = make_trace();
        trace.prompt_assembled = [
            "Conversation Summary: prev discussion.",
            "Long-Term Memories: user prefers Rust.",
            "Recent Episodes: built RAG pipeline.",
            "Recent Conversation Messages: hello.",
            "Retrieved RAG Documents: chunk text here.",
            "Current User Message: explain oauth.",
        ]
        .join("\n\n");

        let score = ev.score_prompt_assembly(&trace);
        assert!(score.passed, "[FAIL] prompt must PASS with sections in correct order. Score={}", score.score);
    }

    #[test]
    fn prompt_fail_sections_out_of_order() {
        let ev = make_evaluator();
        let mut trace = make_trace();
        // RAG Documents before Long-Term Memories — violates spec
        trace.prompt_assembled = [
            "Conversation Summary: prev.",
            "Retrieved RAG Documents: chunk.",  // ← appears too early
            "Long-Term Memories: preference.",
            "Current User Message: test.",
        ]
        .join("\n\n");

        let score = ev.score_prompt_assembly(&trace);
        assert!(
            !score.passed,
            "[BUG] prompt must FAIL when 'Retrieved RAG Documents' appears before 'Long-Term Memories'. Score={}",
            score.score
        );
    }

    #[test]
    fn prompt_fail_duplicate_sections() {
        let ev = make_evaluator();
        let mut trace = make_trace();
        trace.prompt_assembled = [
            "Conversation Summary: first.",
            "Long-Term Memories: mem1.",
            "Long-Term Memories: mem2.", // duplicate
            "Current User Message: test.",
        ]
        .join("\n\n");

        let score = ev.score_prompt_assembly(&trace);
        assert!(
            !score.passed,
            "[BUG] prompt must FAIL when 'Long-Term Memories' section appears twice. Score={}",
            score.score
        );
    }

    #[test]
    fn prompt_fail_overflow() {
        let ev = make_evaluator();
        let mut trace = make_trace();
        trace.prompt_assembled = "x".repeat(120_005); // ~30001 tokens at 4 chars/token
        let score = ev.score_prompt_assembly(&trace);
        assert!(
            !score.passed,
            "[BUG] prompt must FAIL when estimated token count exceeds 30k. Score={}",
            score.score
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DIM 4 — Answer Quality
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn ans_fail_empty_answer() {
        let ev = make_evaluator();
        let test = make_test(make_gt());
        let trace = make_trace();
        let score = ev.score_answer_quality(&test, &trace);
        assert!(!score.passed, "[BUG] answer quality must FAIL for empty answer. Score={}", score.score);
        assert_eq!(score.score, 0.0);
    }

    #[test]
    fn ans_fail_required_keyword_missing() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            required_answer_keywords: vec!["reimbursement".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.final_answer =
            "The policy describes general guidelines and procedures for employees seeking approval.".to_string();

        let score = ev.score_answer_quality(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] answer quality must FAIL when required keyword 'reimbursement' is absent. Score={}",
            score.score
        );
    }

    #[test]
    fn ans_fail_forbidden_term_in_answer() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            forbidden_terms: vec!["quantum computing".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.final_answer =
            "This assistant supports quantum computing operations via its advanced processing pipeline.".to_string();

        let score = ev.score_answer_quality(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] answer quality must FAIL when a forbidden term appears in the answer. Score={}",
            score.score
        );
    }

    #[test]
    fn ans_fail_too_short() {
        let ev = make_evaluator();
        let test = make_test(make_gt());
        let mut trace = make_trace();
        trace.final_answer = "Yes it works.".to_string(); // 3 words < 10

        let score = ev.score_answer_quality(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] answer quality must FAIL for answers shorter than 10 words. Score={}",
            score.score
        );
    }

    #[test]
    fn ans_pass_correct_keywords_no_forbidden() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            required_answer_keywords: vec!["oauth".to_string(), "token".to_string()],
            forbidden_terms: vec!["quantum".to_string()],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.final_answer =
            "Authentication uses the OAuth 2.0 flow. You need to generate an API token from your account settings and provide it to the application. The token is then used in all subsequent requests to authorize access to the system.".to_string();

        let score = ev.score_answer_quality(&test, &trace);
        assert!(
            score.passed,
            "[FAIL] answer quality must PASS with required keywords and no forbidden terms. Details: {:?}",
            score.details
        );
    }

    #[test]
    fn ans_fail_missing_uncertainty_acknowledgement() {
        let ev = make_evaluator();
        let gt = GroundTruth {
            answer_characteristics: vec![AnswerCharacteristic::AcknowledgesUncertainty],
            ..Default::default()
        };
        let test = make_test(gt);
        let mut trace = make_trace();
        trace.final_answer =
            "The Obsidian plugin for NovaTech was built in December 2025 and supports full bidirectional sync with the Qdrant cluster. It processes over 10 million vectors per second in standard configuration.".to_string();

        let score = ev.score_answer_quality(&test, &trace);
        assert!(
            !score.passed,
            "[BUG] answer quality must FAIL when AcknowledgesUncertainty is required but absent. Score={}",
            score.score
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DIM 5 — Hallucination
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn hal_pass_no_claims() {
        let ev = make_evaluator();
        let score = ev.score_hallucination(&[]);
        assert!(score.passed, "[FAIL] hallucination must PASS (100%) with no claims. Score={}", score.score);
        assert_eq!(score.score, 100.0);
    }

    #[test]
    fn hal_pass_all_supported() {
        let ev = make_evaluator();
        let claims = vec![
            make_claim(ClaimSupport::Supported),
            make_claim(ClaimSupport::Supported),
            make_claim(ClaimSupport::PartiallySupported),
        ];
        let score = ev.score_hallucination(&claims);
        assert!(score.passed, "[FAIL] hallucination must PASS when all claims are Supported/PartiallySupported. Score={}", score.score);
    }

    #[test]
    fn hal_fail_single_hallucinated_claim() {
        let ev = make_evaluator();
        let claims = vec![
            make_claim(ClaimSupport::Supported),
            make_claim(ClaimSupport::Hallucinated), // ← triggers failure
            make_claim(ClaimSupport::Supported),
        ];
        let score = ev.score_hallucination(&claims);
        assert!(
            !score.passed,
            "[BUG] hallucination MUST fail (score=0) when any claim is Hallucinated, even among supported claims. Score={}",
            score.score
        );
        assert_eq!(
            score.score, 0.0,
            "[BUG] hallucination score must be exactly 0.0 on any Hallucinated claim, got {}",
            score.score
        );
    }

    #[test]
    fn hal_unsupported_does_not_trigger_failure() {
        // CRITICAL: UNSUPPORTED ≠ HALLUCINATED
        // An unsupported claim may be a valid inference not in the evidence.
        // Only HALLUCINATED (i.e., contradicts evidence) should fail this dimension.
        let ev = make_evaluator();
        let claims = vec![
            make_claim(ClaimSupport::Supported),
            make_claim(ClaimSupport::Unsupported), // absent from evidence but not a contradiction
        ];
        let score = ev.score_hallucination(&claims);
        assert!(
            score.passed,
            "[BUG] UNSUPPORTED must NOT be treated as HALLUCINATED by the hallucination dimension. Score={}",
            score.score
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DIM 6 — Citation Accuracy
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn cit_pass_no_citations() {
        let ev = make_evaluator();
        let trace = make_trace();
        let score = ev.score_citations(&trace);
        assert!(score.passed, "[FAIL] citation must PASS when no citations present. Score={}", score.score);
        assert_eq!(score.score, 100.0);
    }

    #[test]
    fn cit_pass_all_chunks_in_retrieved_set() {
        let ev = make_evaluator();
        let mut trace = make_trace();
        trace.post_rerank_chunks = vec![
            make_chunk("c-001", "authentication_flow_oauth2", "OAuth content"),
            make_chunk("c-002", "qdrant_cluster_scaling", "Qdrant content"),
        ];
        trace.citations = vec![
            make_citation("c-001", "authentication_flow_oauth2"),
            make_citation("c-002", "qdrant_cluster_scaling"),
        ];

        let score = ev.score_citations(&trace);
        assert!(score.passed, "[FAIL] citation must PASS when all cited chunks are retrieved. Score={}", score.score);
        assert_eq!(score.score, 100.0);
    }

    #[test]
    fn cit_fail_fabricated_chunk_id() {
        let ev = make_evaluator();
        let mut trace = make_trace();
        trace.post_rerank_chunks = vec![make_chunk("c-001", "doc-a", "content")];
        trace.citations = vec![
            make_citation("c-001", "doc-a"),    // valid
            make_citation("c-FAKE", "doc-fake"), // NOT in retrieved set
        ];

        let score = ev.score_citations(&trace);
        assert!(
            !score.passed,
            "[BUG] citation must FAIL when a chunk_id is cited but not in the retrieved set. Score={}",
            score.score
        );
        assert_eq!(
            score.score, 0.0,
            "[BUG] any fabricated citation must bring citation score to 0. Got {}",
            score.score
        );
    }

    #[test]
    fn cit_empty_chunk_id_is_allowed() {
        // Memory-based citations may have no chunk_id — they must not fail validation.
        let ev = make_evaluator();
        let mut trace = make_trace();
        trace.citations = vec![Citation {
            chunk_id: String::new(),
            source_document: "memory".to_string(),
            source_type: "memory".to_string(),
            retrieval_score: None,
            rerank_score: 0.0,
            section: None,
            evidence: None,
            evidence_level: None,
            document_title: "memory".to_string(),
            evidence_snippet: None,
            source_connector: "memory".to_string(),
            source: "memory".to_string(),
            document_id: String::new(),
            score: 0.0,
        }];

        let score = ev.score_citations(&trace);
        assert!(
            score.passed,
            "[BUG] memory citation with empty chunk_id must not be treated as fabricated. Score={}",
            score.score
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DIM 7 — Grounding
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn grd_pass_no_claims() {
        let ev = make_evaluator();
        let score = ev.score_grounding(&[]);
        assert!(score.passed, "[FAIL] grounding must PASS (100%) with no claims. Score={}", score.score);
    }

    #[test]
    fn grd_pass_all_supported() {
        let ev = make_evaluator();
        let claims = vec![
            make_claim(ClaimSupport::Supported),
            make_claim(ClaimSupport::PartiallySupported),
        ];
        let score = ev.score_grounding(&claims);
        assert!(score.passed, "[FAIL] grounding must PASS when all claims are Supported/PartiallySupported. Score={}", score.score);
        assert_eq!(score.score, 100.0);
    }

    #[test]
    fn grd_fail_unsupported_claim() {
        let ev = make_evaluator();
        // 1 supported, 1 unsupported → 50% → below 100% threshold
        let claims = vec![
            make_claim(ClaimSupport::Supported),
            make_claim(ClaimSupport::Unsupported),
        ];
        let score = ev.score_grounding(&claims);
        assert!(
            !score.passed,
            "[BUG] grounding must FAIL when any claim is Unsupported (threshold=100%). Score={}",
            score.score
        );
        assert!(
            (score.score - 50.0).abs() < 0.5,
            "[BUG] grounding score should be 50% for 1/2 grounded claims, got {}",
            score.score
        );
    }

    #[test]
    fn grd_fail_hallucinated_counts_as_ungrounded() {
        let ev = make_evaluator();
        let claims = vec![
            make_claim(ClaimSupport::Supported),
            make_claim(ClaimSupport::Hallucinated),
        ];
        let score = ev.score_grounding(&claims);
        assert!(
            !score.passed,
            "[BUG] Hallucinated claim must count as ungrounded (grounding FAIL). Score={}",
            score.score
        );
        assert!(
            (score.score - 50.0).abs() < 0.5,
            "[BUG] grounding score should be 50% (1/2 grounded), got {}",
            score.score
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Cosine similarity (used in Stage 2)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn cosine_identical_returns_one() {
        let v = vec![1.0f32, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-5, "Identical vectors → cosine=1.0, got {}", sim);
    }

    #[test]
    fn cosine_orthogonal_returns_zero() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-5, "Orthogonal vectors → cosine=0.0, got {}", sim);
    }

    #[test]
    fn cosine_zero_vector_returns_zero() {
        let a = vec![0.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0, "Zero vector → cosine=0.0, got {}", sim);
    }

    #[test]
    fn cosine_mismatched_lengths_returns_zero() {
        let a = vec![1.0f32, 2.0];
        let b = vec![1.0f32, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0, "Mismatched lengths → cosine=0.0, got {}", sim);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Sentence tokenizer
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn tokenizer_three_sentences() {
        let text = "The system uses RAG for retrieval. Qdrant is the vector database store. Embeddings are generated by the Ollama service.";
        let sentences = tokenize_sentences(text);
        assert_eq!(
            sentences.len(), 3,
            "Expected 3 sentences, got {}. Sentences: {:?}", sentences.len(), sentences
        );
    }

    #[test]
    fn tokenizer_drops_short_fragments() {
        let text = "Yes. The system uses Qdrant as its primary vector database for semantic document search.";
        let sentences = tokenize_sentences(text);
        // "Yes." is 1 word < 5 → dropped
        assert_eq!(
            sentences.len(), 1,
            "Short fragment 'Yes.' should be filtered out. Got {:?}", sentences
        );
    }

    #[test]
    fn tokenizer_empty_input_empty_output() {
        assert!(tokenize_sentences("").is_empty(), "Empty input must produce empty sentence list");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // EvalScorecard
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn scorecard_production_ready_requires_all_dims_pass() {
        let p = |s: f32, t: f32| DimensionScore::new(s, t, vec![]);

        let all_pass = EvalScorecard {
            retrieval:       p(96.0, 95.0),
            memory:          p(96.0, 95.0),
            prompt_assembly: p(96.0, 95.0),
            answer_quality:  p(96.0, 95.0),
            hallucination:   p(100.0, 100.0),
            citation_accuracy: p(100.0, 100.0),
            grounding:       p(100.0, 100.0),
        };
        assert!(all_pass.production_ready(), "[BUG] all-passing scorecard must be production_ready");

        let one_fail = EvalScorecard {
            retrieval: p(94.0, 95.0), // ← fails (94 < 95)
            ..all_pass.clone()
        };
        assert!(!one_fail.production_ready(), "[BUG] scorecard must NOT be production_ready when retrieval fails");
    }

    #[test]
    fn scorecard_overall_score_is_mean_of_seven() {
        let p = |s: f32| DimensionScore::new(s, 95.0, vec![]);
        let sc = EvalScorecard {
            retrieval: p(80.0),
            memory: p(90.0),
            prompt_assembly: p(100.0),
            answer_quality: p(70.0),
            hallucination: DimensionScore::new(100.0, 100.0, vec![]),
            citation_accuracy: DimensionScore::new(100.0, 100.0, vec![]),
            grounding: DimensionScore::new(60.0, 100.0, vec![]),
        };
        let expected = (80.0 + 90.0 + 100.0 + 70.0 + 100.0 + 100.0 + 60.0) / 7.0;
        let got = sc.overall_score();
        assert!(
            (got - expected).abs() < 0.01,
            "overall_score() should be mean of 7 dimensions. Expected {:.2}, got {:.2}",
            expected, got
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Regression Runner
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn regression_detects_score_drop() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let runner = RegressionRunner::new(dir.path());

        let mut baseline = Baseline::default();
        baseline.entries.insert(
            "t-001".to_string(),
            BaselineEntry {
                test_id: "t-001".to_string(),
                passed: true,
                overall_score: 97.0,
                retrieval_score: 97.0,
                memory_score: 97.0,
                citation_score: 100.0,
                grounding_score: 100.0,
                hallucination_score: 100.0,
                timestamp: "2026-07-15T00:00:00Z".to_string(),
            },
        );

        // Current: 88% — 9% drop, exceeds 2% threshold
        let current = vec![make_full_result("t-001", 88.0, 100.0, false)];
        let (annotated, regressions, _) = runner.compare(current, &baseline);

        assert!(!regressions.is_empty(), "[BUG] 9% score drop must be flagged as regression");
        assert!(annotated[0].is_regression, "[BUG] result must be marked is_regression=true");
    }

    #[test]
    fn regression_detects_improvement() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let runner = RegressionRunner::new(dir.path());

        let mut baseline = Baseline::default();
        baseline.entries.insert(
            "t-001".to_string(),
            BaselineEntry {
                test_id: "t-001".to_string(),
                passed: false,
                overall_score: 78.0,
                retrieval_score: 78.0,
                memory_score: 78.0,
                citation_score: 78.0,
                grounding_score: 78.0,
                hallucination_score: 0.0,
                timestamp: "2026-07-15T00:00:00Z".to_string(),
            },
        );

        // Current: 98% and passing — clear improvement
        let current = vec![make_full_result("t-001", 98.0, 100.0, true)];
        let (annotated, regressions, improvements) = runner.compare(current, &baseline);

        assert!(regressions.is_empty(), "[BUG] an improvement should not be flagged as regression");
        assert!(!improvements.is_empty(), "[BUG] 20% score gain must be flagged as improvement");
        assert!(annotated[0].is_improvement, "[BUG] result must be marked is_improvement=true");
    }

    #[test]
    fn regression_no_flag_within_threshold() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let runner = RegressionRunner::new(dir.path());

        let mut baseline = Baseline::default();
        baseline.entries.insert(
            "t-001".to_string(),
            BaselineEntry {
                test_id: "t-001".to_string(),
                passed: true,
                overall_score: 97.0,
                retrieval_score: 97.0,
                memory_score: 97.0,
                citation_score: 100.0,
                grounding_score: 100.0,
                hallucination_score: 100.0,
                timestamp: "2026-07-15T00:00:00Z".to_string(),
            },
        );

        // Current result: a tiny 0.5% drop on retrieval only.
        // citation, grounding, hallucination all remain at baseline levels.
        // This ensures NO dimension regression (all < 2% drop) and
        // NO overall regression (0.5% drop < 2% threshold).
        let p = |s: f32, t: f32| DimensionScore::new(s, t, vec![]);
        let mut result = make_full_result("t-001", 97.0, 100.0, true);
        result.scorecard.retrieval = p(96.5, 95.0);   // 0.5% drop vs baseline 97.0
        result.scorecard.memory   = p(97.0, 95.0);   // same as baseline
        result.scorecard.citation_accuracy = p(100.0, 100.0); // same as baseline
        result.scorecard.grounding = p(100.0, 100.0); // same as baseline
        result.scorecard.hallucination = p(100.0, 100.0); // same as baseline

        let (_, regressions, improvements) = runner.compare(vec![result], &baseline);

        assert!(regressions.is_empty(),
            "[BUG] a 0.5% retrieval drop is within 2% threshold, must NOT be regression. Flagged: {:?}", regressions);
        assert!(improvements.is_empty(),
            "[BUG] a 0.5% drop must NOT be flagged as improvement either");
    }

    #[test]
    fn regression_new_test_not_in_baseline_is_never_regression() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let runner = RegressionRunner::new(dir.path());
        let baseline = Baseline::default(); // empty

        let current = vec![make_full_result("brand-new", 40.0, 0.0, false)];
        let (_, regressions, _) = runner.compare(current, &baseline);

        assert!(
            regressions.is_empty(),
            "[BUG] a test absent from the baseline cannot be a regression (no prior score to compare against)"
        );
    }
}
