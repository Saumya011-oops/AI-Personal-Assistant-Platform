/// evaluation/mod.rs
///
/// Public API for the QA Evaluation Framework.
///
/// Modules:
///   types     — all shared types (TestCase, ExecutionTrace, EvalResult, …)
///   generator — programmatic test case factory
///   executor  — full pipeline trace capture + memory fixture management
///   evaluator — 7-dimension scoring + three-stage hallucination verifier
///   regression — baseline management + regression/improvement detection
///   reporter  — Markdown + JSON report generation

pub mod evaluator;
pub mod executor;
pub mod generator;
pub mod regression;
pub mod reporter;
pub mod types;

#[cfg(test)]
pub mod tests;

// Re-export the most commonly used types at the module root for ergonomics.
pub use types::{
    AnswerCharacteristic, Baseline, BaselineEntry, ClaimSupport, ClaimVerification, DimensionScore,
    EvalResult, EvalScorecard, ExecutionTrace, FailureRootCause, FixProposal, GroundTruth,
    LatencyBreakdown, MemoryFixture, RankedMemorySnapshot, RunReport, TestCase, TestCategory,
    TestConstraints, TestSuite,
};
