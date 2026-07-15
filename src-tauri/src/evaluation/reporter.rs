/// evaluation/reporter.rs
///
/// Reporter — generates the final Markdown report and JSON scorecard from a
/// completed RunReport.
///
/// Outputs:
///   reports/qa_report_<timestamp>.md      — human-readable full report
///   reports/qa_scorecard_<timestamp>.json — machine-readable scorecard
///
/// The reporter never writes into src-tauri/ to avoid triggering Tauri's
/// file watcher (the root cause of the infinite rebuild bug fixed in Track 1).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use super::types::*;

// ──────────────────────────────────────────────────────────────────────────────
// Reporter
// ──────────────────────────────────────────────────────────────────────────────

pub struct Reporter {
    reports_dir: std::path::PathBuf,
}

impl Reporter {
    pub fn new(reports_dir: &Path) -> Self {
        Self {
            reports_dir: reports_dir.to_path_buf(),
        }
    }

    /// Build a RunReport from the list of EvalResults.
    pub fn build_report(
        &self,
        results: Vec<EvalResult>,
        regressions: Vec<String>,
        improvements: Vec<String>,
    ) -> RunReport {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();

        // Aggregate per-suite pass rates
        let mut suite_buckets: HashMap<String, (usize, usize)> = HashMap::new();
        for result in &results {
            let suite_key = result.suite.to_string();
            let entry = suite_buckets.entry(suite_key).or_insert((0, 0));
            entry.0 += 1; // total
            if result.passed {
                entry.1 += 1; // passed
            }
        }
        let per_suite_scores: HashMap<String, f32> = suite_buckets
            .iter()
            .map(|(suite, (total, passed))| {
                (suite.clone(), (*passed as f32 / *total as f32) * 100.0)
            })
            .collect();

        // Aggregate overall dimension scores (weighted average across all tests)
        let overall_scorecard = aggregate_scorecard(&results);

        let production_ready = overall_scorecard.production_ready()
            && regressions.is_empty();

        RunReport {
            run_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            production_ready,
            total_tests: total,
            passed_tests: passed,
            failed_tests: total - passed,
            regressions,
            improvements,
            overall_scorecard,
            per_suite_scores,
            results,
        }
    }

    /// Write the full Markdown report and JSON scorecard to reports/.
    pub fn write(&self, report: &RunReport) -> Result<()> {
        fs::create_dir_all(&self.reports_dir)
            .context("failed to create reports directory")?;

        let ts = Utc::now().format("%Y%m%d_%H%M%S").to_string();

        // ── JSON scorecard ────────────────────────────────────────────────────
        let scorecard_path = self.reports_dir.join(format!("qa_scorecard_{}.json", ts));
        let scorecard_json = serde_json::to_string_pretty(report)
            .context("failed to serialize report to JSON")?;
        fs::write(&scorecard_path, &scorecard_json)
            .context("failed to write JSON scorecard")?;

        // ── Markdown report ───────────────────────────────────────────────────
        let md = self.render_markdown(report);
        let report_path = self.reports_dir.join(format!("qa_report_{}.md", ts));
        fs::write(&report_path, &md)
            .context("failed to write Markdown report")?;

        // ── Also write a "latest" symlink for easy access ─────────────────────
        let latest_md = self.reports_dir.join("qa_report_latest.md");
        let latest_json = self.reports_dir.join("qa_scorecard_latest.json");
        let _ = fs::copy(&report_path, &latest_md);
        let _ = fs::copy(&scorecard_path, &latest_json);

        println!("\n╔══════════════════════════════════════════════════════╗");
        println!("║            QA EVALUATION COMPLETE                   ║");
        println!("╠══════════════════════════════════════════════════════╣");
        println!("║  Report  → {:<41}║", report_path.display());
        println!("║  Scores  → {:<41}║", scorecard_path.display());
        println!("╚══════════════════════════════════════════════════════╝");

        Ok(())
    }

