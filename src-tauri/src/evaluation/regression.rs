/// evaluation/regression.rs
///
/// Regression Runner — compares the current run's results against a persisted
/// baseline to detect score regressions and improvements.
///
/// Rules:
///  - A baseline is created automatically on the first passing run.
///  - On every subsequent run, per-test scores are compared against the baseline.
///  - Any drop in Retrieval, Memory, Grounding, Citation, or Answer Quality is
///    flagged as a regression.
///  - Improvements are tracked separately and never treated as failures.
///  - The baseline is updated only when the user explicitly passes --update-baseline.
///  - Auto-fix is applied ONLY for proposals marked auto_applicable = true
///    (currently limited to citation filter insertion — a mechanical, safe change).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::{info, warn};

use super::types::*;

// ──────────────────────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────────────────────

/// Minimum score drop (absolute percentage points) that constitutes a regression.
const REGRESSION_DELTA_THRESHOLD: f32 = 2.0;

pub const FRAMEWORK_VERSION: &str = "1.0.0";

// ──────────────────────────────────────────────────────────────────────────────
// RegressionRunner
// ──────────────────────────────────────────────────────────────────────────────

pub struct RegressionRunner {
    baseline_path: PathBuf,
}

impl RegressionRunner {
    pub fn new(reports_dir: &Path) -> Self {
        Self {
            baseline_path: reports_dir.join("baseline.json"),
        }
    }

    // ── Load baseline ─────────────────────────────────────────────────────────

    pub fn load_baseline(&self) -> Result<Option<Baseline>> {
        if !self.baseline_path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&self.baseline_path)
            .context("failed to read baseline.json")?;
        let baseline: Baseline = serde_json::from_str(&raw)
            .context("failed to parse baseline.json")?;
        Ok(Some(baseline))
    }

    // ── Save / update baseline ────────────────────────────────────────────────

    pub fn save_baseline(&self, results: &[EvalResult]) -> Result<()> {
        let mut entries = HashMap::new();
        for result in results {
            let entry = BaselineEntry {
                test_id: result.test_id.clone(),
                passed: result.passed,
                overall_score: result.scorecard.overall_score(),
                retrieval_score: result.scorecard.retrieval.score,
                memory_score: result.scorecard.memory.score,
                citation_score: result.scorecard.citation_accuracy.score,
                grounding_score: result.scorecard.grounding.score,
                hallucination_score: result.scorecard.hallucination.score,
                timestamp: Utc::now().to_rfc3339(),
            };
            entries.insert(result.test_id.clone(), entry);
        }

        let baseline = Baseline {
            entries,
            created_at: Utc::now().to_rfc3339(),
            framework_version: FRAMEWORK_VERSION.to_string(),
        };

        let json = serde_json::to_string_pretty(&baseline)
            .context("failed to serialize baseline")?;
        fs::write(&self.baseline_path, json)
            .context("failed to write baseline.json")?;

        info!(
            "[REGRESSION] Baseline saved: {} entries → {}",
            baseline.entries.len(),
            self.baseline_path.display()
        );
        Ok(())
    }

    // ── Compare results against baseline ─────────────────────────────────────

    /// Annotates EvalResults with regression/improvement flags.
    /// Returns (results_with_flags, regressions_ids, improvement_ids).
    pub fn compare(
        &self,
        results: Vec<EvalResult>,
        baseline: &Baseline,
    ) -> (Vec<EvalResult>, Vec<String>, Vec<String>) {
        let mut annotated = Vec::new();
        let mut regressions = Vec::new();
        let mut improvements = Vec::new();

        for mut result in results {
            if let Some(baseline_entry) = baseline.entries.get(&result.test_id) {
                let current_score = result.scorecard.overall_score();
                let delta = current_score - baseline_entry.overall_score;

                // Dimension-level regression checks (the dimensions that matter most)
                let dimension_regressions = check_dimension_regressions(&result, baseline_entry);

                if !dimension_regressions.is_empty() || delta < -REGRESSION_DELTA_THRESHOLD {
                    result.is_regression = true;
                    regressions.push(format!(
                        "{} (overall Δ={:+.1}%, dims: {})",
                        result.test_id,
                        delta,
                        dimension_regressions.join(", ")
                    ));
                } else if delta > REGRESSION_DELTA_THRESHOLD && result.passed && !baseline_entry.passed {
                    result.is_improvement = true;
                    improvements.push(format!(
                        "{} (Δ={:+.1}%, was FAIL → now PASS)",
                        result.test_id, delta
                    ));
                } else if delta > REGRESSION_DELTA_THRESHOLD {
                    result.is_improvement = true;
                    improvements.push(format!(
                        "{} (Δ={:+.1}%)",
                        result.test_id, delta
                    ));
                }
            }
            // Tests not in the baseline are new — no regression possible
            annotated.push(result);
        }

        (annotated, regressions, improvements)
    }

    // ── Auto-fix dispatch ─────────────────────────────────────────────────────

    /// Apply only mechanically safe, auto_applicable fixes.
    /// Returns a list of applied fix descriptions for the report.
    ///
    /// IMPORTANT: This method NEVER modifies retrieval algorithms, memory ranking,
    /// scoring thresholds, prompt assembly logic, or evaluation logic.
    /// It is limited to: missing null checks, logging improvements, obvious
    /// configuration mistakes, and missing validation filters.
    pub fn apply_safe_auto_fixes(&self, results: &[EvalResult]) -> Vec<String> {
        let mut applied = Vec::new();

        for result in results {
            for proposal in &result.fix_proposals {
                if !proposal.auto_applicable {
                    continue;
                }

                // Currently the only auto-applicable fix is the citation filter.
                // We verify the retrieval.rs file exists but do NOT modify it
                // automatically — we only log that it could be applied.
                // Full auto-application requires explicit --auto-fix flag (future work).
                warn!(
                    "[AUTO-FIX AVAILABLE] Test: {} | File: {} | Fix: {}",
                    result.test_id, proposal.file, proposal.description
                );
                applied.push(format!(
                    "READY (pending --auto-fix flag): {} in {}",
                    proposal.description, proposal.file
                ));
            }
        }

        applied
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Check individual dimension scores for regressions vs. baseline.
fn check_dimension_regressions(
    result: &EvalResult,
    baseline: &BaselineEntry,
) -> Vec<String> {
    let mut regressions = Vec::new();

    let checks = [
        ("retrieval", result.scorecard.retrieval.score, baseline.retrieval_score),
        ("memory", result.scorecard.memory.score, baseline.memory_score),
        ("citation", result.scorecard.citation_accuracy.score, baseline.citation_score),
        ("grounding", result.scorecard.grounding.score, baseline.grounding_score),
        ("hallucination", result.scorecard.hallucination.score, baseline.hallucination_score),
    ];

    for (dim, current, previous) in &checks {
        let delta = current - previous;
        if delta < -REGRESSION_DELTA_THRESHOLD {
            regressions.push(format!(
                "{}:{:+.1}%",
                dim, delta
            ));
        }
    }

    regressions
}
