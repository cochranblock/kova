// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Temp project creation for sandbox compilation. Used by pipeline, factory, moe, academy.

use std::path::Path;

/// Write a temp Cargo project (Cargo.toml + src/main.rs or src/lib.rs).
pub fn f312(dir: &Path, code: &str, is_binary: bool) {
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .ok();
    std::fs::create_dir_all(dir.join("src")).ok();

    let file_name = if is_binary { "main.rs" } else { "lib.rs" };

    let content = if is_binary {
        code.to_string()
    } else {
        format!("#![allow(dead_code)]\n{}", code)
    };

    std::fs::write(dir.join("src").join(file_name), content).ok();
}

/// Write a validation project. Infers binary vs lib from rel_path.
pub fn f313(dir: &Path, code: &str, rel_path: &str) {
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .ok();
    std::fs::create_dir_all(dir.join("src")).ok();

    let file_name = if rel_path.contains("main") {
        "main.rs"
    } else {
        "lib.rs"
    };
    let content = if file_name == "lib.rs" {
        format!("#![allow(dead_code)]\n{}", code)
    } else {
        code.to_string()
    };
    std::fs::write(dir.join("src").join(file_name), content).ok();
}

/// Write a simple temp lib crate (always lib.rs with dead_code allowed).
pub fn f314(dir: &Path, code: &str) {
    f312(dir, code, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_temp_project_lib() {
        let tmp = tempfile::TempDir::new().unwrap();
        f312(tmp.path(), "pub fn foo() {}", false);
        assert!(tmp.path().join("Cargo.toml").exists());
        assert!(tmp.path().join("src/lib.rs").exists());
        let content = std::fs::read_to_string(tmp.path().join("src/lib.rs")).unwrap();
        assert!(content.contains("dead_code"));
        assert!(content.contains("pub fn foo()"));
    }

    #[test]
    fn write_temp_project_binary() {
        let tmp = tempfile::TempDir::new().unwrap();
        f312(tmp.path(), "fn main() {}", true);
        assert!(tmp.path().join("src/main.rs").exists());
        let content = std::fs::read_to_string(tmp.path().join("src/main.rs")).unwrap();
        assert!(!content.contains("dead_code"));
    }

    #[test]
    fn write_temp_crate_is_lib() {
        let tmp = tempfile::TempDir::new().unwrap();
        f314(tmp.path(), "pub fn bar() {}");
        assert!(tmp.path().join("src/lib.rs").exists());
    }
}