    // ── Markdown rendering ────────────────────────────────────────────────────

    fn render_markdown(&self, report: &RunReport) -> String {
        let mut md = String::new();

        // ── Header ────────────────────────────────────────────────────────────
        let verdict = if report.production_ready {
            "✅ PRODUCTION READY"
        } else {
            "❌ NOT PRODUCTION READY"
        };

        md.push_str(&format!(
            "# QA Evaluation Report\n\n\
             **Run ID**: `{}`  \n\
             **Timestamp**: {}  \n\
             **Verdict**: {}  \n\n",
            report.run_id, report.timestamp, verdict
        ));

        // ── Overall scorecard ─────────────────────────────────────────────────
        md.push_str("## Overall Scorecard\n\n");
        md.push_str("| Dimension | Score | Threshold | Status |\n");
        md.push_str("|---|---|---|---|\n");

        let sc = &report.overall_scorecard;
        let dims = [
            ("Retrieval", &sc.retrieval),
            ("Memory", &sc.memory),
            ("Prompt Assembly", &sc.prompt_assembly),
            ("Answer Quality", &sc.answer_quality),
            ("Hallucination Rate", &sc.hallucination),
            ("Citation Accuracy", &sc.citation_accuracy),
            ("Grounding", &sc.grounding),
        ];
        for (name, dim) in &dims {
            let icon = if dim.passed { "✅" } else { "❌" };
            md.push_str(&format!(
                "| {} | {:.1}% | ≥ {:.0}% | {} |\n",
                name, dim.score, dim.threshold, icon
            ));
        }

        md.push_str(&format!(
            "\n**Overall**: {:.1}% | **Passed**: {}/{} | **Failed**: {}/{}\n\n",
            report.overall_scorecard.overall_score(),
            report.passed_tests,
            report.total_tests,
            report.failed_tests,
            report.total_tests
        ));

        // ── Per-suite scores ──────────────────────────────────────────────────
        md.push_str("## Per-Suite Scores\n\n");
        md.push_str("| Suite | Score |\n|---|---|\n");
        let mut suite_scores: Vec<(&String, &f32)> = report.per_suite_scores.iter().collect();
        suite_scores.sort_by(|a, b| a.0.cmp(b.0));
        for (suite, score) in &suite_scores {
            let icon = if **score >= 95.0 { "✅" } else { "❌" };
            md.push_str(&format!("| {} | {:.1}% {} |\n", suite, score, icon));
        }
        md.push('\n');

        // ── Regressions ───────────────────────────────────────────────────────
        if !report.regressions.is_empty() {
            md.push_str("## ⚠️ Regressions Detected\n\n");
            for reg in &report.regressions {
                md.push_str(&format!("- {}\n", reg));
            }
            md.push('\n');
        }

        // ── Improvements ─────────────────────────────────────────────────────
        if !report.improvements.is_empty() {
            md.push_str("## 🚀 Improvements vs. Baseline\n\n");
            for imp in &report.improvements {
                md.push_str(&format!("- {}\n", imp));
            }
            md.push('\n');
        }

        // ── Failed tests with root causes ─────────────────────────────────────
        let failed: Vec<&EvalResult> = report.results.iter().filter(|r| !r.passed).collect();
        if !failed.is_empty() {
            md.push_str("## Failed Tests — Root Cause Analysis\n\n");
            for result in &failed {
                let regression_flag = if result.is_regression { " 🔴 REGRESSION" } else { "" };
                md.push_str(&format!(
                    "### ❌ `{}` [{}]{}\n\n",
                    result.test_id, result.suite, regression_flag
                ));
                md.push_str(&format!("**Query**: {}\n\n", result.query));

                // Scorecard for this test
                md.push_str("**Scores**:\n");
                let sc = &result.scorecard;
                let test_dims = [
                    ("Retrieval", sc.retrieval.score, sc.retrieval.passed),
                    ("Memory", sc.memory.score, sc.memory.passed),
                    ("Prompt", sc.prompt_assembly.score, sc.prompt_assembly.passed),
                    ("Answer", sc.answer_quality.score, sc.answer_quality.passed),
                    ("Hallucination", sc.hallucination.score, sc.hallucination.passed),
                    ("Citation", sc.citation_accuracy.score, sc.citation_accuracy.passed),
                    ("Grounding", sc.grounding.score, sc.grounding.passed),
                ];
                for (name, score, passed) in &test_dims {
                    md.push_str(&format!(
                        "- {}: {:.1}% {}\n",
                        name,
                        score,
                        if *passed { "✅" } else { "❌" }
                    ));
                }
                md.push('\n');

                // Root causes
                if !result.root_causes.is_empty() {
                    md.push_str("**Root Causes**:\n");
                    for cause in &result.root_causes {
                        md.push_str(&format!("- {}\n", format_root_cause(cause)));
                    }
                    md.push('\n');
                }

                // Fix proposals
                if !result.fix_proposals.is_empty() {
                    md.push_str("**Fix Proposals**:\n");
                    for fix in &result.fix_proposals {
                        let auto = if fix.auto_applicable {
                            " *(auto-applicable)*"
                        } else {
                            " *(manual)*"
                        };
                        md.push_str(&format!(
                            "\n> 📍 **{}**{}: {}\n>\n> ```rust\n",
                            fix.file, auto, fix.description
                        ));
                        for line in fix.proposed_change.lines() {
                            md.push_str(&format!("> {}\n", line));
                        }
                        md.push_str("> ```\n");
                    }
                    md.push('\n');
                }

                // Claim verifications
                let hallucinated_claims: Vec<&ClaimVerification> = result
                    .claim_verifications
                    .iter()
                    .filter(|c| {
                        c.support == ClaimSupport::Hallucinated
                            || c.support == ClaimSupport::Unsupported
                    })
                    .collect();

                if !hallucinated_claims.is_empty() {
                    md.push_str("**Ungrounded Claims**:\n\n");
                    md.push_str("| Claim | Status | Determined By | Similarity |\n");
                    md.push_str("|---|---|---|---|\n");
                    for claim in &hallucinated_claims {
                        md.push_str(&format!(
                            "| `{}` | {} | {} | {:.2} |\n",
                            truncate(&claim.claim, 80),
                            claim.support,
                            claim.determined_by,
                            claim.similarity_score.unwrap_or(0.0)
                        ));
                    }
                    md.push('\n');
                }

                md.push_str("---\n\n");
            }
        }

        // ── Full test table ───────────────────────────────────────────────────
        md.push_str("## All Test Results\n\n");
        md.push_str("| ID | Suite | Query | Pass | Ret | Mem | Prompt | Ans | Hal | Cit | Grd | Latency | Regression |\n");
        md.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");

        for result in &report.results {
            let reg_flag = if result.is_regression {
                "🔴"
            } else if result.is_improvement {
                "🟢"
            } else {
                ""
            };
            let sc = &result.scorecard;
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {}ms | {} |\n",
                result.test_id,
                result.suite,
                truncate(&result.query, 45),
                if result.passed { "✅" } else { "❌" },
                sc.retrieval.score,
                sc.memory.score,
                sc.prompt_assembly.score,
                sc.answer_quality.score,
                sc.hallucination.score,
                sc.citation_accuracy.score,
                sc.grounding.score,
                result.trace.latency.total_ms,
                reg_flag
            ));
        }
        md.push('\n');

        // ── Recommended next steps ────────────────────────────────────────────
        md.push_str("## Recommended Next Steps\n\n");
        if report.production_ready {
            md.push_str(
                "✅ All production-readiness criteria met. \
                 Update the baseline with `--update-baseline` to lock in these scores.\n",
            );
        } else {
            // Collect unique fix files from all failed tests
            let fix_files: std::collections::HashSet<String> = report
                .results
                .iter()
                .filter(|r| !r.passed)
                .flat_map(|r| r.fix_proposals.iter().map(|f| f.file.clone()))
                .collect();

            md.push_str("Address the following before re-evaluating:\n\n");
            for file in &fix_files {
                md.push_str(&format!("- Review `{}`\n", file));
            }
            md.push_str(
                "\nAfter fixes, run:\n\
                 ```bash\n\
                 cargo run --bin run_qa_eval --no-default-features\n\
                 ```\n",
            );
        }

        md
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Score aggregation
// ──────────────────────────────────────────────────────────────────────────────

