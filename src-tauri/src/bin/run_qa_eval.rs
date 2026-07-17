/// run_qa_eval.rs — CLI entry point for the QA Evaluation Framework
///
/// Usage:
///   cargo run --bin run_qa_eval --no-default-features
///   cargo run --bin run_qa_eval --no-default-features -- --suite=retrieval
///   cargo run --bin run_qa_eval --no-default-features -- --suite=memory
///   cargo run --bin run_qa_eval --no-default-features -- --suite=hallucination
///   cargo run --bin run_qa_eval --no-default-features -- --mode=regression
///   cargo run --bin run_qa_eval --no-default-features -- --update-baseline
///
/// The binary initializes all production services (same as the app), generates
/// test cases programmatically, runs them through the full pipeline, scores each
/// one across 7 dimensions, compares against the baseline, and writes reports to
/// the reports/ directory at the project root.

use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::info;

use assistant_core::config::AppConfig;
use assistant_core::db::Database;
use assistant_core::evaluation::evaluator::Evaluator;
use assistant_core::evaluation::executor::Executor;
use assistant_core::evaluation::generator::generate_tests;
use assistant_core::evaluation::regression::{RegressionRunner, FRAMEWORK_VERSION};
use assistant_core::evaluation::reporter::Reporter;
use assistant_core::evaluation::types::TestSuite;
use assistant_core::services::context_builder::ContextBuilder;
use assistant_core::services::groq::GroqService;
use assistant_core::services::memory::MemoryService;
use assistant_core::services::ollama::OllamaService;
use assistant_core::services::qdrant::QdrantService;
use assistant_core::services::query_analyzer::QueryAnalyzerService;
use assistant_core::services::reranker::RerankerService;
use assistant_core::services::retrieval::RetrievalService;
use assistant_core::services::sparse::SparseRetrievalService;
use assistant_core::services::CredentialService;

// ──────────────────────────────────────────────────────────────────────────────
// CLI argument parsing (no external crate — simple manual parsing)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct CliArgs {
    /// Which suite(s) to run. None = all suites.
    suites: Option<Vec<TestSuite>>,
    /// If true, only compare against baseline (no new pipeline calls).
    regression_only: bool,
    /// If true, save the current results as the new baseline.
    update_baseline: bool,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cli = CliArgs::default();

    for arg in &args {
        if arg.starts_with("--suite=") {
            let suite_str = &arg["--suite=".len()..];
            let suites: Vec<TestSuite> = suite_str
                .split(',')
                .filter_map(|s| parse_suite(s.trim()))
                .collect();
            if !suites.is_empty() {
                cli.suites = Some(suites);
            }
        } else if arg == "--mode=regression" {
            cli.regression_only = true;
        } else if arg == "--update-baseline" {
            cli.update_baseline = true;
        }
    }
    cli
}

