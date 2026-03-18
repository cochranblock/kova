// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! CodeGenStrategy trait — composable orchestration.
//! T181, MoE, Academy, Gauntlet become impl CodeGenStrategy over shared CodeGenInfra.

/// Shared config for all code gen strategies.
#[derive(Debug, Clone)]
pub struct T211 {
    pub max_fix_retries: u32,
    pub run_clippy: bool,
    pub run_tests: bool,
    pub run_review: bool,
    pub num_ctx: u32,
}

impl Default for T211 {
    fn default() -> Self {
        Self {
            max_fix_retries: crate::config::orchestration_max_fix_retries(),
            run_clippy: crate::config::orchestration_run_clippy(),
            run_tests: true,
            run_review: true,
            num_ctx: 8192,
        }
    }
}
