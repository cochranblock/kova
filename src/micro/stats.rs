// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! stats — Persistent per-template run statistics.
//! Tracks: run count, pass/fail, avg latency, total tokens.
//! Stored as JSON in ~/.kova/micro/stats.json.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// T157=TemplateStats
/// Per-template stats.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T157 {
    pub runs: u64,
    pub passes: u64,
    pub failures: u64,
    pub errors: u64,
    pub total_duration_ms: u64,
    pub total_tokens: u64,
}

impl T157 {
    fn new() -> Self {
        T157 {
            runs: 0,
            passes: 0,
            failures: 0,
            errors: 0,
            total_duration_ms: 0,
            total_tokens: 0,
        }
    }

    pub fn avg_duration_ms(&self) -> u64 {
        if self.runs == 0 {
            0
        } else {
            self.total_duration_ms / self.runs
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.runs == 0 {
            0.0
        } else {
            (self.passes as f64 / self.runs as f64) * 100.0
        }
    }
}

/// T158=MicroStats
/// All stats, keyed by template ID.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct T158 {
    pub templates: HashMap<String, T157>,
}

impl Default for T158 {
    fn default() -> Self {
        Self::new()
    }
}

impl T158 {
    pub fn new() -> Self {
        T158 {
            templates: HashMap::new(),
        }
    }

    /// Load from JSON file. Returns empty stats if file doesn't exist.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| T158::new()),
            Err(_) => T158::new(),
        }
    }

    /// Save to JSON file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Record a successful run.
    pub fn record_pass(&mut self, template_id: &str, duration_ms: u64, tokens: u64) {
        let entry = self
            .templates
            .entry(template_id.to_string())
            .or_insert_with(T157::new);
        entry.runs += 1;
        entry.passes += 1;
        entry.total_duration_ms += duration_ms;
        entry.total_tokens += tokens;
    }

    /// Record a failed run (model responded but output was wrong/invalid).
    pub fn record_fail(&mut self, template_id: &str, duration_ms: u64, tokens: u64) {
        let entry = self
            .templates
            .entry(template_id.to_string())
            .or_insert_with(T157::new);
        entry.runs += 1;
        entry.failures += 1;
        entry.total_duration_ms += duration_ms;
        entry.total_tokens += tokens;
    }

    /// Record an error (model didn't respond at all).
    pub fn record_error(&mut self, template_id: &str, duration_ms: u64) {
        let entry = self
            .templates
            .entry(template_id.to_string())
            .or_insert_with(T157::new);
        entry.runs += 1;
        entry.errors += 1;
        entry.total_duration_ms += duration_ms;
    }

    /// Print stats table.
    pub fn print(&self) {
        println!("Micro-Model Statistics");
        println!("─────────────────────────────────────────────────────────────────");
        println!(
            "{:<12} {:>6} {:>6} {:>6} {:>6} {:>8} {:>8} {:>8}",
            "ID", "Runs", "Pass", "Fail", "Err", "Avg(ms)", "Tokens", "Rate%"
        );
        println!("─────────────────────────────────────────────────────────────────");

        let mut entries: Vec<_> = self.templates.iter().collect();
        entries.sort_by(|a, b| b.1.runs.cmp(&a.1.runs));

        for (id, s) in &entries {
            println!(
                "{:<12} {:>6} {:>6} {:>6} {:>6} {:>8} {:>8} {:>7.0}%",
                id,
                s.runs,
                s.passes,
                s.failures,
                s.errors,
                s.avg_duration_ms(),
                s.total_tokens,
                s.success_rate()
            );
        }

        let total_runs: u64 = entries.iter().map(|(_, s)| s.runs).sum();
        let total_passes: u64 = entries.iter().map(|(_, s)| s.passes).sum();
        println!("─────────────────────────────────────────────────────────────────");
        println!(
            "Total: {} runs, {:.0}% success rate",
            total_runs,
            if total_runs > 0 {
                (total_passes as f64 / total_runs as f64) * 100.0
            } else {
                0.0
            }
        );
    }
}

/// f246=stats_path
/// Default stats file path: ~/.kova/micro/stats.json
pub fn f246() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".kova")
        .join("micro")
        .join("stats.json")
}
