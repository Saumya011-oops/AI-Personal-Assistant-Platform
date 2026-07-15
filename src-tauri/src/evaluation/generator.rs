/// evaluation/generator.rs
///
/// Programmatic test case factory.
///
/// Rather than a static JSON file, this module generates TestCase structs at
/// runtime from templates and the live document corpus. New test categories can
/// be added by extending the `generate_*` functions without touching any other
/// layer.
///
/// Design rules:
///  - Every generated test has a complete GroundTruth (what the answer MUST contain).
///  - Seeded memory tests declare MemoryFixtures that the Executor installs and
///    then cleans up automatically.
///  - Adversarial / edge-case tests explicitly set forbidden_terms to catch
///    hallucinations.
///  - No test is hardcoded in a JSON file.

use super::types::*;
use uuid::Uuid;

// ──────────────────────────────────────────────────────────────────────────────
// Public entry point
// ──────────────────────────────────────────────────────────────────────────────

/// Generates all test cases for the requested suites.
/// Pass `None` to generate tests for every suite.
pub fn generate_tests(suites: Option<&[TestSuite]>) -> Vec<TestCase> {
    let all: Vec<Box<dyn Fn() -> Vec<TestCase>>> = vec![
        Box::new(retrieval_tests),
        Box::new(memory_tests),
        Box::new(combined_tests),
        Box::new(hallucination_tests),
        Box::new(citation_tests),
        Box::new(prompt_assembly_tests),
        Box::new(grounding_tests),
    ];

    let mut cases = Vec::new();
    for generator in &all {
        let batch = generator();
        if let Some(filter) = suites {
            for tc in batch {
                if filter.contains(&tc.suite) {
                    cases.push(tc);
                }
            }
        } else {
            cases.extend(batch);
        }
    }
    cases
}

// ──────────────────────────────────────────────────────────────────────────────
// Helper: unique test id
// ──────────────────────────────────────────────────────────────────────────────

fn tid(prefix: &str) -> String {
    format!("{}-{}", prefix, &Uuid::new_v4().to_string()[..8])
}

// ──────────────────────────────────────────────────────────────────────────────
// 1. RETRIEVAL TESTS
// ──────────────────────────────────────────────────────────────────────────────

