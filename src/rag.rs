// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! RAG — Retrieval-Augmented Generation. Sled-backed vector store + fastembed.
//! Embeds code chunks locally, stores in sled, retrieves via cosine similarity.
//! Research: vectorize-io (chunking strategy, RAG pipeline design), fastembed-rs, SahomeDB (sled vector patterns).

use std::path::{Path, PathBuf};

use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};

// ── Types ────────────────────────────────────────────────────────

/// A chunk of code or text with its embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Source file path.
    pub file: String,
    /// Line range (start, end) in the source file.
    pub lines: (usize, usize),
    /// Raw text content of the chunk.
    pub text: String,
    /// Vector embedding (384-dim for BGE-small).
    pub embedding: Vec<f32>,
}

/// Search result: a chunk with its similarity score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk: Chunk,
    pub score: f32,
}

/// Stats about the vector store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagStats {
    pub total_chunks: usize,
    pub total_files: usize,
    pub embedding_dim: usize,
}

// ── Embedding ────────────────────────────────────────────────────

/// Generate embeddings for a batch of texts. Uses BGE-small-en (384-dim).
/// First call downloads the model (~30MB), then cached locally.
pub fn embed_texts(texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

    let mut model = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true),
    )?;

    // BGE models want "passage: " prefix for documents
    let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {}", t)).collect();
    let embeddings = model.embed(prefixed, None)?;
    Ok(embeddings)
}

/// Generate a query embedding. Uses "query: " prefix per BGE spec.
pub fn embed_query(query: &str) -> anyhow::Result<Vec<f32>> {
    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

    let mut model = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(false),
    )?;

    let results = model.embed(vec![format!("query: {}", query)], None)?;
    results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no embedding returned"))
}

// ── Vector Store (sled-backed) ───────────────────────────────────

/// Sled-backed vector store. Each chunk is serialized with its embedding.
/// Retrieval is brute-force cosine similarity (fast enough for <100K chunks).
pub struct VectorStore {
    _db: sled::Db,
    tree: sled::Tree,
}

impl VectorStore {
    /// Open or create a vector store at the given path.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let db = sled::open(path)?;
        let tree = db.open_tree("rag_chunks")?;
        Ok(Self { _db: db, tree })
    }

    /// Default store path: ~/.kova/rag/vectors
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".kova").join("rag").join("vectors")
    }

    /// Insert a chunk. Key = file:start_line.
    pub fn insert(&self, chunk: &Chunk) -> anyhow::Result<()> {
        let key = format!("{}:{}", chunk.file, chunk.lines.0);
        let value = serde_json::to_vec(chunk)?;
        self.tree.insert(key.as_bytes(), value)?;
        Ok(())
    }

    /// Insert many chunks (batch).
    pub fn insert_many(&self, chunks: &[Chunk]) -> anyhow::Result<usize> {
        let mut count = 0;
        for chunk in chunks {
            self.insert(chunk)?;
            count += 1;
        }
        self.tree.flush()?;
        Ok(count)
    }

    /// Search for the top-k most similar chunks to the query embedding.
    pub fn search(&self, query_embedding: &[f32], k: usize) -> anyhow::Result<Vec<SearchResult>> {
        let mut results: Vec<(OrderedFloat<f32>, Chunk)> = Vec::new();

        for entry in self.tree.iter() {
            let (_, value) = entry?;
            let chunk: Chunk = serde_json::from_slice(&value)?;
            let score = cosine_similarity(query_embedding, &chunk.embedding);
            results.push((OrderedFloat(score), chunk));
        }

        // Sort descending by similarity
        results.sort_by(|a, b| b.0.cmp(&a.0));
        results.truncate(k);

        Ok(results
            .into_iter()
            .map(|(score, chunk)| SearchResult {
                chunk,
                score: score.into_inner(),
            })
            .collect())
    }

    /// Remove all chunks for a given file (re-index).
    pub fn remove_file(&self, file: &str) -> anyhow::Result<usize> {
        let prefix = format!("{}:", file);
        let mut removed = 0;
        for entry in self.tree.scan_prefix(prefix.as_bytes()) {
            let (key, _) = entry?;
            self.tree.remove(key)?;
            removed += 1;
        }
        Ok(removed)
    }

    /// Clear the entire store.
    pub fn clear(&self) -> anyhow::Result<()> {
        self.tree.clear()?;
        self.tree.flush()?;
        Ok(())
    }

    /// Get stats.
    pub fn stats(&self) -> anyhow::Result<RagStats> {
        let mut total_chunks = 0;
        let mut files = std::collections::HashSet::new();
        let mut dim = 0;

        for entry in self.tree.iter() {
            let (_, value) = entry?;
            let chunk: Chunk = serde_json::from_slice(&value)?;
            total_chunks += 1;
            files.insert(chunk.file.clone());
            if dim == 0 {
                dim = chunk.embedding.len();
            }
        }

        Ok(RagStats {
            total_chunks,
            total_files: files.len(),
            embedding_dim: dim,
        })
    }
}

