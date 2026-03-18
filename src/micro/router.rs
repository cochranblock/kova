//! router — Route incoming requests to the correct micro-model.
//! Uses epsilon-greedy bandit for learned routing (Mattbusel/tokio-prompt-orchestrator).
//! Falls back to keyword matching when no history exists.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::collections::HashMap;
use std::sync::Mutex;

use super::registry::T149;

/// T150=RouteDecision
/// Routing decision: which micro-model handles this input.
#[derive(Debug, Clone)]
pub struct T150 {
    /// Template ID selected.
    pub template_id: String,
    /// Confidence (0.0-1.0). Higher = more certain.
    pub confidence: f32,
    /// How the decision was made.
    pub method: T151,
}

/// T151=RouteMethod
#[derive(Debug, Clone)]
pub enum T151 {
    /// Keyword match (no history).
    Keyword,
    /// Epsilon-greedy bandit (learned from outcomes).
    Bandit,
    /// Explicit: caller specified the template ID.
    Explicit,
}

/// T152=RouteOutcome
/// Outcome of a micro-model run, fed back to the router for learning.
#[derive(Debug, Clone)]
pub struct T152 {
    pub template_id: String,
    /// 0.0 = total failure, 1.0 = perfect.
    pub reward: f32,
}

/// Per-category reward history: category -> template_id -> (total_reward, count).
type CatHistory = HashMap<String, HashMap<String, (f32, u32)>>;

/// T153=MicroRouter
/// Learned router state. Epsilon-greedy bandit over template selection.
/// Inspired by Mattbusel/tokio-prompt-orchestrator's epsilon-greedy routing.
pub struct T153 {
    history: Mutex<CatHistory>,
    /// Exploration rate (0.0 = pure exploit, 1.0 = pure explore).
    pub epsilon: f32,
}

impl Default for T153 {
    fn default() -> Self {
        Self::new()
    }
}

impl T153 {
    pub fn new() -> Self {
        T153 {
            history: Mutex::new(HashMap::new()),
            epsilon: 0.1,
        }
    }

    /// Route an input to the best micro-model.
    /// If `explicit_id` is Some, skip routing and use that template directly.
    pub fn route(
        &self,
        input: &str,
        registry: &T149,
        explicit_id: Option<&str>,
    ) -> T150 {
        // Explicit override
        if let Some(id) = explicit_id && registry.get(id).is_some() {
            return T150 {
                    template_id: id.to_string(),
                    confidence: 1.0,
                    method: T151::Explicit,
                };
        }

        // Classify the input into a category via keywords
        let category = f242(input);

        // Check bandit history for this category
        let history = self.history.lock().unwrap();
        if let Some(cat_history) = history.get(&category)
            && !cat_history.is_empty()
            && rand_f32() > self.epsilon
        {
                // Exploit: pick the template with highest average reward
                let best = cat_history
                    .iter()
                    .filter(|(id, _)| registry.get(id).is_some())
                    .max_by(|(_, (r1, c1)), (_, (r2, c2))| {
                        let avg1 = r1 / (*c1 as f32).max(1.0);
                        let avg2 = r2 / (*c2 as f32).max(1.0);
                        avg1.partial_cmp(&avg2).unwrap_or(std::cmp::Ordering::Equal)
                    });

                if let Some((id, (total, count))) = best {
                    return T150 {
                        template_id: id.clone(),
                        confidence: total / (*count as f32).max(1.0),
                        method: T151::Bandit,
                    };
                }
        }
        drop(history);

        // Keyword fallback: map category to default template
        let template_id = f243(&category);
        T150 {
            template_id,
            confidence: 0.5,
            method: T151::Keyword,
        }
    }

    /// Record the outcome of a micro-model run for future routing.
    pub fn record(&self, category: &str, outcome: T152) {
        let mut history = self.history.lock().unwrap();
        let cat = history.entry(category.to_string()).or_default();
        let entry = cat.entry(outcome.template_id).or_insert((0.0, 0));
        entry.0 += outcome.reward;
        entry.1 += 1;
    }

    /// Get average reward for a template in a category.
    pub fn avg_reward(&self, category: &str, template_id: &str) -> Option<f32> {
        let history = self.history.lock().unwrap();
        history
            .get(category)
            .and_then(|cat| cat.get(template_id))
            .map(|(total, count)| total / (*count as f32).max(1.0))
    }
}

/// f242=classify_keywords
/// Classify input into a category via keyword matching.
pub fn f242(input: &str) -> String {
    let lower = input.to_lowercase();

    if lower.contains("fix") || lower.contains("compile") || lower.contains("error") {
        "fix_compile".into()
    } else if lower.contains("clippy") || lower.contains("warning") || lower.contains("lint") {
        "clippy_fix".into()
    } else if lower.contains("test") || lower.contains("unit test") {
        "test_write".into()
    } else if lower.contains("review") || lower.contains("check") || lower.contains("audit") {
        "code_review".into()
    } else if lower.contains("explain") || lower.contains("trace") || lower.contains("what") {
        "explain".into()
    } else if lower.contains("generate")
        || lower.contains("write")
        || lower.contains("create")
        || lower.contains("build")
        || lower.contains("add")
        || lower.contains("implement")
    {
        "code_gen".into()
    } else {
        "general".into()
    }
}

/// f243=category_to_template
/// Map a category name to its default template ID.
pub fn f243(category: &str) -> String {
    match category {
        "fix_compile" => "f81".into(),
        "clippy_fix" => "f_clippy_fix".into(),
        "test_write" => "f_test_write".into(),
        "code_review" => "f_code_review".into(),
        "explain" => "f115".into(),
        "code_gen" => "f80".into(),
        _ => "f79".into(), // classify first, then re-route
    }
}

/// Cheap pseudo-random f32 in [0, 1) — no external crate needed.
fn rand_f32() -> f32 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 10000) as f32 / 10000.0
}