fn retrieval_tests() -> Vec<TestCase> {
    vec![
        // ── Normal: Factual Lookup ────────────────────────────────────────────
        TestCase {
            id: tid("RET-FACT"),
            suite: TestSuite::Retrieval,
            category: TestCategory::FactualLookup,
            description: "Direct factual lookup — Notion OAuth setup steps".to_string(),
            query: "How do I connect Notion to the assistant using OAuth?".to_string(),
            ground_truth: GroundTruth {
                required_facts: vec![
                    "OAuth token is required to connect Notion".to_string(),
                    "API credentials must be saved".to_string(),
                ],
                required_doc_ids: vec!["notion".to_string()],
                required_entities: vec!["Notion".to_string(), "OAuth".to_string()],
                required_answer_keywords: vec!["token".to_string(), "notion".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Normal: Semantic Search ───────────────────────────────────────────
        TestCase {
            id: tid("RET-SEM"),
            suite: TestSuite::Retrieval,
            category: TestCategory::SemanticSearch,
            description: "Semantic search — authentication issues (paraphrase of 'login error')".to_string(),
            query: "Users are unable to sign in to the application. What should be checked?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["authentication".to_string()],
                required_answer_keywords: vec!["login".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Normal: Keyword Lookup ────────────────────────────────────────────
        TestCase {
            id: tid("RET-KWD"),
            suite: TestSuite::Retrieval,
            category: TestCategory::KeywordLookup,
            description: "Exact keyword match — 'Qdrant' vector database".to_string(),
            query: "What is Qdrant used for in the assistant?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Qdrant".to_string()],
                required_answer_keywords: vec!["qdrant".to_string(), "vector".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Normal: Metadata Filtering ────────────────────────────────────────
        TestCase {
            id: tid("RET-META"),
            suite: TestSuite::Retrieval,
            category: TestCategory::MetadataFilter,
            description: "Document-type faceted filter — policy documents".to_string(),
            query: "Show me all policy documents related to reimbursement.".to_string(),
            ground_truth: GroundTruth {
                required_answer_keywords: vec!["policy".to_string(), "reimbursement".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints {
                expected_strategy: Some("FACETED".to_string()),
                ..Default::default()
            },
        },

        // ── Normal: Broad Question ────────────────────────────────────────────
        TestCase {
            id: tid("RET-BROAD"),
            suite: TestSuite::Retrieval,
            category: TestCategory::BroadQuestion,
            description: "Broad question spanning multiple document types".to_string(),
            query: "Give me an overview of all the integrations supported by the assistant.".to_string(),
            ground_truth: GroundTruth {
                required_answer_keywords: vec!["notion".to_string(), "google".to_string()],
                min_citations: 2,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Normal: Comparison Question ───────────────────────────────────────
        TestCase {
            id: tid("RET-CMP"),
            suite: TestSuite::Retrieval,
            category: TestCategory::ComparisonQuestion,
            description: "Cross-document comparison — Notion vs Obsidian sync".to_string(),
            query: "What is the difference between syncing Notion documents and syncing Obsidian notes?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Notion".to_string(), "Obsidian".to_string()],
                required_answer_keywords: vec!["notion".to_string(), "obsidian".to_string()],
                min_citations: 2,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                answer_characteristics: vec![AnswerCharacteristic::ContainsComparison],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Normal: Multi-Hop Reasoning ───────────────────────────────────────
        TestCase {
            id: tid("RET-HOP"),
            suite: TestSuite::Retrieval,
            category: TestCategory::MultiHopReasoning,
            description: "Multi-hop — how authentication relates to document access".to_string(),
            query: "How does the authentication system interact with document access permissions?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["authentication".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Normal: Date-Filtered ─────────────────────────────────────────────
        TestCase {
            id: tid("RET-DATE"),
            suite: TestSuite::Retrieval,
            category: TestCategory::DateFilter,
            description: "Temporal filter — recent documents".to_string(),
            query: "What documents were added or updated recently?".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string(), "EMPTY_RETRIEVAL".to_string()],
                answer_characteristics: vec![AnswerCharacteristic::ContainsDate],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints {
                expected_strategy: Some("CONTEXTUAL".to_string()),
                ..Default::default()
            },
        },

        // ── Edge: Typo Query ──────────────────────────────────────────────────
        TestCase {
            id: tid("RET-TYPO"),
            suite: TestSuite::Retrieval,
            category: TestCategory::TypoQuery,
            description: "Typo resilience — 'Obsidean' instead of 'Obsidian'".to_string(),
            query: "How do I sync my Obsidean vault?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Obsidian".to_string()],
                required_answer_keywords: vec!["obsidian".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                forbidden_terms: vec![],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Edge: Synonym Query ───────────────────────────────────────────────
        TestCase {
            id: tid("RET-SYN"),
            suite: TestSuite::Retrieval,
            category: TestCategory::SynonymQuery,
            description: "Synonym — 'vector store' instead of 'Qdrant'".to_string(),
            query: "How does the vector store work in the assistant?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Qdrant".to_string()],
                required_answer_keywords: vec!["vector".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Edge: Acronym ─────────────────────────────────────────────────────
        TestCase {
            id: tid("RET-ACR"),
            suite: TestSuite::Retrieval,
            category: TestCategory::AcronymQuery,
            description: "Acronym expansion — 'RAG' to full term".to_string(),
            query: "How does RAG work in this system?".to_string(),
            ground_truth: GroundTruth {
                required_answer_keywords: vec!["retrieval".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Edge: Ambiguous ───────────────────────────────────────────────────
        TestCase {
            id: tid("RET-AMB"),
            suite: TestSuite::Retrieval,
            category: TestCategory::AmbiguousQuery,
            description: "Ambiguous query — 'it' with no clear referent".to_string(),
            query: "How do I reset it?".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec![
                    "AMBIGUOUS_RETRIEVAL".to_string(),
                    "PARTIAL_RETRIEVAL".to_string(),
                    "OK".to_string(),
                ],
                answer_characteristics: vec![AnswerCharacteristic::AcknowledgesUncertainty],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Edge: Empty Retrieval Canary ──────────────────────────────────────
        TestCase {
            id: tid("RET-EMPTY"),
            suite: TestSuite::Retrieval,
            category: TestCategory::EmptyRetrieval,
            description: "Empty retrieval — topic with no documents (quantum computing)".to_string(),
            query: "What is the assistant's quantum computing integration roadmap?".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec![
                    "EMPTY_RETRIEVAL".to_string(),
                    "LOW_CONFIDENCE_RETRIEVAL".to_string(),
                ],
                forbidden_terms: vec![
                    "quantum".to_string(),
                    "qubit".to_string(),
                    "superposition".to_string(),
                ],
                answer_characteristics: vec![AnswerCharacteristic::AcknowledgesUncertainty],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Edge: Incomplete Query ────────────────────────────────────────────
        TestCase {
            id: tid("RET-INC"),
            suite: TestSuite::Retrieval,
            category: TestCategory::IncompleteQuery,
            description: "Incomplete query — trailing fragment".to_string(),
            query: "What happens when the sync".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec![
                    "OK".to_string(),
                    "AMBIGUOUS_RETRIEVAL".to_string(),
                    "PARTIAL_RETRIEVAL".to_string(),
                    "LOW_CONFIDENCE_RETRIEVAL".to_string(),
                ],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
// 2. MEMORY TESTS
// ──────────────────────────────────────────────────────────────────────────────

fn memory_tests() -> Vec<TestCase> {
    vec![
        // ── Seeded: Long-Term Recall ──────────────────────────────────────────
        TestCase {
            id: tid("MEM-LTR"),
            suite: TestSuite::Memory,
            category: TestCategory::LongTermRecall,
            description: "Seeded PROFILE memory — recall user name preference".to_string(),
            query: "What is my preferred name?".to_string(),
            ground_truth: GroundTruth {
                required_memory_content: vec!["preferred name is TestUser_LTR".to_string()],
                required_answer_keywords: vec!["testuser_ltr".to_string()],
                min_citations: 0,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![MemoryFixture {
                id: "eval-mem-ltr-001".to_string(),
                memory_type: "PROFILE".to_string(),
                content: "User's preferred name is TestUser_LTR".to_string(),
                importance: 8,
                simulated_age_days: 5.0,
                is_stale: false,
            }],
            constraints: TestConstraints {
                uses_seeded_memories: true,
                ..Default::default()
            },
        },

        // ── Seeded: Preference Recall ─────────────────────────────────────────
        TestCase {
            id: tid("MEM-PREF"),
            suite: TestSuite::Memory,
            category: TestCategory::PreferenceRecall,
            description: "Seeded PREFERENCE memory — recall coding language preference".to_string(),
            query: "What programming language do I prefer?".to_string(),
            ground_truth: GroundTruth {
                required_memory_content: vec!["prefers Rust".to_string()],
                required_answer_keywords: vec!["rust".to_string()],
                min_citations: 0,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![MemoryFixture {
                id: "eval-mem-pref-001".to_string(),
                memory_type: "PREFERENCE".to_string(),
                content: "User prefers Rust as their primary programming language".to_string(),
                importance: 7,
                simulated_age_days: 3.0,
                is_stale: false,
            }],
            constraints: TestConstraints {
                uses_seeded_memories: true,
                ..Default::default()
            },
        },

        // ── Seeded: Episodic Recall ───────────────────────────────────────────
        TestCase {
            id: tid("MEM-EP"),
            suite: TestSuite::Memory,
            category: TestCategory::EpisodicRecall,
            description: "Seeded EPISODE memory — recall a past conversation topic".to_string(),
            query: "Did we discuss Obsidian vault setup previously?".to_string(),
            ground_truth: GroundTruth {
                required_memory_content: vec!["discussed Obsidian vault configuration".to_string()],
                required_answer_keywords: vec!["obsidian".to_string(), "vault".to_string()],
                min_citations: 0,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![MemoryFixture {
                id: "eval-mem-ep-001".to_string(),
                memory_type: "EPISODE".to_string(),
                content: "User and assistant discussed Obsidian vault configuration and sync settings".to_string(),
                importance: 6,
                simulated_age_days: 10.0,
                is_stale: false,
            }],
            constraints: TestConstraints {
                uses_seeded_memories: true,
                ..Default::default()
            },
        },

        // ── Seeded: Goal Recall ───────────────────────────────────────────────
        TestCase {
            id: tid("MEM-GOAL"),
            suite: TestSuite::Memory,
            category: TestCategory::GoalRecall,
            description: "Seeded GOAL memory — recall user's stated objective".to_string(),
            query: "What was my goal for this quarter?".to_string(),
            ground_truth: GroundTruth {
                required_memory_content: vec!["ship the RAG evaluation framework".to_string()],
                required_answer_keywords: vec!["evaluation".to_string()],
                min_citations: 0,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![MemoryFixture {
                id: "eval-mem-goal-001".to_string(),
                memory_type: "GOAL".to_string(),
                content: "User's goal for this quarter is to ship the RAG evaluation framework to production".to_string(),
                importance: 9,
                simulated_age_days: 1.0,
                is_stale: false,
            }],
            constraints: TestConstraints {
                uses_seeded_memories: true,
                ..Default::default()
            },
        },

        // ── Seeded: Stale Memory Rejection ───────────────────────────────────
        TestCase {
            id: tid("MEM-STALE"),
            suite: TestSuite::Memory,
            category: TestCategory::StaleMemoryRejection,
            description: "Stale memory — high-age memory should rank below fresh one".to_string(),
            query: "What database do I prefer?".to_string(),
            ground_truth: GroundTruth {
                // The fresh memory (PostgreSQL) should win over stale (MySQL)
                required_memory_content: vec!["prefers PostgreSQL".to_string()],
                required_answer_keywords: vec!["postgresql".to_string()],
                forbidden_terms: vec![], // MySQL might still be mentioned
                min_citations: 0,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![
                MemoryFixture {
                    id: "eval-mem-stale-old".to_string(),
                    memory_type: "PREFERENCE".to_string(),
                    content: "User prefers MySQL as their database".to_string(),
                    importance: 7,
                    simulated_age_days: 180.0, // 6 months old
                    is_stale: true,
                },
                MemoryFixture {
                    id: "eval-mem-stale-new".to_string(),
                    memory_type: "PREFERENCE".to_string(),
                    content: "User prefers PostgreSQL as their database".to_string(),
                    importance: 8,
                    simulated_age_days: 1.0, // very recent
                    is_stale: false,
                },
            ],
            constraints: TestConstraints {
                uses_seeded_memories: true,
                ..Default::default()
            },
        },

        // ── Seeded: Irrelevant Memory Rejection ──────────────────────────────
        TestCase {
            id: tid("MEM-IRR"),
            suite: TestSuite::Memory,
            category: TestCategory::IrrelevantMemoryRejection,
            description: "Irrelevant memory — hobby memory should not surface for technical query".to_string(),
            query: "How do I configure the Qdrant vector database connection?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Qdrant".to_string()],
                required_answer_keywords: vec!["qdrant".to_string()],
                forbidden_terms: vec!["gardening".to_string(), "cooking".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![MemoryFixture {
                id: "eval-mem-irr-001".to_string(),
                memory_type: "PREFERENCE".to_string(),
                content: "User enjoys gardening and cooking as hobbies".to_string(),
                importance: 5,
                simulated_age_days: 2.0,
                is_stale: false,
            }],
            constraints: TestConstraints {
                uses_seeded_memories: true,
                ..Default::default()
            },
        },

        // ── Live: Memory Deduplication ────────────────────────────────────────
        TestCase {
            id: tid("MEM-DEDUP"),
            suite: TestSuite::Memory,
            category: TestCategory::MemoryDeduplication,
            description: "Verify recalled memories contain no duplicates".to_string(),
            query: "What do you know about me?".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints {
                uses_seeded_memories: false, // uses live memories
                ..Default::default()
            },
        },
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
// 3. COMBINED RETRIEVAL + MEMORY TESTS
// ──────────────────────────────────────────────────────────────────────────────

fn combined_tests() -> Vec<TestCase> {
    vec![
        TestCase {
            id: tid("CMB-001"),
            suite: TestSuite::Combined,
            category: TestCategory::FactualLookup,
            description: "Combined — personal preference + document facts".to_string(),
            query: "How should I set up Notion given my workflow preferences?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Notion".to_string()],
                required_answer_keywords: vec!["notion".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![MemoryFixture {
                id: "eval-mem-cmb-001".to_string(),
                memory_type: "PREFERENCE".to_string(),
                content: "User prefers a folder-based hierarchical structure for their notes".to_string(),
                importance: 7,
                simulated_age_days: 4.0,
                is_stale: false,
            }],
            constraints: TestConstraints {
                uses_seeded_memories: true,
                ..Default::default()
            },
        },
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
// 4. HALLUCINATION TESTS
// ──────────────────────────────────────────────────────────────────────────────

fn hallucination_tests() -> Vec<TestCase> {
    vec![
        // ── Canary: Non-existent feature ─────────────────────────────────────
        TestCase {
            id: tid("HAL-001"),
            suite: TestSuite::Hallucination,
            category: TestCategory::HallucinationCanary,
            description: "Canary — assistant must not fabricate quantum computing feature".to_string(),
            query: "Explain the assistant's built-in quantum computing module.".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec![
                    "EMPTY_RETRIEVAL".to_string(),
                    "LOW_CONFIDENCE_RETRIEVAL".to_string(),
                ],
                forbidden_terms: vec![
                    "quantum module".to_string(),
                    "qubit".to_string(),
                    "quantum circuit".to_string(),
                ],
                answer_characteristics: vec![AnswerCharacteristic::AcknowledgesUncertainty],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Canary: Non-existent person ───────────────────────────────────────
        TestCase {
            id: tid("HAL-002"),
            suite: TestSuite::Hallucination,
            category: TestCategory::HallucinationCanary,
            description: "Canary — must not invent a CEO named 'Dr. Evelyn Park'".to_string(),
            query: "What did Dr. Evelyn Park say about the assistant's roadmap?".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec![
                    "EMPTY_RETRIEVAL".to_string(),
                    "LOW_CONFIDENCE_RETRIEVAL".to_string(),
                    "AMBIGUOUS_RETRIEVAL".to_string(),
                ],
                forbidden_terms: vec!["dr. evelyn park said".to_string(), "evelyn park stated".to_string()],
                answer_characteristics: vec![AnswerCharacteristic::AcknowledgesUncertainty],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Canary: Contradicts known facts ──────────────────────────────────
        TestCase {
            id: tid("HAL-003"),
            suite: TestSuite::Hallucination,
            category: TestCategory::HallucinationCanary,
            description: "Canary — must not claim Obsidian uses cloud sync (it's local)".to_string(),
            query: "Does Obsidian sync documents to the cloud automatically?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Obsidian".to_string()],
                forbidden_terms: vec!["automatically syncs to the cloud".to_string()],
                min_citations: 0,
                expected_statuses: vec![
                    "OK".to_string(),
                    "PARTIAL_RETRIEVAL".to_string(),
                    "LOW_CONFIDENCE_RETRIEVAL".to_string(),
                ],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Canary: No data for time period ──────────────────────────────────
        TestCase {
            id: tid("HAL-004"),
            suite: TestSuite::Hallucination,
            category: TestCategory::HallucinationCanary,
            description: "Canary — must not fabricate future release dates".to_string(),
            query: "When will version 3.0 of the assistant be released?".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec![
                    "EMPTY_RETRIEVAL".to_string(),
                    "LOW_CONFIDENCE_RETRIEVAL".to_string(),
                ],
                forbidden_terms: vec!["version 3.0 will be released on".to_string()],
                answer_characteristics: vec![AnswerCharacteristic::AcknowledgesUncertainty],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
// 5. CITATION TESTS
// ──────────────────────────────────────────────────────────────────────────────

fn citation_tests() -> Vec<TestCase> {
    vec![
        // ── Citation integrity ────────────────────────────────────────────────
        TestCase {
            id: tid("CIT-INT"),
            suite: TestSuite::Citation,
            category: TestCategory::SpecificQuestion,
            description: "Citation integrity — every cited chunk_id must be in retrieved set".to_string(),
            query: "How does the reranking step improve retrieval quality?".to_string(),
            ground_truth: GroundTruth {
                required_answer_keywords: vec!["rerank".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Citation: Multi-source ────────────────────────────────────────────
        TestCase {
            id: tid("CIT-MULTI"),
            suite: TestSuite::Citation,
            category: TestCategory::BroadQuestion,
            description: "Multi-source citation — answer should cite multiple distinct documents".to_string(),
            query: "How does the system handle both Notion documents and Obsidian notes?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Notion".to_string(), "Obsidian".to_string()],
                min_citations: 2,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
// 6. PROMPT ASSEMBLY TESTS
// ──────────────────────────────────────────────────────────────────────────────

fn prompt_assembly_tests() -> Vec<TestCase> {
    vec![
        // ── Section Ordering ──────────────────────────────────────────────────
        TestCase {
            id: tid("PROMPT-ORD"),
            suite: TestSuite::PromptAssembly,
            category: TestCategory::PromptOrderCheck,
            description: "Verify prompt section ordering: Summary → LTM → Episodes → Messages → RAG → Query".to_string(),
            query: "What do you know about my Obsidian setup?".to_string(),
            ground_truth: GroundTruth {
                min_citations: 0,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![MemoryFixture {
                id: "eval-mem-prompt-ord".to_string(),
                memory_type: "EPISODE".to_string(),
                content: "User configured Obsidian vault at ~/Documents/Notes".to_string(),
                importance: 6,
                simulated_age_days: 2.0,
                is_stale: false,
            }],
            constraints: TestConstraints {
                uses_seeded_memories: true,
                has_conversation_context: true,
                prior_messages: vec![
                    ("user".to_string(), "I use Obsidian for my notes.".to_string()),
                    ("assistant".to_string(), "Understood, I will remember that.".to_string()),
                ],
                ..Default::default()
            },
        },

        // ── No Duplication ────────────────────────────────────────────────────
        TestCase {
            id: tid("PROMPT-DEDUP"),
            suite: TestSuite::PromptAssembly,
            category: TestCategory::PromptDuplicationCheck,
            description: "Verify no duplicate context blocks appear in assembled prompt".to_string(),
            query: "What are the authentication requirements for using the Google integration?".to_string(),
            ground_truth: GroundTruth {
                required_entities: vec!["Google".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
// 7. GROUNDING TESTS
// ──────────────────────────────────────────────────────────────────────────────

fn grounding_tests() -> Vec<TestCase> {
    vec![
        // ── Per-claim grounding ───────────────────────────────────────────────
        TestCase {
            id: tid("GRD-001"),
            suite: TestSuite::Grounding,
            category: TestCategory::SpecificQuestion,
            description: "Per-claim grounding — verify every sentence traces to a chunk".to_string(),
            query: "What are the steps to onboard a new user to the assistant?".to_string(),
            ground_truth: GroundTruth {
                required_answer_keywords: vec!["onboard".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                required_facts: vec![
                    "onboarding involves setting up integrations".to_string(),
                ],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },

        // ── Grounding with policy ─────────────────────────────────────────────
        TestCase {
            id: tid("GRD-002"),
            suite: TestSuite::Grounding,
            category: TestCategory::PolicyQuestion,
            description: "Policy grounding — every claim must trace to a policy document".to_string(),
            query: "What is the expense reimbursement policy?".to_string(),
            ground_truth: GroundTruth {
                required_answer_keywords: vec!["reimbursement".to_string(), "policy".to_string()],
                min_citations: 1,
                expected_statuses: vec!["OK".to_string(), "PARTIAL_RETRIEVAL".to_string()],
                ..Default::default()
            },
            memory_fixtures: vec![],
            constraints: TestConstraints::default(),
        },
    ]
}
