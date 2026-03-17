// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! f91–f93. Cargo check, clippy, test. Delegates to crate::cargo.

use std::path::Path;

/// f91=cargo_check. Returns (success, stderr).
pub fn cargo_check(project_dir: &Path) -> (bool, String) {
    crate::cargo::cargo_check(project_dir)
}

/// f92=cargo_clippy. Returns (success, stderr).
pub fn cargo_clippy(project_dir: &Path) -> (bool, String) {
    crate::cargo::cargo_clippy(project_dir)
}

/// f93=cargo_test. Returns (success, stderr).
pub fn cargo_test(project_dir: &Path) -> (bool, String) {
    crate::cargo::cargo_test(project_dir)
}

