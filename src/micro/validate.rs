// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, Mattbusel (hallucination detection)
//! validate — Multi-method output validation for micro-model responses.
//! Inspired by Mattbusel/LLM-Hallucination-Detection-Script's multi-method approach:
//! confidence patterns, factual density, coherence scoring, contradiction detection.

use super::runner::T154;

/// T173=ValidationResult
/// Validation verdict.
#[derive(Debug, Clone)]
pub struct T173 {
    /// Overall pass/fail.
    pub passed: bool,
    /// Individual check results.
    pub checks: Vec<T174>,
    /// Overall confidence (0.0-1.0).
    pub confidence: f32,
    /// One-line summary.
    pub summary: String,
}

/// T174=ValidationCheck
/// A single validation check.
#[derive(Debug, Clone)]
pub struct T174 {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

/// Validate a micro-model result using multiple methods.
/// Inspired by Mattbusel/LLM-Hallucination-Detection-Script:
/// - Completeness check: no TODO/unimplemented/placeholder markers
/// - Confidence patterns: detect hedging language that signals low confidence
/// - Coherence: response should relate to the input (basic overlap check)
/// - Format: response should match expected output schema
/// f263=validate
pub fn f263(result: &T154, input: &str, expected_format: &str) -> T173 {
    let mut checks = Vec::new();
    let response = &result.response;

    // 1. Completeness — no placeholders
    let completeness = check_completeness(response);
    checks.push(completeness);

    // 2. Confidence patterns — detect hedging
    let confidence = check_confidence_patterns(response);
    checks.push(confidence);

    // 3. Coherence — response relates to input
    let coherence = check_coherence(response, input);
    checks.push(coherence);

    // 4. Format — matches expected output pattern
    let format = check_format(response, expected_format);
    checks.push(format);

    // 5. Non-empty
    let nonempty = T174 {
        name: "non_empty".into(),
        passed: !response.trim().is_empty(),
        detail: if response.trim().is_empty() {
            "empty response".into()
        } else {
            format!("{} chars", response.len())
        },
    };
    checks.push(nonempty);

    // Aggregate
    let passed_count = checks.iter().filter(|c| c.passed).count();
    let total = checks.len();
    let confidence_score = passed_count as f32 / total as f32;
    let all_passed = checks.iter().all(|c| c.passed);

    let summary = if all_passed {
        "PASS — all checks passed".into()
    } else {
        let failed: Vec<_> = checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| c.name.as_str())
            .collect();
        format!("FAIL — failed: {}", failed.join(", "))
    };

    T173 {
        passed: all_passed,
        checks,
        confidence: confidence_score,
        summary,
    }
}

/// Check for placeholder/incomplete markers.
fn check_completeness(response: &str) -> T174 {
    let markers = [
        "todo!()",
        "unimplemented!()",
        "todo!(\"",
        "// TODO",
        "/* TODO",
        "PLACEHOLDER",
        "FIXME",
        "...",
    ];

    let found: Vec<&&str> = markers.iter().filter(|m| response.contains(**m)).collect();

    T174 {
        name: "completeness".into(),
        passed: found.is_empty(),
        detail: if found.is_empty() {
            "no placeholder markers".into()
        } else {
            format!(
                "found: {}",
                found.iter().map(|m| **m).collect::<Vec<_>>().join(", ")
            )
        },
    }
}

/// Detect hedging language that signals low model confidence.
/// From Mattbusel's confidence pattern detection.
fn check_confidence_patterns(response: &str) -> T174 {
    let hedges = [
        "i'm not sure",
        "i think",
        "maybe",
        "perhaps",
        "it might",
        "possibly",
        "i believe",
        "i cannot",
        "i can't determine",
    ];

    let lower = response.to_lowercase();
    let found: Vec<&&str> = hedges.iter().filter(|h| lower.contains(**h)).collect();

    T174 {
        name: "confidence".into(),
        passed: found.is_empty(),
        detail: if found.is_empty() {
            "no hedging language".into()
        } else {
            format!(
                "hedging detected: {}",
                found.iter().map(|h| **h).collect::<Vec<_>>().join(", ")
            )
        },
    }
}

/// Basic coherence: check that response shares some terms with input.
fn check_coherence(response: &str, input: &str) -> T174 {
    let input_words: std::collections::HashSet<&str> = input
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 3)
        .collect();

    let response_lower = response.to_lowercase();
    let overlap = input_words
        .iter()
        .filter(|w| response_lower.contains(&w.to_lowercase()))
        .count();

    let ratio = if input_words.is_empty() {
        1.0
    } else {
        overlap as f32 / input_words.len() as f32
    };

    T174 {
        name: "coherence".into(),
        passed: ratio > 0.1 || input_words.len() < 3,
        detail: format!(
            "{:.0}% term overlap ({}/{})",
            ratio * 100.0,
            overlap,
            input_words.len()
        ),
    }
}

/// Check if response matches expected format hints.
fn check_format(response: &str, expected: &str) -> T174 {
    if expected.is_empty() {
        return T174 {
            name: "format".into(),
            passed: true,
            detail: "no format constraint".into(),
        };
    }

    let lower_expected = expected.to_lowercase();
    let passed = if lower_expected.contains("```rust") {
        response.contains("```rust") || response.contains("fn ")
    } else if lower_expected.contains("pass") || lower_expected.contains("fail") {
        let upper = response.to_uppercase();
        upper.contains("PASS") || upper.contains("FAIL") || upper.contains("LGTM")
    } else if lower_expected.contains("category") || lower_expected.contains("single") {
        // Single-word response expected
        response.split_whitespace().count() <= 3
    } else {
        true
    };

    T174 {
        name: "format".into(),
        passed,
        detail: if passed {
            "matches expected format".into()
        } else {
            format!("expected format: {}", expected)
        },
    }
}

/// f264=quick_validate
/// Quick pass/fail validation — just checks if the response looks valid.
pub fn f264(response: &str) -> bool {
    !response.trim().is_empty()
        && !response.contains("todo!()")
        && !response.contains("unimplemented!()")
}
