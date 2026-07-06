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

    // 1. Calculate raw features
    let raw_features: Vec<(DbMemory, f64, f64, f64, f64)> = candidates
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

            (mem, similarity, importance, recency, access_count)
        })
        .collect();

    // 2. Perform Min-Max Normalization over the candidate pool

    let mut sim_min = f64::MAX; let mut sim_max = f64::MIN;
    let mut imp_min = f64::MAX; let mut imp_max = f64::MIN;
    let mut rec_min = f64::MAX; let mut rec_max = f64::MIN;
    let mut acc_min = f64::MAX; let mut acc_max = f64::MIN;

    for (_, sim, imp, rec, acc) in &raw_features {
        if *sim < sim_min { sim_min = *sim; }
        if *sim > sim_max { sim_max = *sim; }
        if *imp < imp_min { imp_min = *imp; }
        if *imp > imp_max { imp_max = *imp; }
        if *rec < rec_min { rec_min = *rec; }
        if *rec > rec_max { rec_max = *rec; }
        if *acc < acc_min { acc_min = *acc; }
        if *acc > acc_max { acc_max = *acc; }
    }

    // EPSILON to prevent division by zero
    let eps = 1e-6;

    let mut ranked: Vec<RankedMemory> = raw_features
        .into_iter()
        .map(|(mem, sim, imp, rec, acc)| {
            // Min-Max normalization with fallbacks if max == min
            let norm_sim = if (sim_max - sim_min).abs() < eps {
                sim // Fallback to absolute similarity
            } else {
                (sim - sim_min) / (sim_max - sim_min)
            };

            let norm_imp = if (imp_max - imp_min).abs() < eps {
                imp / 10.0 // Fallback to absolute scale 1-10
            } else {
                (imp - imp_min) / (imp_max - imp_min)
            };

            let norm_rec = if (rec_max - rec_min).abs() < eps {
                rec // Fallback to absolute exp decay
            } else {
                (rec - rec_min) / (rec_max - rec_min)
            };

            let norm_acc = if (acc_max - acc_min).abs() < eps {
                (acc / 20.0).min(1.0) // Fallback to cap at 20 accesses
            } else {
                (acc - acc_min) / (acc_max - acc_min)
            };

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
