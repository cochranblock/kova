//! Shared code gen helpers. One copy — no more duplication across factory/moe/academy/pipeline.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

/// f327=extract_rust_block. Extract first ```rust ... ``` block (or bare ```) from LLM output.
pub fn f327(s: &str) -> Option<String> {
    crate::cargo::f309(s)
}

/// Build system prompt for code generation (binary vs lib).
pub fn f311(wants_binary: bool) -> String {
    crate::cargo::f311(wants_binary)
}

/// Detect if prompt asks for a binary.
pub fn f310(prompt: &str) -> bool {
    crate::cargo::f310(prompt)
}

/// f330=truncate. Truncate string.
pub fn f330(s: &str, max: usize) -> String {
    crate::cargo::f308(s, max)
}