// ── Cosine Similarity ────────────────────────────────────────────

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

// ── Code Chunking ────────────────────────────────────────────────

/// Chunk a Rust source file into logical blocks.
/// Strategy: split on function/struct/impl/mod boundaries.
/// Fallback: sliding window of ~50 lines with 25% overlap.
pub fn chunk_rust_file(_file_path: &str, content: &str) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current_start = 0;
    let mut brace_depth: i32 = 0;
    let mut in_block = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Detect block starts: fn, struct, enum, impl, mod, trait
        let is_block_start = !in_block
            && (trimmed.starts_with("pub fn ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("pub mod ")
                || trimmed.starts_with("mod ")
                || trimmed.starts_with("pub trait ")
                || trimmed.starts_with("trait ")
                || trimmed.starts_with("#[cfg(test)]")
                || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("async fn "));

        if is_block_start {
            // Save any preceding non-block lines as a chunk
            if i > current_start && !in_block {
                let text = lines[current_start..i].join("\n");
                if text.trim().len() > 20 {
                    chunks.push((current_start + 1, i, text));
                }
            }
            current_start = i;
            in_block = true;
            brace_depth = 0;
        }

        // Track brace depth
        for ch in trimmed.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                _ => {}
            }
        }

        // Block ends when braces balance
        if in_block && brace_depth == 0 && trimmed.contains('}') {
            let text = lines[current_start..=i].join("\n");
            chunks.push((current_start + 1, i + 1, text));
            current_start = i + 1;
            in_block = false;
        }
    }

    // Remaining lines
    if current_start < lines.len() {
        let text = lines[current_start..].join("\n");
        if text.trim().len() > 20 {
            chunks.push((current_start + 1, lines.len(), text));
        }
    }

    // If no blocks found (non-Rust file), use sliding window
    if chunks.is_empty() {
        return chunk_sliding_window(&lines, 50, 12);
    }

    // Split oversized chunks (>100 lines) into sub-chunks
    let mut final_chunks = Vec::new();
    for (start, end, text) in chunks {
        let chunk_lines: Vec<&str> = text.lines().collect();
        if chunk_lines.len() > 100 {
            let sub = chunk_sliding_window(&chunk_lines, 50, 12);
            for (s, e, t) in sub {
                final_chunks.push((start + s - 1, start + e - 1, t));
            }
        } else {
            final_chunks.push((start, end, text));
        }
    }

    final_chunks
}

/// Sliding window chunking: window_size lines with overlap.
fn chunk_sliding_window(lines: &[&str], window: usize, overlap: usize) -> Vec<(usize, usize, String)> {
    let mut chunks = Vec::new();
    let step = window.saturating_sub(overlap).max(1);
    let mut i = 0;
    while i < lines.len() {
        let end = (i + window).min(lines.len());
        let text = lines[i..end].join("\n");
        if text.trim().len() > 20 {
            chunks.push((i + 1, end, text));
        }
        i += step;
    }
    chunks
}

// ── Index a project ──────────────────────────────────────────────

