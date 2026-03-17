// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Shared code gen helpers. One copy — no more duplication across factory/moe/academy/pipeline.

/// Extract first ```rust ... ``` block (or bare ```) from LLM output.
pub fn extract_rust_block(s: &str) -> Option<String> {
    crate::cargo::extract_rust_block(s)
}

/// Build system prompt for code generation (binary vs lib).
pub fn build_system_prompt(wants_binary: bool) -> String {
    crate::cargo::build_system_prompt(wants_binary)
}

/// Detect if prompt asks for a binary.
pub fn prompt_wants_binary(prompt: &str) -> bool {
    crate::cargo::prompt_wants_binary(prompt)
}

/// Truncate string.
pub fn truncate(s: &str, max: usize) -> String {
    crate::cargo::truncate(s, max)
}
