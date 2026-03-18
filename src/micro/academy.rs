// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! academy — Recursive training feedback loop.
//!
//! Tournament results → gap detection → challenge generation → curriculum update.
//! The academy reads tournament data, identifies weak spots, and generates
//! new challenges targeting those gaps. Each tournament run feeds the next.
//!
//! Flow:
//!   1. Analyze tournament results per model, per category
//!   2. Score challenge difficulty from pass rates
//!   3. Detect skill gaps (categories where models fail most)
//!   4. Generate new challenges targeting gaps
//!   5. Retire easy challenges (100% pass rate = too simple)
//!   6. Feed back into next tournament

use std::collections::HashMap;
use std::path::PathBuf;

use super::tournament::T165;

// ── Difficulty Scoring ──────────────────────────────────────────

/// T140=ChallengeDifficulty
/// Per-challenge difficulty score derived from tournament pass rates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T140 {
    pub description: String,
    pub category: String,
    pub total_attempts: usize,
    pub total_passes: usize,
    pub pass_rate: f64,
    /// 0.0 = trivial (everyone passes), 1.0 = impossible (nobody passes)
    pub difficulty: f64,
    /// Should this challenge be retired? (pass_rate == 1.0 with enough attempts)
    pub retire: bool,
    /// Should this challenge be flagged for review? (pass_rate == 0.0)
    pub broken: bool,
}

/// T141=ModelProfile
/// Per-model skill profile from tournament results.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T141 {
    pub model: String,
    pub node_id: String,
    /// Per-category accuracy: category → (passed, total, accuracy)
    pub categories: HashMap<String, T142>,
    /// Overall accuracy
    pub overall_accuracy: f64,
    /// Weakest categories (sorted worst → best)
    pub gaps: Vec<String>,
    /// Strongest categories
    pub strengths: Vec<String>,
}

/// T142=CategorySkill
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T142 {
    pub passed: usize,
    pub total: usize,
    pub accuracy: f64,
}

/// T143=AcademyReport
/// Full academy analysis of a tournament.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T143 {
    pub timestamp: String,
    pub challenge_difficulties: Vec<T140>,
    pub model_profiles: Vec<T141>,
    /// Challenges to retire (too easy)
    pub retire_candidates: Vec<String>,
    /// Challenges to review (possibly broken)
    pub broken_candidates: Vec<String>,
    /// Categories needing more challenges (high fail rate + few challenges)
    pub curriculum_gaps: Vec<T144>,
    /// Recommended next actions
    pub recommendations: Vec<String>,
}

/// T144=CurriculumGap
/// A gap in the curriculum that needs new challenges.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T144 {
    pub category: String,
    /// Average pass rate across all models in this category
    pub avg_pass_rate: f64,
    /// Number of existing challenges
    pub challenge_count: usize,
    /// Models that struggle most here
    pub struggling_models: Vec<String>,
    /// Suggested difficulty for new challenges (0.0-1.0)
    pub target_difficulty: f64,
}

// ── Analysis ────────────────────────────────────────────────────

