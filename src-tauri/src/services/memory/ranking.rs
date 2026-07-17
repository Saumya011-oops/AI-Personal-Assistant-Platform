use chrono::{DateTime, Utc, NaiveDateTime};
use super::db::DbMemory;

pub struct RankedMemory {
    pub memory: DbMemory,
    pub final_score: f64,
    pub similarity: f64,
    pub importance_score: f64,
    pub recency_score: f64,
    pub access_freq_score: f64,
}

pub fn rank_memories(
    candidates: Vec<(DbMemory, f32)>, // Vector of (Memory, SimilarityScore)
) -> Vec<RankedMemory> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let now = Utc::now();

    let mut ranked: Vec<RankedMemory> = candidates
        .into_iter()
        .map(|(mem, sim)| {
            let similarity = sim as f64;
            let importance = mem.importance as f64;
            
            // Recency: exp(-days_elapsed / 7.0)
            let parsed_time = NaiveDateTime::parse_from_str(&mem.last_used, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
                .unwrap_or(now);
            let elapsed_seconds = now.signed_duration_since(parsed_time).num_seconds().max(0);
            let elapsed_days = (elapsed_seconds as f64) / (24.0 * 3600.0);
            let recency = (-elapsed_days / 7.0).exp();

            let access_count = mem.access_count as f64;

            // Fixed/stable normalization independent of the candidate pool size or values
            let norm_sim = similarity; 
            let norm_imp = importance / 10.0; 
            let norm_rec = recency; 
            let norm_acc = (access_count / 10.0).min(1.0); 

            // Weights: 0.55 Similarity, 0.20 Importance, 0.15 Recency, 0.10 Access Frequency
            let final_score = 0.55 * norm_sim + 0.20 * norm_imp + 0.15 * norm_rec + 0.10 * norm_acc;

            RankedMemory {
                memory: mem,
                final_score,
                similarity: norm_sim,
                importance_score: norm_imp,
                recency_score: norm_rec,
                access_freq_score: norm_acc,
            }
        })
        .collect();

    // Sort by final score descending
    ranked.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}
