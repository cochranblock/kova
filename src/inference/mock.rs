// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! mock — Deterministic canned-response backend for end-to-end agent loop
//! tests. Each call to f422 pops the next line from KOVA_INFERENCE_MOCK_FILE
//! (a JSONL file where each line is `{"text": "<full response>"}` or a bare
//! plain-text response with no JSON wrapping). A process-local AtomicUsize
//! tracks position; each fresh `kova chat` subprocess starts at line 0.
//!
//! f422=mock_stream.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, OnceLock};

/// Cursor into the mock file. Each f422 call advances by one.
///
/// IMPORTANT: This is process-static. Test isolation relies on each `kova chat`
/// invocation being a fresh subprocess. Calling f422 directly from multiple
/// threads inside one process (e.g., parallel cargo tests) would share the
/// cursor and produce nondeterministic responses. agent_loop_tests::f423
/// invokes only via spawned children, so the contract holds.
static MOCK_CURSOR: AtomicUsize = AtomicUsize::new(0);

/// One-time load of the mock-response file. Subsequent f422 calls in the same
/// process reuse the cached `Vec<String>` instead of re-reading from disk per
/// inference turn. Cleared only by process exit.
static MOCK_LINES: OnceLock<MockLoad> = OnceLock::new();

enum MockLoad {
    Ok(Vec<String>),
    Err(String),
}

/// f422=mock_stream. Read the next canned response from KOVA_INFERENCE_MOCK_FILE
/// and send it as a single streamed chunk. Falls back to a marker string if the
/// cursor walks past the end of the file (so the agent loop terminates
/// gracefully on overconsumption rather than hanging).
///
/// Mock file format: one response per line.
///   - `{"text": "..."}` — JSON-wrapped; preferred for multi-line responses.
///   - Anything else — sent verbatim as the response.
pub fn f422(
    _model_path: &Path,
    _system_prompt: &str,
    _history: &[(String, String)],
    _user_input: &str,
) -> mpsc::Receiver<Arc<str>> {
    let (tx, rx) = mpsc::channel();

    let load = MOCK_LINES.get_or_init(load_mock_file);
    let lines = match load {
        MockLoad::Ok(v) => v,
        MockLoad::Err(msg) => {
            // Prefix with a clearly-non-LLM marker so callers can tell config
            // errors apart from real model output.
            let _ = tx.send(Arc::from(format!("[KOVA_MOCK_ERROR] {msg}")));
            return rx;
        }
    };

    let idx = MOCK_CURSOR.fetch_add(1, Ordering::Relaxed);
    let raw = lines.get(idx).map(String::as_str).unwrap_or(
        // Overconsumption guard: agent loop saw more turns than the test
        // scripted. Emit a no-tool-call response so f148 finishes cleanly.
        "[mock: end of script]",
    );

    let text = parse_mock_response(raw);
    let _ = tx.send(Arc::from(text));
    rx
}

/// One-shot loader: read the configured mock file into memory, splitting on
/// newlines and skipping blanks. Called at most once per process via OnceLock.
fn load_mock_file() -> MockLoad {
    let file_path = match std::env::var("KOVA_INFERENCE_MOCK_FILE") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            return MockLoad::Err(
                "KOVA_INFERENCE=mock requires KOVA_INFERENCE_MOCK_FILE to be set.".into(),
            );
        }
    };
    let content = match std::fs::read_to_string(&file_path) {
        Ok(s) => s,
        Err(e) => return MockLoad::Err(format!("failed to read mock file {file_path}: {e}")),
    };
    let lines = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect();
    MockLoad::Ok(lines)
}

/// Parse a mock-file line: either `{"text": "..."}` JSON-wrapped, or raw text.
fn parse_mock_response(line: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line)
        && let Some(t) = parsed.get("text").and_then(|v| v.as_str())
    {
        return t.to_string();
    }
    line.to_string()
}

/// Reset the mock cursor. Intended for tests that re-enter inference within
/// the same process; not exposed to production code paths.
#[doc(hidden)]
pub fn reset_cursor() {
    MOCK_CURSOR.store(0, Ordering::Relaxed);
}
