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
    db: sled::Db,
    tree: sled::Tree,
}

impl VectorStore {
    /// Open or create a vector store at the given path.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let db = sled::open(path)?;
        let tree = db.open_tree("rag_chunks")?;
        Ok(Self { db, tree })
    }

    /// Default store path: ~/.kova/rag/vectors
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".kova").join("rag").join("vectors")
    }

    /// Insert a chunk. Key = file\0start_line (null byte separator avoids collision with : in paths).
    pub fn insert(&self, chunk: &Chunk) -> anyhow::Result<()> {
        let key = format!("{}\0{}", chunk.file, chunk.lines.0);
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
        let prefix = format!("{}\0", file);
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

/// Chunk a Rust source file into logical blocks using syntax-aware symbol extraction.
/// Uses crate::syntax::f201() for AST-aware boundaries.
/// Fallback: sliding window of ~50 lines with 25% overlap (non-Rust or empty).
pub fn chunk_rust_file(_file_path: &str, content: &str) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    // Use syntax module for symbol-aware chunking.
    let symbols = crate::syntax::f201(content);

    let mut chunks = Vec::new();

    if symbols.is_empty() {
        // No symbols found — fall back to sliding window.
        return chunk_sliding_window(&lines, 50, 12);
    }

    // Collect preamble (use/mod/comment lines before first symbol).
    let first_start = symbols[0].line_start;
    if first_start > 0 {
        let text = lines[..first_start].join("\n");
        if text.trim().len() > 20 {
            chunks.push((1, first_start, text));
        }
    }

    // Each symbol → one chunk. Gaps between symbols become their own chunk.
    let mut prev_end = first_start;
    for sym in &symbols {
        // Gap between previous symbol end and this symbol start.
        if sym.line_start > prev_end + 1 {
            let gap_text = lines[prev_end + 1..sym.line_start].join("\n");
            if gap_text.trim().len() > 20 {
                chunks.push((prev_end + 2, sym.line_start, gap_text));
            }
        }

        let start = sym.line_start;
        let end = sym.line_end;
        let text = lines[start..=end.min(lines.len() - 1)].join("\n");
        if text.trim().len() > 5 {
            chunks.push((start + 1, end + 1, text));
        }
        prev_end = end;
    }

    // Trailing lines after last symbol.
    if prev_end + 1 < lines.len() {
        let text = lines[prev_end + 1..].join("\n");
        if text.trim().len() > 20 {
            chunks.push((prev_end + 2, lines.len(), text));
        }
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
        let embeddings = embed_texts(batch)?;

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

    // Remove old chunks only after embeddings are ready (avoids partial state).
    let files_to_update: std::collections::HashSet<&str> = embedded_chunks
        .iter()
        .map(|c| c.file.as_str())
        .collect();
    for file in &files_to_update {
        let _ = store.remove_file(file);
    }

    let count = store.insert_many(&embedded_chunks)?;
    mark_indexed(store, dir)?;
    eprintln!("[rag] indexed {} chunks", count);
    Ok(count)
}

/// Search the store with a natural language query.
pub fn search(store: &VectorStore, query: &str, k: usize) -> anyhow::Result<Vec<SearchResult>> {
    let query_emb = embed_query(query)?;
    store.search(&query_emb, k)
}

// ── Auto-Reindex ─────────────────────────────────────────────────

/// f167=needs_reindex. Check if any .rs files in `dir` have been modified since
/// the last index time recorded in the store. Returns true if re-indexing is needed.
pub fn needs_reindex(store: &VectorStore, dir: &Path) -> bool {
    let indexed_tree = match store.db.open_tree("last_indexed") {
        Ok(t) => t,
        Err(_) => return true,
    };

    let key = dir.to_string_lossy();
    let ts_bytes = match indexed_tree.get(key.as_bytes()) {
        Ok(Some(v)) => v,
        _ => return true,
    };

    let last_ts: u64 = match std::str::from_utf8(&ts_bytes) {
        Ok(s) => s.parse().unwrap_or(0),
        Err(_) => return true,
    };

    let last_indexed_time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(last_ts);

    // Walk .rs files, check if any modified after last_indexed_time
    let pattern = dir.join("**/*.rs");
    let pattern_str = pattern.to_string_lossy();
    let entries = match glob::glob(&pattern_str) {
        Ok(e) => e,
        Err(_) => return true,
    };

    for entry in entries {
        let path = match entry {
            Ok(p) => p,
            Err(_) => continue,
        };

        let path_str = path.to_string_lossy();
        if path_str.contains("/target/") || path_str.contains("/.") {
            continue;
        }

        if let Ok(meta) = std::fs::metadata(&path)
            && let Ok(modified) = meta.modified()
            && modified > last_indexed_time
        {
            return true;
        }
    }

    false
}

/// f168=mark_indexed. Record that `dir` was indexed at the current timestamp.
pub fn mark_indexed(store: &VectorStore, dir: &Path) -> anyhow::Result<()> {
    let indexed_tree = store.db.open_tree("last_indexed")?;
    let key = dir.to_string_lossy();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    indexed_tree.insert(key.as_bytes(), now.to_string().as_bytes())?;
    indexed_tree.flush()?;
    Ok(())
}

/// f169=auto_reindex. Check if `dir` needs re-indexing and do it if so.
/// Returns the number of chunks indexed (0 if skipped because index is fresh).
pub fn auto_reindex(store: &VectorStore, dir: &Path) -> anyhow::Result<usize> {
    if !needs_reindex(store, dir) {
        return Ok(0);
    }
    let count = index_directory(store, dir)?;
    Ok(count)
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
        let lines: Vec<&str> = (0..100).map(|_| "some code line here").collect();
        let chunks = chunk_sliding_window(&lines, 50, 12);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn f168_mark_and_check_indexed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = VectorStore::open(tmp.path()).unwrap();
        let dir = tmp.path();

        // Before marking: needs_reindex should return true (no record)
        assert!(needs_reindex(&store, dir));

        // Mark indexed
        mark_indexed(&store, dir).unwrap();

        // After marking with no .rs files modified after: should be false
        assert!(!needs_reindex(&store, dir));
    }

    #[test]
    fn f167_needs_reindex_detects_new_file() {
        // Use /tmp/kova_test_reindex to avoid macOS tempdir paths containing /. (hidden dir filter)
        let base = std::path::PathBuf::from("/tmp/kova_test_reindex");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        let store = VectorStore::open(&base.join("db")).unwrap();
        let src_dir = base.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        // Write a timestamp in the past (60 seconds ago) so any new file is "newer"
        let indexed_tree = store.db.open_tree("last_indexed").unwrap();
        let key = src_dir.to_string_lossy();
        let past_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 60;
        indexed_tree
            .insert(key.as_bytes(), past_ts.to_string().as_bytes())
            .unwrap();

        // Create a .rs file (mtime = now, which is after past_ts)
        std::fs::write(src_dir.join("new.rs"), "fn main() {}").unwrap();

        // Should detect the new file
        assert!(needs_reindex(&store, &src_dir));

        // Cleanup
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn f169_auto_reindex_skips_fresh() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = VectorStore::open(&tmp.path().join("db")).unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        // Mark as indexed, no .rs files exist
        mark_indexed(&store, &src_dir).unwrap();

        // auto_reindex should return 0 (skipped)
        let count = auto_reindex(&store, &src_dir).unwrap();
        assert_eq!(count, 0);
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

    #[test]
    fn chunk_rust_file_with_strings_and_comments() {
        let code = r##"
// comment with { braces } inside
fn parse() {
    let s = "string with { more } braces";
    let r = r#"raw string { }"#;
}
"##;
        let chunks = chunk_rust_file("parse.rs", code);
        assert!(!chunks.is_empty(), "should chunk despite braces in strings/comments");
        let full: String = chunks.iter().map(|(_, _, t)| t.as_str()).collect::<Vec<_>>().join("\n");
        assert!(full.contains("parse"), "should include fn parse");
    }

    #[test]
    fn vector_store_remove_then_insert_same_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = VectorStore::open(tmp.path()).unwrap();

        let chunk1 = Chunk {
            file: "a.rs".to_string(),
            lines: (1, 5),
            text: "old".into(),
            embedding: vec![1.0, 0.0, 0.0],
        };
        store.insert(&chunk1).unwrap();
        let removed = store.remove_file("a.rs").unwrap();
        assert_eq!(removed, 1);

        let chunk2 = Chunk {
            file: "a.rs".to_string(),
            lines: (1, 8),
            text: "new".into(),
            embedding: vec![0.0, 1.0, 0.0],
        };
        store.insert(&chunk2).unwrap();

        let results = store.search(&[0.0, 1.0, 0.0], 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.text, "new");
    }

    // ── TEST-2/3: Cross-module integration & edge cases ─────

    #[test]
    fn chunk_rust_file_uses_syntax_symbols() {
        // Verify that chunk_rust_file produces symbol-aligned chunks via syntax::extract_symbols
        let code = "pub fn alpha() {\n    1 + 1\n}\n\npub fn beta() {\n    2 + 2\n}\n";
        let chunks = chunk_rust_file("sym.rs", code);
        // Should produce at least two chunks (one per function), possibly with preamble
        let fn_chunks: Vec<_> = chunks.iter().filter(|(_, _, t)| t.contains("fn ")).collect();
        assert!(fn_chunks.len() >= 2, "expected at least 2 function chunks, got {}", fn_chunks.len());
    }

    #[test]
    fn chunk_rust_file_empty() {
        let chunks = chunk_rust_file("empty.rs", "");
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_rust_file_no_symbols() {
        // File with only comments and blank lines — no symbols
        let code = "// just a comment\n// another comment\n";
        let chunks = chunk_rust_file("comments.rs", code);
        // Should produce a preamble chunk or nothing, but not panic
        assert!(chunks.len() <= 1);
    }

    #[test]
    fn chunk_rust_file_large_function_splits() {
        // A function >100 lines should be split via sliding window
        let mut code = String::from("fn big() {\n");
        for i in 0..120 {
            code.push_str(&format!("    let x{} = {};\n", i, i));
        }
        code.push_str("}\n");
        let chunks = chunk_rust_file("big.rs", &code);
        // Should be split into multiple chunks
        assert!(chunks.len() >= 2, "big function should split, got {} chunks", chunks.len());
    }

    #[test]
    fn chunk_rust_file_preamble_captured() {
        // Use/mod lines before first symbol should be captured as preamble
        let code = "use std::io;\nuse std::path::Path;\n\npub fn work() {\n    42\n}\n";
        let chunks = chunk_rust_file("preamble.rs", code);
        let all_text: String = chunks.iter().map(|(_, _, t)| t.as_str()).collect::<Vec<_>>().join("\n");
        assert!(all_text.contains("use std::io"), "preamble should be captured");
        assert!(all_text.contains("fn work"), "symbol should be captured");
    }

    #[test]
    fn vector_store_search_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = VectorStore::open(tmp.path()).unwrap();
        let results = store.search(&[1.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn vector_store_multiple_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = VectorStore::open(tmp.path()).unwrap();

        for i in 0..3 {
            let chunk = Chunk {
                file: format!("file{}.rs", i),
                lines: (1, 5),
                text: format!("content {}", i),
                embedding: vec![i as f32, 0.0, 0.0],
            };
            store.insert(&chunk).unwrap();
        }

        // Remove one file
        let removed = store.remove_file("file1.rs").unwrap();
        assert_eq!(removed, 1);

        // Should have 2 remaining
        let results = store.search(&[1.0, 0.0, 0.0], 10).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.chunk.file != "file1.rs"));
    }

    #[test]
    fn vector_store_cosine_similarity_ordering() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = VectorStore::open(tmp.path()).unwrap();

        // Insert chunks with known embeddings
        let close = Chunk {
            file: "close.rs".into(),
            lines: (1, 1),
            text: "close match".into(),
            embedding: vec![0.9, 0.1, 0.0],
        };
        let far = Chunk {
            file: "far.rs".into(),
            lines: (1, 1),
            text: "far match".into(),
            embedding: vec![0.0, 0.0, 1.0],
        };
        store.insert(&close).unwrap();
        store.insert(&far).unwrap();

        let results = store.search(&[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk.file, "close.rs", "closer vector should rank first");
    }
}
