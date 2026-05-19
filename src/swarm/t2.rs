//! T2 molecular coordinator. Takes T1 classifier outputs as an 8-float feature
//! vector and routes to code_responder (0) or prose_responder (1).
//!
//! Feature layout (T1_MODELS order × 2 values each = 8 features):
//!   [0..1] slop_detector:      class_idx/1, confidence
//!   [2..3] code_vs_english:    class_idx/1, confidence
//!   [4..5] lang_detector:      class_idx/4, confidence
//!   [6..7] intent_classifier:  class_idx/9, confidence
//!
//! f410=t2_features, f411=t2_route.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Sonnet 4.6

pub const T2_FEATURE_DIM: usize = 8;

const T1_MODELS: &[(&str, usize)] = &[
    ("slop_detector",     2),
    ("code_vs_english",   2),
    ("lang_detector",     5),
    ("intent_classifier", 10),
];

/// Routing decision from T2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    CodeResponder,
    ProseResponder,
}

impl Route {
    pub fn name(self) -> &'static str {
        match self {
            Route::CodeResponder  => "code",
            Route::ProseResponder => "prose",
        }
    }

    pub fn system_prefix(self) -> &'static str {
        match self {
            Route::CodeResponder  => CODE_PREFIX,
            Route::ProseResponder => PROSE_PREFIX,
        }
    }
}

const CODE_PREFIX: &str = "\
## Mode: code
Prioritize working code. Use code blocks. Be terse. \
Go straight to the solution — no preamble.";

const PROSE_PREFIX: &str = "\
## Mode: prose
Prioritize clarity. Use complete sentences. \
Walk through reasoning step by step before concluding.";

/// f410=t2_features. Build an 8-float T2 feature vector from T1 outputs on `text`.
pub fn f410(nb: &crate::nanobyte::Nanobyte, text: &str) -> [f32; T2_FEATURE_DIM] {
    let mut feat = [0.0f32; T2_FEATURE_DIM];
    let mut i = 0;
    for &(model, n_classes) in T1_MODELS {
        if let Ok((idx, conf)) = nb.infer(model, text) {
            feat[i]     = idx as f32 / (n_classes as f32 - 1.0).max(1.0);
            feat[i + 1] = conf;
        }
        i += 2;
    }
    feat
}

/// f411=t2_route. Extract T1 features and run the T2 coordinator model.
/// Falls back to code_vs_english signal if t2_coordinator is not in the nanobyte.
pub fn f411(nb: &crate::nanobyte::Nanobyte, text: &str) -> (Route, f32) {
    let features = f410(nb, text);
    match nb.infer_raw("t2_coordinator", &features) {
        Ok((0, conf)) => (Route::CodeResponder, conf),
        Ok((_, conf)) => (Route::ProseResponder, conf),
        Err(_) => {
            match nb.infer("code_vs_english", text) {
                Ok((1, conf)) if conf > 0.6 => (Route::CodeResponder, conf),
                _ => (Route::ProseResponder, 0.5),
            }
        }
    }
}

/// Return a system-prompt prefix for this turn, or empty string if T2 unavailable
/// or confidence is below threshold (avoids noisy routing on ambiguous inputs).
pub fn select_prompt(nb: &crate::nanobyte::Nanobyte, text: &str) -> String {
    let t2_present = nb.manifests().iter().any(|m| m.name == "t2_coordinator");
    let (route, conf) = f411(nb, text);
    if !t2_present || conf < 0.65 {
        return String::new();
    }
    route.system_prefix().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t2_features_length() {
        let nb = crate::nanobyte::starter().expect("starter.nanobyte required");
        let feat = f410(&nb, "fn main() {}");
        assert_eq!(feat.len(), T2_FEATURE_DIM);
    }

    #[test]
    fn t2_features_in_unit_range() {
        let nb = crate::nanobyte::starter().expect("starter.nanobyte required");
        let feat = f410(&nb, "explain how tokio works");
        for (i, &v) in feat.iter().enumerate() {
            assert!((0.0..=1.0).contains(&v), "feature[{i}] = {v} out of [0,1]");
        }
    }

    #[test]
    fn t2_fallback_without_coordinator() {
        let nb = crate::nanobyte::starter().expect("starter.nanobyte required");
        // starter.nanobyte has no t2_coordinator yet; fallback path must not panic.
        let (route, conf) = f411(&nb, "fn add(a: i32, b: i32) -> i32 { a + b }");
        let _ = (route, conf); // just verify it doesn't panic
    }
}