/// Index all Rust files in a directory. Returns number of chunks indexed.
pub fn index_directory(store: &VectorStore, dir: &Path) -> anyhow::Result<usize> {
    let mut all_chunks = Vec::new();

    // Walk directory for .rs files
    let pattern = dir.join("**/*.rs");
    let pattern_str = pattern.to_string_lossy();
    for entry in glob::glob(&pattern_str).map_err(|e| anyhow::anyhow!("glob: {}", e))? {
        let path = entry.map_err(|e| anyhow::anyhow!("glob entry: {}", e))?;

        // Skip target/ and hidden dirs
        let path_str = path.to_string_lossy();
        if path_str.contains("/target/") || path_str.contains("/.") {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let file_str = path.to_string_lossy().to_string();

        // Remove old chunks for this file
        let _ = store.remove_file(&file_str);

        let file_chunks = chunk_rust_file(&file_str, &content);
        for (start, end, text) in file_chunks {
            all_chunks.push((file_str.clone(), start, end, text));
        }
    }

    if all_chunks.is_empty() {
        return Ok(0);
    }

    // Batch embed all chunk texts
    eprintln!("[rag] embedding {} chunks...", all_chunks.len());
    let texts: Vec<String> = all_chunks.iter().map(|(_, _, _, t)| t.clone()).collect();

    // Process in batches of 256 (fastembed default)
    let mut embedded_chunks = Vec::new();
    for batch_start in (0..texts.len()).step_by(256) {
        let batch_end = (batch_start + 256).min(texts.len());
        let batch = &texts[batch_start..batch_end];
        let embeddings = embed_texts(&batch.to_vec())?;

        for (i, emb) in embeddings.into_iter().enumerate() {
            let idx = batch_start + i;
            let (ref file, start, end, ref text) = all_chunks[idx];
            embedded_chunks.push(Chunk {
                file: file.clone(),
                lines: (start, end),
                text: text.clone(),
                embedding: emb,
            });
        }
    }

    let count = store.insert_many(&embedded_chunks)?;
    eprintln!("[rag] indexed {} chunks", count);
    Ok(count)
}

/// Search the store with a natural language query.
pub fn search(store: &VectorStore, query: &str, k: usize) -> anyhow::Result<Vec<SearchResult>> {
    let query_emb = embed_query(query)?;
    store.search(&query_emb, k)
}

/// Format search results as context for LLM injection.
pub fn format_context(results: &[SearchResult], max_tokens: usize) -> String {
    let mut out = String::new();
    let mut token_est = 0;

    for (i, r) in results.iter().enumerate() {
        let header = format!(
            "--- {} (lines {}-{}, score: {:.3}) ---\n",
            r.chunk.file, r.chunk.lines.0, r.chunk.lines.1, r.score
        );
        let entry = format!("{}{}\n\n", header, r.chunk.text);
        let est = entry.len() / 4; // rough token estimate

        if token_est + est > max_tokens && i > 0 {
            break;
        }

        out.push_str(&entry);
        token_est += est;
    }

    out
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 0.001);
    }

    #[test]
    fn chunk_rust_basic() {
        let code = r#"
use std::path::Path;

fn hello() {
    println!("hello");
}

pub struct Foo {
    bar: i32,
}
"#;
        let chunks = chunk_rust_file("test.rs", code);
        assert!(chunks.len() >= 2, "should find fn + struct: got {}", chunks.len());
    }

    #[test]
    fn chunk_sliding_window_basic() {
        let lines: Vec<&str> = (0..100).map(|i| "some code line here").collect();
        let chunks = chunk_sliding_window(&lines, 50, 12);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn sled_store_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = VectorStore::open(tmp.path()).unwrap();

        let chunk = Chunk {
            file: "test.rs".into(),
            lines: (1, 10),
            text: "fn main() {}".into(),
            embedding: vec![1.0, 0.0, 0.5],
        };

        store.insert(&chunk).unwrap();

        let results = store.search(&[1.0, 0.0, 0.5], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].score - 1.0).abs() < 0.001);
        assert_eq!(results[0].chunk.file, "test.rs");
    }
}