/// Compute weighted-average scorecard across all results.
fn aggregate_scorecard(results: &[EvalResult]) -> EvalScorecard {
    if results.is_empty() {
        let zero = DimensionScore::new(0.0, 95.0, vec!["No results".to_string()]);
        return EvalScorecard {
            retrieval: zero.clone(),
            memory: zero.clone(),
            prompt_assembly: zero.clone(),
            answer_quality: zero.clone(),
            hallucination: zero.clone(),
            citation_accuracy: zero.clone(),
            grounding: zero,
        };
    }

    let n = results.len() as f32;

    let avg = |f: fn(&EvalResult) -> f32| -> f32 {
        results.iter().map(f).sum::<f32>() / n
    };

    let with_details = |score: f32, threshold: f32, label: &str| -> DimensionScore {
        DimensionScore::new(
            score,
            threshold,
            vec![format!("Average across {} tests: {:.1}%", results.len(), score)],
        )
    };

    EvalScorecard {
        retrieval: with_details(
            avg(|r| r.scorecard.retrieval.score),
            95.0,
            "retrieval",
        ),
        memory: with_details(
            avg(|r| r.scorecard.memory.score),
            95.0,
            "memory",
        ),
        prompt_assembly: with_details(
            avg(|r| r.scorecard.prompt_assembly.score),
            95.0,
            "prompt_assembly",
        ),
        answer_quality: with_details(
            avg(|r| r.scorecard.answer_quality.score),
            95.0,
            "answer_quality",
        ),
        hallucination: with_details(
            avg(|r| r.scorecard.hallucination.score),
            100.0,
            "hallucination",
        ),
        citation_accuracy: with_details(
            avg(|r| r.scorecard.citation_accuracy.score),
            100.0,
            "citation_accuracy",
        ),
        grounding: with_details(
            avg(|r| r.scorecard.grounding.score),
            100.0,
            "grounding",
        ),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Utilities
// ──────────────────────────────────────────────────────────────────────────────

fn format_root_cause(cause: &FailureRootCause) -> String {
    match cause {
        FailureRootCause::EmptyRetrieval =>
            "EmptyRetrieval: No documents matched the query".to_string(),
        FailureRootCause::RankingFailure { expected, got } =>
            format!("RankingFailure: expected {:?}, got {:?}", expected, &got[..got.len().min(3)]),
        FailureRootCause::EntityRecallGap { missing } =>
            format!("EntityRecallGap: entities {:?} not in retrieved chunks", missing),
        FailureRootCause::MemoryRecallError { detail } =>
            format!("MemoryRecallError: {}", detail),
        FailureRootCause::PromptAssemblyError { detail } =>
            format!("PromptAssemblyError: {}", detail),
        FailureRootCause::HallucinatedClaim { claim } =>
            format!("HallucinatedClaim: \"{}\"", truncate(claim, 100)),
        FailureRootCause::FabricatedCitation { chunk_id } =>
            format!("FabricatedCitation: chunk_id '{}'", chunk_id),
        FailureRootCause::AnswerQualityFailure { missing } =>
            format!("AnswerQualityFailure: missing keywords {:?}", missing),
        FailureRootCause::ExecutionError { message } =>
            format!("ExecutionError: {}", message),
        FailureRootCause::Unknown { detail } =>
            format!("Unknown: {}", detail),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
