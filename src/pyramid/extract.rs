// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6

//! extract — Training data extraction from the 15-repo portfolio.
//! Walks all repos, extracts patterns by category, outputs training pairs.
//! Each expert gets trained on its own slice of the codebase.

use std::path::{Path, PathBuf};

/// Training pair: input context → expected output code.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrainingPair {
    pub expert: String,
    pub input: String,
    pub output: String,
    pub source_file: String,
    pub source_repo: String,
}

/// Extract training data from all repos in a directory.
pub fn extract_all(base_dir: &Path) -> Vec<TrainingPair> {
    let repos = [
        "kova", "cochranblock", "approuter", "oakilydokily", "rogue-repo",
        "ronin-sites", "pixel-forge", "exopack", "any-gpu", "whyyoulying",
        "ghost-fabric", "call-shield", "pocket-server", "wowasticker",
        "provenance-docs",
    ];

    let mut pairs = Vec::new();

    for repo in &repos {
        let repo_dir = base_dir.join(repo);
        if !repo_dir.exists() {
            continue;
        }

        // Cargo.toml → cargo-gen training
        let cargo_path = repo_dir.join("Cargo.toml");
        if cargo_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_path) {
                pairs.push(TrainingPair {
                    expert: "CargoToml".to_string(),
                    input: format!("Generate Cargo.toml for project '{}'", repo),
                    output: content,
                    source_file: "Cargo.toml".to_string(),
                    source_repo: repo.to_string(),
                });
            }
        }

        // Walk src/ for Rust files
        let src_dir = repo_dir.join("src");
        if !src_dir.exists() {
            continue;
        }

        let walker = walkdir(&src_dir);
        for file_path in walker {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                let rel = file_path.strip_prefix(&repo_dir).unwrap_or(&file_path);
                let pairs_for_file = categorize_content(&content, rel, repo);
                pairs.extend(pairs_for_file);
            }
        }
    }

    println!("[extract] {} training pairs from {} repos", pairs.len(), repos.len());
    for expert in unique_experts(&pairs) {
        let count = pairs.iter().filter(|p| p.expert == expert).count();
        println!("[extract]   {}: {} pairs", expert, count);
    }

    pairs
}

/// Walk a directory for .rs files, skip target/.
fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_str().unwrap_or("");
                if name != "target" && name != "vendor" && name != ".git" {
                    files.extend(walkdir(&path));
                }
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                files.push(path);
            }
        }
    }
    files
}

/// Categorize a file's content into training pairs for specific experts.
fn categorize_content(content: &str, rel_path: &Path, repo: &str) -> Vec<TrainingPair> {
    let mut pairs = Vec::new();
    let file_str = rel_path.to_str().unwrap_or("");

    // Struct definitions → StructDef expert
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
            // Extract the full struct block
            if let Some(block) = extract_block(content, trimmed) {
                pairs.push(TrainingPair {
                    expert: "StructDef".to_string(),
                    input: format!("Define struct from {}/{}", repo, file_str),
                    output: block,
                    source_file: file_str.to_string(),
                    source_repo: repo.to_string(),
                });
            }
        }

        // Enum definitions → EnumDef expert
        if trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") {
            if let Some(block) = extract_block(content, trimmed) {
                let expert = if content.contains("thiserror") && block.contains("#[error") {
                    "ThiserrorEnum"
                } else {
                    "EnumDef"
                };
                pairs.push(TrainingPair {
                    expert: expert.to_string(),
                    input: format!("Define enum from {}/{}", repo, file_str),
                    output: block,
                    source_file: file_str.to_string(),
                    source_repo: repo.to_string(),
                });
            }
        }

        // Test functions → TestUnit expert
        if trimmed == "#[test]" {
            // Next function is a test
            if let Some(block) = extract_next_fn(content, trimmed) {
                pairs.push(TrainingPair {
                    expert: "TestUnit".to_string(),
                    input: format!("Write test from {}/{}", repo, file_str),
                    output: block,
                    source_file: file_str.to_string(),
                    source_repo: repo.to_string(),
                });
            }
        }
    }

    // Axum route handlers → AxumHandler expert
    if content.contains("axum") && content.contains("async fn") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pub async fn") && trimmed.contains("State(") {
                if let Some(block) = extract_block(content, trimmed) {
                    pairs.push(TrainingPair {
                        expert: "AxumHandler".to_string(),
                        input: format!("Axum handler from {}/{}", repo, file_str),
                        output: block,
                        source_file: file_str.to_string(),
                        source_repo: repo.to_string(),
                    });
                }
            }
        }
    }

    // Sled operations → SledRead/SledWrite
    if content.contains("sled") {
        if content.contains(".get(") || content.contains(".scan_prefix") {
            pairs.push(TrainingPair {
                expert: "SledRead".to_string(),
                input: format!("Sled read pattern from {}/{}", repo, file_str),
                output: content.to_string(),
                source_file: file_str.to_string(),
                source_repo: repo.to_string(),
            });
        }
        if content.contains(".insert(") || content.contains(".remove(") {
            pairs.push(TrainingPair {
                expert: "SledWrite".to_string(),
                input: format!("Sled write pattern from {}/{}", repo, file_str),
                output: content.to_string(),
                source_file: file_str.to_string(),
                source_repo: repo.to_string(),
            });
        }
    }

    // Clap parser → ClapParser expert
    if content.contains("clap") && content.contains("Parser") {
        pairs.push(TrainingPair {
            expert: "ClapParser".to_string(),
            input: format!("Clap CLI from {}/{}", repo, file_str),
            output: content.to_string(),
            source_file: file_str.to_string(),
            source_repo: repo.to_string(),
        });
    }

    pairs
}

/// Extract a code block starting from a line containing `start_marker`.
fn extract_block(content: &str, start_marker: &str) -> Option<String> {
    let idx = content.find(start_marker)?;
    let from = &content[idx..];
    let mut depth = 0;
    let mut end = 0;
    for (i, ch) in from.char_indices() {
        if ch == '{' { depth += 1; }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                end = i + 1;
                break;
            }
        }
    }
    if end > 0 {
        Some(from[..end].to_string())
    } else {
        None
    }
}

/// Extract the next function after a marker (e.g., #[test]).
fn extract_next_fn(content: &str, marker: &str) -> Option<String> {
    let idx = content.find(marker)?;
    let after = &content[idx..];
    // Find the fn keyword after the marker
    let fn_idx = after.find("fn ")?;
    let from = &after[fn_idx..];
    extract_block(from, "fn ")
}

fn unique_experts(pairs: &[TrainingPair]) -> Vec<String> {
    let mut experts: Vec<String> = pairs.iter().map(|p| p.expert.clone()).collect();
    experts.sort();
    experts.dedup();
    experts
}