fn parse_suite(s: &str) -> Option<TestSuite> {
    match s.to_lowercase().as_str() {
        "retrieval" => Some(TestSuite::Retrieval),
        "memory"    => Some(TestSuite::Memory),
        "combined"  => Some(TestSuite::Combined),
        "hallucination" => Some(TestSuite::Hallucination),
        "citation"  => Some(TestSuite::Citation),
        "prompt" | "prompt_assembly" => Some(TestSuite::PromptAssembly),
        "grounding" => Some(TestSuite::Grounding),
        "regression" => Some(TestSuite::Regression),
        _ => {
            eprintln!("Unknown suite '{}', skipping.", s);
            None
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Main
// ──────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    let cli = parse_args();

    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║     AI Personal Assistant — QA Evaluation v{}       ║", FRAMEWORK_VERSION);
    println!("╚══════════════════════════════════════════════════════╝\n");

    // ── Load config ───────────────────────────────────────────────────────────
    info!("Loading configuration...");
    let config = AppConfig::load().context("Failed to load AppConfig")?;

    // Reports directory at project root (never inside src-tauri/)
    let reports_dir = config.data_dir.parent()
        .unwrap_or(std::path::Path::new("."))
        .join("reports");

    // ── Connect to database ───────────────────────────────────────────────────
    info!("Connecting to database at {}", config.database_path.display());
    let database = Database::connect(&config.database_path)
        .context("Failed to connect to database")?;
    database
        .run_migrations()
        .context("Failed to run database migrations")?;

    // ── Initialize services ───────────────────────────────────────────────────
    info!("Initializing services...");

    let ollama_service = Arc::new(OllamaService::new(
        config.ollama_url.clone(),
        config.embedding_model.clone(),
    ));
    let qdrant_service = QdrantService::new(
        config.qdrant_url.clone(),
        config.qdrant_collection.clone(),
    );
    let sparse_service = SparseRetrievalService::new(
        config.sparse_helper_port,
        config.sparse_helper_script_path(),
        config.node_binary.clone(),
    );
    let cred_service = Arc::new(
        CredentialService::new_no_handle()
            .context("Failed to create credential service")?,
    );
    let groq_service = GroqService::new(
        config.groq_api_key.clone(),
        Some(database.clone()),
        Some(cred_service),
        config.groq_base_url.clone(),
        config.groq_model_primary.clone(),
        config.groq_model_fallback.clone(),
    );
    let query_analyzer = QueryAnalyzerService::new(groq_service.clone());
    let reranker_service = RerankerService::new(
        config.reranker_helper_port,
        config.reranker_worker_script_path(),
        config.reranker_python_path(),
        config.reranker_model.clone(),
        config.reranker_model_cache_dir.clone(),
    );
    let context_builder = ContextBuilder::new();

    let retrieval_service = Arc::new(RetrievalService::new(
        (*ollama_service).clone(),
        qdrant_service,
        sparse_service,
        groq_service.clone(),
        query_analyzer,
        reranker_service,
        context_builder,
    ));

    let memory_service = Arc::new(MemoryService::new(
        database.clone(),
        (*ollama_service).clone(),
        groq_service.clone(),
        &config.qdrant_url,
    ));

    info!("Initializing Memory Service...");
    memory_service
        .initialize()
        .await
        .context("Failed to initialize memory service")?;

    info!("Initializing Retrieval Service...");
    retrieval_service
        .initialize(&database)
        .await
        .context("Failed to initialize retrieval service")?;

    // ── Framework layers ──────────────────────────────────────────────────────
    let executor = Executor::new(
        database.clone(),
        retrieval_service,
        memory_service,
        ollama_service.clone(),
    );
    executor.ensure_eval_conversation()?;

    let evaluator = Evaluator::new((*ollama_service).clone(), groq_service);
    let regression_runner = RegressionRunner::new(&reports_dir);
    let reporter = Reporter::new(&reports_dir);

    // ── Load baseline ─────────────────────────────────────────────────────────
    let baseline = regression_runner.load_baseline()?;
    if baseline.is_some() {
        println!("📊 Baseline loaded from {}", reports_dir.join("baseline.json").display());
    } else {
        println!("📊 No baseline found. This run will establish the first baseline.");
    }

    // ── Generate test cases ───────────────────────────────────────────────────
    let suites_ref: Option<&[TestSuite]> = cli.suites.as_deref();
    let test_cases = generate_tests(suites_ref);
    println!(
        "🧪 Generated {} test cases across {} suite(s)\n",
        test_cases.len(),
        cli.suites.as_ref().map(|s| s.len()).unwrap_or(7)
    );

    if test_cases.is_empty() {
        println!("No test cases to run. Exiting.");
        return Ok(());
    }

    // ── Execute and evaluate ──────────────────────────────────────────────────
    let mut eval_results = Vec::new();

    for (idx, test) in test_cases.iter().enumerate() {
        println!(
            "[{:>2}/{}] {} | {} | \"{}\"",
            idx + 1,
            test_cases.len(),
            test.suite,
            test.id,
            if test.query.len() > 60 {
                format!("{}…", &test.query[..60])
            } else {
                test.query.clone()
            }
        );

        // Execute through full pipeline
        let trace = executor.run(test).await;
        let exec_error = trace.error.clone();

        // Evaluate the trace
        let eval_result = evaluator.evaluate(test, &trace).await;

        let pass_icon = if eval_result.passed { "  ✅" } else { "  ❌" };
        let overall = eval_result.scorecard.overall_score();
        println!(
            "{}  Overall: {:.1}%  Ret:{:.0} Mem:{:.0} Hal:{:.0} Cit:{:.0} Grd:{:.0}  {}ms",
            pass_icon,
            overall,
            eval_result.scorecard.retrieval.score,
            eval_result.scorecard.memory.score,
            eval_result.scorecard.hallucination.score,
            eval_result.scorecard.citation_accuracy.score,
            eval_result.scorecard.grounding.score,
            trace.latency.total_ms,
        );

        if let Some(err) = exec_error {
            println!("     ⚠️  Error: {}", &err[..err.len().min(120)]);
        }

        eval_results.push(eval_result);
    }

    println!();

    // ── Regression comparison ─────────────────────────────────────────────────
    let (annotated_results, regressions, improvements) = if let Some(ref bl) = baseline {
        let (results, regs, imps) = regression_runner.compare(eval_results, bl);
        if !regs.is_empty() {
            println!("🔴 Regressions detected:");
            for r in &regs {
                println!("   - {}", r);
            }
        }
        if !imps.is_empty() {
            println!("🟢 Improvements detected:");
            for i in &imps {
                println!("   - {}", i);
            }
        }
        (results, regs, imps)
    } else {
        (eval_results, vec![], vec![])
    };

    // Check for auto-applicable fixes
    let auto_fixes = regression_runner.apply_safe_auto_fixes(&annotated_results);
    if !auto_fixes.is_empty() {
        println!("\n🔧 Auto-applicable fixes available (pass --auto-fix to apply):");
        for fix in &auto_fixes {
            println!("   - {}", fix);
        }
    }

    // ── Build and write report ────────────────────────────────────────────────
    let run_report = reporter.build_report(annotated_results, regressions, improvements);
    reporter.write(&run_report)?;

    // ── Print summary ─────────────────────────────────────────────────────────
    println!("\n─────────────────────────────────────────────────────");
    println!(
        "  Verdict: {}",
        if run_report.production_ready {
            "✅ PRODUCTION READY"
        } else {
            "❌ NOT PRODUCTION READY"
        }
    );
    println!(
        "  Tests:   {}/{} passed  ({} failed)",
        run_report.passed_tests, run_report.total_tests, run_report.failed_tests
    );
    let sc = &run_report.overall_scorecard;
    println!("  Scores:");
    println!("    Retrieval    {:.1}% {}", sc.retrieval.score, if sc.retrieval.passed { "✅" } else { "❌" });
    println!("    Memory       {:.1}% {}", sc.memory.score, if sc.memory.passed { "✅" } else { "❌" });
    println!("    Prompt       {:.1}% {}", sc.prompt_assembly.score, if sc.prompt_assembly.passed { "✅" } else { "❌" });
    println!("    Answer       {:.1}% {}", sc.answer_quality.score, if sc.answer_quality.passed { "✅" } else { "❌" });
    println!("    Hallucination{:.1}% {}", sc.hallucination.score, if sc.hallucination.passed { "✅" } else { "❌" });
    println!("    Citations    {:.1}% {}", sc.citation_accuracy.score, if sc.citation_accuracy.passed { "✅" } else { "❌" });
    println!("    Grounding    {:.1}% {}", sc.grounding.score, if sc.grounding.passed { "✅" } else { "❌" });
    println!("─────────────────────────────────────────────────────\n");

    // ── Update baseline if requested or if first run ──────────────────────────
    if cli.update_baseline || baseline.is_none() {
        info!("Saving baseline...");
        regression_runner.save_baseline(&run_report.results)?;
        if cli.update_baseline {
            println!("✅ Baseline updated with current run results.");
        } else {
            println!("✅ First baseline established.");
        }
    }

    // Exit with non-zero code if not production ready (useful for CI pipelines)
    if !run_report.production_ready {
        std::process::exit(1);
    }

    Ok(())
}