/// f230=analyze
/// Analyze tournament results and produce an academy report.
pub fn f230(result: &T165) -> T143 {
    let difficulties = score_difficulties(result);
    let profiles = build_profiles(result);
    let curriculum_gaps = detect_gaps(result, &difficulties);

    let retire_candidates: Vec<String> = difficulties.iter()
        .filter(|d| d.retire)
        .map(|d| d.description.clone())
        .collect();

    let broken_candidates: Vec<String> = difficulties.iter()
        .filter(|d| d.broken)
        .map(|d| d.description.clone())
        .collect();

    let recommendations = generate_recommendations(&difficulties, &profiles, &curriculum_gaps);

    T143 {
        timestamp: format!("{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs()),
        challenge_difficulties: difficulties,
        model_profiles: profiles,
        retire_candidates,
        broken_candidates,
        curriculum_gaps,
        recommendations,
    }
}

/// Score each challenge's difficulty from pass rates.
fn score_difficulties(result: &T165) -> Vec<T140> {
    let mut by_challenge: HashMap<String, (String, usize, usize)> = HashMap::new();

    for m in &result.matches {
        let entry = by_challenge.entry(m.challenge.clone())
            .or_insert_with(|| (m.category.clone(), 0, 0));
        entry.1 += 1; // total
        if m.passed { entry.2 += 1; } // passes
    }

    let min_attempts = 3; // need at least 3 attempts to judge difficulty

    by_challenge.into_iter().map(|(desc, (cat, total, passed))| {
        let pass_rate = if total > 0 { passed as f64 / total as f64 } else { 0.0 };
        T140 {
            description: desc,
            category: cat,
            total_attempts: total,
            total_passes: passed,
            pass_rate,
            difficulty: 1.0 - pass_rate,
            retire: total >= min_attempts && passed == total,
            broken: total >= min_attempts && passed == 0,
        }
    }).collect()
}

/// Build per-model skill profiles.
fn build_profiles(result: &T165) -> Vec<T141> {
    let mut by_model: HashMap<String, HashMap<String, (usize, usize)>> = HashMap::new();
    let mut model_nodes: HashMap<String, String> = HashMap::new();

    for m in &result.matches {
        let key = format!("{}@{}", m.competitor.model, m.competitor.node_id);
        model_nodes.insert(key.clone(), m.competitor.node_id.clone());
        let cats = by_model.entry(key).or_default();
        let entry = cats.entry(m.category.clone()).or_insert((0, 0));
        entry.0 += 1; // total
        if m.passed { entry.1 += 1; } // passes
    }

    by_model.into_iter().map(|(key, cats)| {
        let model = key.split('@').next().unwrap_or(&key).to_string();
        let node_id = model_nodes.get(&key).cloned().unwrap_or_default();

        let categories: HashMap<String, T142> = cats.into_iter()
            .map(|(cat, (total, passed))| {
                let accuracy = if total > 0 { passed as f64 / total as f64 } else { 0.0 };
                (cat, T142 { passed, total, accuracy })
            })
            .collect();

        let overall_total: usize = categories.values().map(|c| c.total).sum();
        let overall_passed: usize = categories.values().map(|c| c.passed).sum();
        let overall_accuracy = if overall_total > 0 {
            overall_passed as f64 / overall_total as f64
        } else { 0.0 };

        // Sort categories by accuracy to find gaps and strengths
        let mut sorted: Vec<(String, f64)> = categories.iter()
            .map(|(k, v)| (k.clone(), v.accuracy))
            .collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let gaps: Vec<String> = sorted.iter()
            .filter(|(_, acc)| *acc < 0.5)
            .map(|(k, _)| k.clone())
            .collect();

        let strengths: Vec<String> = sorted.iter().rev()
            .filter(|(_, acc)| *acc >= 0.7)
            .map(|(k, _)| k.clone())
            .collect();

        T141 {
            model,
            node_id,
            categories,
            overall_accuracy,
            gaps,
            strengths,
        }
    }).collect()
}

/// Detect curriculum gaps — categories that need more/better challenges.
fn detect_gaps(
    result: &T165,
    difficulties: &[T140],
) -> Vec<T144> {
    // Group by category
    let mut cat_stats: HashMap<String, (f64, usize, usize)> = HashMap::new(); // (sum_pass_rate, count, model_count)
    for d in difficulties {
        let entry = cat_stats.entry(d.category.clone()).or_insert((0.0, 0, 0));
        entry.0 += d.pass_rate;
        entry.1 += 1;
    }

    // Count struggling models per category
    let mut cat_strugglers: HashMap<String, Vec<String>> = HashMap::new();
    let mut by_model_cat: HashMap<(String, String), (usize, usize)> = HashMap::new();
    for m in &result.matches {
        let key = (m.competitor.model.clone(), m.category.clone());
        let entry = by_model_cat.entry(key).or_insert((0, 0));
        entry.0 += 1;
        if m.passed { entry.1 += 1; }
    }
    for ((model, cat), (total, passed)) in &by_model_cat {
        if *total > 0 && (*passed as f64 / *total as f64) < 0.3 {
            cat_strugglers.entry(cat.clone()).or_default().push(model.clone());
        }
    }

    cat_stats.into_iter().map(|(cat, (sum_pass, count, _))| {
        let avg_pass_rate = if count > 0 { sum_pass / count as f64 } else { 0.0 };
        let struggling = cat_strugglers.get(&cat).cloned().unwrap_or_default();

        // Target difficulty: if avg pass rate is high, make harder challenges
        // If avg pass rate is low, add easier stepping-stone challenges
        let target_difficulty = if avg_pass_rate > 0.7 {
            0.8 // most models pass → add harder ones
        } else if avg_pass_rate < 0.3 {
            0.3 // most models fail → add easier stepping stones
        } else {
            0.5 // balanced
        };

        T144 {
            category: cat,
            avg_pass_rate,
            challenge_count: count,
            struggling_models: struggling,
            target_difficulty,
        }
    }).collect()
}

/// Generate actionable recommendations from the analysis.
fn generate_recommendations(
    difficulties: &[T140],
    profiles: &[T141],
    gaps: &[T144],
) -> Vec<String> {
    let mut recs = Vec::new();

    // Retirement recommendations
    let retire_count = difficulties.iter().filter(|d| d.retire).count();
    if retire_count > 0 {
        recs.push(format!(
            "Retire {} challenges with 100% pass rate — they no longer differentiate models",
            retire_count
        ));
    }

    // Broken challenge recommendations
    let broken_count = difficulties.iter().filter(|d| d.broken).count();
    if broken_count > 0 {
        recs.push(format!(
            "Review {} challenges with 0% pass rate — verify criteria or adjust difficulty",
            broken_count
        ));
    }

    // Curriculum gaps
    for gap in gaps {
        if gap.avg_pass_rate < 0.2 && gap.challenge_count < 5 {
            recs.push(format!(
                "Add easier {} challenges — {:.0}% avg pass rate, {} struggling models",
                gap.category, gap.avg_pass_rate * 100.0, gap.struggling_models.len()
            ));
        } else if gap.avg_pass_rate > 0.8 {
            recs.push(format!(
                "Add harder {} challenges — {:.0}% avg pass rate, current set is too easy",
                gap.category, gap.avg_pass_rate * 100.0
            ));
        }
    }

    // Model-specific recommendations
    let best = profiles.iter().max_by(|a, b|
        a.overall_accuracy.partial_cmp(&b.overall_accuracy).unwrap_or(std::cmp::Ordering::Equal)
    );
    if let Some(best) = best && !best.gaps.is_empty() {
        recs.push(format!(
                "Even the best model ({}, {:.0}% overall) struggles with: {}",
                best.model, best.overall_accuracy * 100.0,
                best.gaps.join(", ")
            ));
    }

    // Training data recommendations
    let total_matches = profiles.iter().map(|p|
        p.categories.values().map(|c| c.total).sum::<usize>()
    ).sum::<usize>();
    if total_matches < 1000 {
        recs.push("Run more tournament rounds — need >1000 matches for solid DPO training data".into());
    }

    recs
}

// ── Display ─────────────────────────────────────────────────────

/// f231=print_report
/// Print academy report.
pub fn f231(report: &T143) {
    println!("\nKOVA ACADEMY — CURRICULUM ANALYSIS");
    println!("═══════════════════════════════════════════════════════════════════");

    // Challenge difficulty distribution
    println!("\nCHALLENGE DIFFICULTY DISTRIBUTION");
    println!("───────────────────────────────────────────────────────────────────");
    let mut sorted = report.challenge_difficulties.clone();
    sorted.sort_by(|a, b| b.difficulty.partial_cmp(&a.difficulty).unwrap_or(std::cmp::Ordering::Equal));
    for d in &sorted {
        let bar_len = (d.difficulty * 20.0) as usize;
        let bar: String = "█".repeat(bar_len) + &"░".repeat(20 - bar_len);
        let flag = if d.retire { " [RETIRE]" } else if d.broken { " [BROKEN]" } else { "" };
        println!(
            "  {:<40} {} {:.0}% fail  ({}/{}){}",
            d.description, bar, d.difficulty * 100.0,
            d.total_attempts - d.total_passes, d.total_attempts, flag
        );
    }

    // Model profiles
    println!("\nMODEL SKILL PROFILES");
    println!("───────────────────────────────────────────────────────────────────");
    let mut profiles = report.model_profiles.clone();
    profiles.sort_by(|a, b| b.overall_accuracy.partial_cmp(&a.overall_accuracy).unwrap_or(std::cmp::Ordering::Equal));
    for p in &profiles {
        let gaps_str = if p.gaps.is_empty() { "none".into() } else { p.gaps.join(", ") };
        let strengths_str = if p.strengths.is_empty() { "none".into() } else { p.strengths.join(", ") };
        println!(
            "  {:<28} {:.0}% overall  gaps=[{}]  strengths=[{}]",
            p.model, p.overall_accuracy * 100.0, gaps_str, strengths_str
        );
    }

    // Curriculum gaps
    if !report.curriculum_gaps.is_empty() {
        println!("\nCURRICULUM GAPS");
        println!("───────────────────────────────────────────────────────────────────");
        let mut gaps = report.curriculum_gaps.clone();
        gaps.sort_by(|a, b| a.avg_pass_rate.partial_cmp(&b.avg_pass_rate).unwrap_or(std::cmp::Ordering::Equal));
        for g in &gaps {
            println!(
                "  {:<16} {:.0}% avg pass  {} challenges  {} struggling models  → target difficulty {:.0}%",
                g.category, g.avg_pass_rate * 100.0, g.challenge_count,
                g.struggling_models.len(), g.target_difficulty * 100.0
            );
        }
    }

    // Retire/broken
    if !report.retire_candidates.is_empty() {
        println!("\nRETIRE (100% pass rate):");
        for c in &report.retire_candidates { println!("  - {}", c); }
    }
    if !report.broken_candidates.is_empty() {
        println!("\nBROKEN (0% pass rate):");
        for c in &report.broken_candidates { println!("  - {}", c); }
    }

    // Recommendations
    if !report.recommendations.is_empty() {
        println!("\nRECOMMENDATIONS");
        println!("───────────────────────────────────────────────────────────────────");
        for (i, r) in report.recommendations.iter().enumerate() {
            println!("  {}. {}", i + 1, r);
        }
    }

    println!("\n═══════════════════════════════════════════════════════════════════");
}

/// f232=save_report
/// Save academy report to disk.
pub fn f232(report: &T143) -> Result<PathBuf, String> {
    let path = academy_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

fn academy_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".kova").join("micro").join("academy.json")
}
