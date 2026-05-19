#![allow(non_camel_case_types)]
//! Intent layer. t0=Intent t1=IntentKind t2=Constraint. No I/O. WASM-safe.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use serde::{Deserialize, Serialize};

/// t0 = Intent. Structured intent: what user wants, separate from how.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct t0 {
    pub s0: t1,
    pub s1: Option<String>,
    pub s2: Vec<t2>,
}

/// t1 = IntentKind
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum t1 {
    Compile {
        release: bool,
        check_only: bool,
    },
    Test,
    FixWarnings,
    FullPipeline,
    TunnelUpdate,
    SetupRoguerepo,
    CloudflarePurge,
    Custom {
        cmd: String,
        args: Vec<String>,
    },
}

/// t2 = Constraint. Deontic: must, may, must-not.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum t2 {
    MustNotBreakTests,
    MayUseGpu,
    TimeoutSecs(u64),
    MaxJobs(usize),
}

impl t0 {
    pub fn f18(p4: bool) -> Self {
        Self {
            s0: t1::Compile {
                release: p4,
                check_only: false,
            },
            s1: None,
            s2: vec![t2::MustNotBreakTests],
        }
    }

    pub fn f19() -> Self {
        Self {
            s0: t1::Test,
            s1: None,
            s2: vec![t2::MustNotBreakTests],
        }
    }

    pub fn f20() -> Self {
        Self {
            s0: t1::FullPipeline,
            s1: None,
            s2: vec![t2::MustNotBreakTests, t2::MayUseGpu],
        }
    }

    pub fn f21() -> Self {
        Self {
            s0: t1::TunnelUpdate,
            s1: None,
            s2: vec![],
        }
    }

    pub fn f22() -> Self {
        Self {
            s0: t1::SetupRoguerepo,
            s1: None,
            s2: vec![],
        }
    }

    pub fn f23() -> Self {
        Self {
            s0: t1::CloudflarePurge,
            s1: None,
            s2: vec![],
        }
    }
}

/// f62 = parse_intent. Map keywords to intent. No LLM.
pub fn f62(input: &str) -> Option<t0> {
    let lower = input.to_lowercase();
    let trimmed = lower.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains("full") || trimmed.contains("pipeline") || trimmed.contains("run pipeline") {
        return Some(t0::f20());
    }
    if trimmed.contains("tunnel") || trimmed.contains("update tunnel") {
        return Some(t0::f21());
    }
    if trimmed.contains("setup") && (trimmed.contains("rogue") || trimmed.contains("repo")) {
        return Some(t0::f22());
    }
    if trimmed.contains("cloudflare") || trimmed.contains("purge") || trimmed.contains("cache") {
        return Some(t0::f23());
    }
    if trimmed.contains("test") && !trimmed.contains("full") {
        return Some(t0::f19());
    }
    if trimmed.contains("compile") || trimmed.contains("build") {
        let release = trimmed.contains("release");
        return Some(t0::f18(release));
    }
    if trimmed.contains("fix") && (trimmed.contains("warn") || trimmed.contains("warning")) {
        return Some(t0 {
            s0: t1::FixWarnings,
            s1: None,
            s2: vec![t2::MustNotBreakTests],
        });
    }
    None
}

/// Intent name for display (e.g. "full-pipeline").
pub fn f325(i: &t1) -> &'static str {
    match i {
        t1::Compile { .. } => "compile",
        t1::Test => "test",
        t1::FixWarnings => "fix-warnings",
        t1::FullPipeline => "full-pipeline",
        t1::TunnelUpdate => "tunnel-update",
        t1::SetupRoguerepo => "setup-roguerepo",
        t1::CloudflarePurge => "cloudflare-purge",
        t1::Custom { .. } => "custom",
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_intent_empty_returns_none() {
        assert!(f62("").is_none());
        assert!(f62("   ").is_none());
    }

    #[test]
    fn parse_intent_unrecognised_returns_none() {
        assert!(f62("hello world").is_none());
        assert!(f62("do something").is_none());
    }

    #[test]
    fn parse_intent_full_pipeline() {
        let i = f62("run full pipeline").unwrap();
        assert!(matches!(i.s0, t1::FullPipeline));
        let i = f62("run pipeline now").unwrap();
        assert!(matches!(i.s0, t1::FullPipeline));
        let i = f62("full build").unwrap();
        assert!(matches!(i.s0, t1::FullPipeline));
    }

    #[test]
    fn parse_intent_test() {
        let i = f62("run tests").unwrap();
        assert!(matches!(i.s0, t1::Test));
        let i = f62("cargo test").unwrap();
        assert!(matches!(i.s0, t1::Test));
    }

    #[test]
    fn parse_intent_compile_debug() {
        let i = f62("compile").unwrap();
        assert!(matches!(i.s0, t1::Compile { release: false, .. }));
        let i = f62("build").unwrap();
        assert!(matches!(i.s0, t1::Compile { release: false, .. }));
    }

    #[test]
    fn parse_intent_compile_release() {
        let i = f62("build release").unwrap();
        assert!(matches!(i.s0, t1::Compile { release: true, .. }));
        let i = f62("compile release").unwrap();
        assert!(matches!(i.s0, t1::Compile { release: true, .. }));
    }

    #[test]
    fn parse_intent_fix_warnings() {
        let i = f62("fix warnings").unwrap();
        assert!(matches!(i.s0, t1::FixWarnings));
        let i = f62("fix warning").unwrap();
        assert!(matches!(i.s0, t1::FixWarnings));
    }

    #[test]
    fn parse_intent_tunnel() {
        let i = f62("update tunnel").unwrap();
        assert!(matches!(i.s0, t1::TunnelUpdate));
        let i = f62("tunnel").unwrap();
        assert!(matches!(i.s0, t1::TunnelUpdate));
    }

    #[test]
    fn parse_intent_case_insensitive() {
        let i = f62("COMPILE").unwrap();
        assert!(matches!(i.s0, t1::Compile { .. }));
        assert!(f62("BUILD RELEASE").map(|i| matches!(i.s0, t1::Compile { release: true, .. })).unwrap_or(false));
        let i = f62("RUN TESTS").unwrap();
        assert!(matches!(i.s0, t1::Test));
    }

    #[test]
    fn intent_name_all_variants() {
        assert_eq!(f325(&t1::Compile { release: true, check_only: false }), "compile");
        assert_eq!(f325(&t1::Test),                 "test");
        assert_eq!(f325(&t1::FixWarnings),           "fix-warnings");
        assert_eq!(f325(&t1::FullPipeline),          "full-pipeline");
        assert_eq!(f325(&t1::TunnelUpdate),          "tunnel-update");
        assert_eq!(f325(&t1::SetupRoguerepo),        "setup-roguerepo");
        assert_eq!(f325(&t1::CloudflarePurge),       "cloudflare-purge");
        assert_eq!(f325(&t1::Custom { cmd: "x".into(), args: vec![] }), "custom");
    }
}
