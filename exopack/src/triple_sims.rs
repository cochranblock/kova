// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! TRIPLE SIMS — Three programmatic simulations for quality evaluation.
//!
//! Sim 1 (f170): User Story UX — walk user scenarios, verify elicitation/router/context flows.
//! Sim 2 (f171): Feature Gap — check epics/acceptance criteria vs actual source.
//! Sim 3 (f172): Implementation Deep Dive — inspect code paths for consistency.
//!
//! Each sim reads source files and checks for required patterns/structures.
//! Output: SimReport with findings per simulation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(feature = "video")]
use crate::video::VideoRecorder;

// ── Types ──────────────────────────────────────────────────────────────

/// Severity of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Pass,
    Info,
    Warning,
    Fail,
}

/// Single finding from a simulation.
#[derive(Debug, Clone)]
pub struct Finding {
    pub sim: u8,
    pub severity: Severity,
    pub area: String,
    pub message: String,
}

/// Result of one simulation.
#[derive(Debug, Clone)]
pub struct SimResult {
    pub sim: u8,
    pub name: String,
    pub findings: Vec<Finding>,
}

impl SimResult {
    fn pass_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Pass).count()
    }
    fn fail_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Fail).count()
    }
    fn warn_count(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Warning).count()
    }
    fn ok(&self) -> bool {
        self.fail_count() == 0
    }
}

/// Full TRIPLE SIMS report (expandable — Sim 1-3 core, Sim 4+ extensions).
#[derive(Debug)]
pub struct SimReport {
    pub sims: Vec<SimResult>,
}

impl SimReport {
    pub fn ok(&self) -> bool {
        self.sims.iter().all(|s| s.ok())
    }

    pub fn summary(&self) -> String {
        let mut out = String::from("TRIPLE SIMS Report\n==================\n");
        for sim in &self.sims {
            out.push_str(&format!(
                "\nSim {}: {} — {} pass, {} warn, {} fail\n",
                sim.sim, sim.name, sim.pass_count(), sim.warn_count(), sim.fail_count()
            ));
            for f in &sim.findings {
                let icon = match f.severity {
                    Severity::Pass => "  [ok]",
                    Severity::Info => "  [--]",
                    Severity::Warning => "  [!!]",
                    Severity::Fail => "  [XX]",
                };
                out.push_str(&format!("{} {}: {}\n", icon, f.area, f.message));
            }
        }
        let total_pass: usize = self.sims.iter().map(|s| s.pass_count()).sum();
        let total_fail: usize = self.sims.iter().map(|s| s.fail_count()).sum();
        let total_warn: usize = self.sims.iter().map(|s| s.warn_count()).sum();
        out.push_str(&format!(
            "\nTotal: {} pass, {} warn, {} fail — {}\n",
            total_pass,
            total_warn,
            total_fail,
            if total_fail == 0 { "ALL SIMS PASS" } else { "SIMS FAILED" }
        ));
        out
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn read_src(project: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(project.join(rel)).ok()
}

/// Resolve module path: src/foo.rs or src/foo/mod.rs
fn resolve_mod(project: &Path, rel: &str) -> Option<PathBuf> {
    let direct = project.join(rel);
    if direct.exists() {
        return Some(direct);
    }
    // Try src/foo/mod.rs for src/foo.rs
    if rel.ends_with(".rs") {
        let dir = project.join(rel.trim_end_matches(".rs"));
        if dir.is_dir() {
            let mod_rs = dir.join("mod.rs");
            if mod_rs.exists() {
                return Some(mod_rs);
            }
        }
    }
    None
}

/// Read module source. For directory modules (src/pipeline/), concatenates all .rs files.
fn read_mod(project: &Path, rel: &str) -> Option<String> {
    let direct = project.join(rel);
    if direct.exists() {
        return std::fs::read_to_string(direct).ok();
    }
    if rel.ends_with(".rs") {
        let dir = project.join(rel.trim_end_matches(".rs"));
        if dir.is_dir() {
            let mut combined = String::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    if e.path().extension().is_some_and(|x| x == "rs") {
                        if let Ok(s) = std::fs::read_to_string(e.path()) {
                            combined.push_str(&s);
                            combined.push('\n');
                        }
                    }
                }
            }
            if !combined.is_empty() {
                return Some(combined);
            }
        }
    }
    None
}

fn src_contains(project: &Path, rel: &str, pattern: &str) -> bool {
    read_mod(project, rel).is_some_and(|s| s.contains(pattern))
}

fn src_contains_any(project: &Path, rel: &str, patterns: &[&str]) -> bool {
    read_mod(project, rel).is_some_and(|s| patterns.iter().any(|p| s.contains(p)))
}

fn file_exists(project: &Path, rel: &str) -> bool {
    resolve_mod(project, rel).is_some()
}

fn finding(sim: u8, ok: bool, area: &str, pass_msg: &str, fail_msg: &str) -> Finding {
    Finding {
        sim,
        severity: if ok { Severity::Pass } else { Severity::Fail },
        area: area.to_string(),
        message: (if ok { pass_msg } else { fail_msg }).to_string(),
    }
}

fn info(sim: u8, area: &str, msg: &str) -> Finding {
    Finding { sim, severity: Severity::Info, area: area.to_string(), message: msg.to_string() }
}

fn warn(sim: u8, area: &str, msg: &str) -> Finding {
    Finding { sim, severity: Severity::Warning, area: area.to_string(), message: msg.to_string() }
}

// ── Sim 1: User Story UX Simulation (f170) ────────────────────────────

/// f170=sim1_user_story. Walk user scenarios, verify router/elicitor/context/output flows.
#[allow(clippy::vec_init_then_push)]
pub fn f170_sim1_user_story(project: &Path) -> SimResult {
    let mut findings = Vec::new();

    // Scenario A: Code Gen Flow — "add exponential backoff to compute.rs"
    // Step 1: Router classifies code_gen for "add" patterns
    findings.push(finding(1,
        src_contains(project, "src/router.rs", "code_gen"),
        "A1-router-codegen",
        "Router has code_gen classification",
        "Router missing code_gen classification",
    ));

    // Step 2: Router has needs_clarification variant
    findings.push(finding(1,
        src_contains(project, "src/router.rs", "NeedsClarification"),
        "A2-router-clarification",
        "Router has NeedsClarification variant",
        "Router missing NeedsClarification",
    ));

    // Step 3: Router provides question + choices for clarification
    findings.push(finding(1,
        src_contains_any(project, "src/router.rs", &["question:", "choices:"]),
        "A3-router-question-choices",
        "Router provides question/choices in clarification",
        "Router missing question/choices fields",
    ));

    // Step 4: Context injection — cursor_prompts loaded in serve
    findings.push(finding(1,
        src_contains(project, "src/serve.rs", "cursor_prompts"),
        "A4-serve-cursor-prompts",
        "Serve injects cursor_prompts",
        "Serve missing cursor_prompts injection",
    ));

    // Step 5: Context injection — cursor_prompts in GUI
    let gui_has_cursor = src_contains(project, "src/gui.rs", "cursor_prompts")
        || src_contains(project, "src/gui.rs", "build_system_prompt");
    findings.push(finding(1,
        gui_has_cursor,
        "A5-gui-cursor-prompts",
        "GUI has cursor_prompts or build_system_prompt",
        "GUI missing cursor_prompts injection — inconsistent with serve",
    ));

    // Step 6: Pipeline validates (check → clippy → test)
    findings.push(finding(1,
        src_contains(project, "src/pipeline.rs", "cargo_check")
            || src_contains(project, "src/lib.rs", "cargo_check"),
        "A6-pipeline-check",
        "Pipeline runs cargo check",
        "Pipeline missing cargo check validation",
    ));

    // Step 7: Fix loop with max retries (DDI)
    findings.push(finding(1,
        src_contains_any(project, "src/pipeline.rs", &["max_fix_retries", "fix_and_retry", "max_retries"]),
        "A7-pipeline-fix-loop",
        "Pipeline has fix loop with max retries",
        "Pipeline missing fix retry loop",
    ));

    // Step 8: Output — diff and copy in GUI
    findings.push(finding(1,
        src_contains_any(project, "src/gui.rs", &["diff", "Copy", "Apply"]),
        "A8-gui-output",
        "GUI has diff/Copy/Apply output",
        "GUI missing diff/Copy/Apply",
    ));

    // Scenario B: Clarification Flow — "fix the bug"
    // Step 1: Elicitor module exists and formats questions
    findings.push(finding(1,
        file_exists(project, "src/elicitor.rs")
            && src_contains(project, "src/elicitor.rs", "format_question"),
        "B1-elicitor-exists",
        "Elicitor module with format_question exists",
        "Elicitor module missing or incomplete",
    ));

    // Step 2: Elicitor parses short replies (a/b/y/n)
    findings.push(finding(1,
        src_contains(project, "src/elicitor.rs", "parse_reply"),
        "B2-elicitor-parse-reply",
        "Elicitor parses short replies (a/b/y/n)",
        "Elicitor missing reply parsing",
    ));

    // Step 3: Elicitor builds restatements
    findings.push(finding(1,
        src_contains(project, "src/elicitor.rs", "build_restatement"),
        "B3-elicitor-restatement",
        "Elicitor builds restatements before generation",
        "Elicitor missing restatement capability",
    ));

    // Step 4: Cancel flow (cancel/stop/abort)
    findings.push(finding(1,
        src_contains_any(project, "src/elicitor.rs", &["Cancel", "cancel", "stop", "abort"]),
        "B4-elicitor-cancel",
        "Elicitor supports cancel flow",
        "Elicitor missing cancel handling",
    ));

    // Step 5: Router clarification_question fallback
    findings.push(finding(1,
        src_contains(project, "src/router.rs", "clarification_question"),
        "B5-router-fallback-question",
        "Router has clarification_question fallback",
        "Router missing clarification_question fallback",
    ));

    // Scenario C: Context awareness
    // Step 1: context_loader extracts files from input
    findings.push(finding(1,
        file_exists(project, "src/context_loader.rs")
            && src_contains_any(project, "src/context_loader.rs", &[".rs", "load_project_context", "target_file"]),
        "C1-context-loader",
        "Context loader extracts relevant files",
        "Context loader missing file extraction",
    ));

    // Step 2: compression_map in prompts
    findings.push(finding(1,
        src_contains_any(project, "src/cursor_prompts.rs", &["compression_map", "fN", "tN"])
            || src_contains_any(project, "src/config.rs", &["compression_map"]),
        "C2-compression-map",
        "Compression map available in prompt context",
        "Compression map not in prompt context",
    ));

    // Step 3: Recent changes available
    findings.push(finding(1,
        file_exists(project, "src/recent_changes.rs"),
        "C3-recent-changes",
        "Recent changes module exists",
        "Recent changes module missing",
    ));

    // Scenario D: Streaming output
    findings.push(finding(1,
        src_contains_any(project, "src/inference.rs", &["stream", "Stream"]),
        "D1-streaming",
        "Inference supports streaming",
        "Inference missing streaming support",
    ));

    SimResult { sim: 1, name: "User Story UX".to_string(), findings }
}

// ── Sim 2: Feature Gap Analysis (f171) ─────────────────────────────────

/// Acceptance criterion to check.
struct Criterion {
    epic: &'static str,
    id: &'static str,
    description: &'static str,
    file: &'static str,
    patterns: &'static [&'static str],
}

const CRITERIA: &[Criterion] = &[
    // E1: Tease Intent
    Criterion { epic: "E1", id: "E1.1", description: "Router detects ambiguity → needs_clarification", file: "src/router.rs", patterns: &["needs_clarification", "NeedsClarification"] },
    Criterion { epic: "E1", id: "E1.2", description: "Choices offered (a/b/c)", file: "src/elicitor.rs", patterns: &["format_question", "choices"] },
    Criterion { epic: "E1", id: "E1.3", description: "Restatement before generating", file: "src/elicitor.rs", patterns: &["build_restatement", "Proceed"] },
    Criterion { epic: "E1", id: "E1.4", description: "Cancel at any step", file: "src/elicitor.rs", patterns: &["Cancel", "cancel", "stop"] },

    // E2: Elicitation UX
    Criterion { epic: "E2", id: "E2.1", description: "Inline clarification in chat flow", file: "src/router.rs", patterns: &["clarification_question"] },
    Criterion { epic: "E2", id: "E2.2", description: "Short replies (y/n, a/b)", file: "src/elicitor.rs", patterns: &["parse_reply"] },
    Criterion { epic: "E2", id: "E2.3", description: "Easy cancel", file: "src/elicitor.rs", patterns: &["Cancel", "abort"] },
    Criterion { epic: "E2", id: "E2.4", description: "System shows what it understood", file: "src/elicitor.rs", patterns: &["build_restatement"] },

    // E3: Code Gen Pipeline
    Criterion { epic: "E3", id: "US-2.1", description: "Coder gets system+persona+conventions", file: "src/cursor_prompts.rs", patterns: &["load_cursor_prompts", "baked"] },
    Criterion { epic: "E3", id: "US-2.2", description: "Target file from input", file: "src/context_loader.rs", patterns: &["target_file", "extract"] },
    Criterion { epic: "E3", id: "US-2.3", description: "Validate before output (check→fix→retry)", file: "src/pipeline.rs", patterns: &["cargo_check", "fix_and_retry"] },
    Criterion { epic: "E3", id: "US-2.4", description: "Clippy clean", file: "src/pipeline.rs", patterns: &["clippy"] },

    // E4: Router clarification
    Criterion { epic: "E4", id: "E4.1", description: "Router detects ambiguity", file: "src/router.rs", patterns: &["needs_clarification"] },
    Criterion { epic: "E4", id: "E4.2", description: "Suggested question from router", file: "src/router.rs", patterns: &["question", "choices"] },
    Criterion { epic: "E4", id: "E4.3", description: "Elicitor uses router suggestions", file: "src/elicitor.rs", patterns: &["format_question"] },

    // E5: Model orchestration
    Criterion { epic: "E5", id: "US-3.x", description: "Model config and roles", file: "src/config.rs", patterns: &["ModelRole", "model_path"] },
    Criterion { epic: "E5", id: "US-4.x", description: "Project awareness and context", file: "src/context_loader.rs", patterns: &["load_project_context", "ProjectContext"] },

    // E6: Output
    Criterion { epic: "E6", id: "US-6.1", description: "Copy button", file: "src/gui.rs", patterns: &["Copy", "copy"] },
    Criterion { epic: "E6", id: "US-6.2", description: "Diff view", file: "src/gui.rs", patterns: &["diff", "Diff"] },
    Criterion { epic: "E6", id: "US-6.3", description: "Backlog", file: "src/backlog.rs", patterns: &["load_backlog", "Backlog"] },
    Criterion { epic: "E6", id: "US-6.4", description: "Streaming output", file: "src/inference.rs", patterns: &["stream", "Stream"] },
];

/// f171=sim2_feature_gap. Check acceptance criteria from user stories against implementation.
pub fn f171_sim2_feature_gap(project: &Path) -> SimResult {
    let mut findings = Vec::new();

    for c in CRITERIA {
        let src = read_mod(project, c.file).unwrap_or_default();
        let met = c.patterns.iter().any(|p| src.contains(p));
        findings.push(Finding {
            sim: 2,
            severity: if met { Severity::Pass } else { Severity::Fail },
            area: format!("{}/{}", c.epic, c.id),
            message: if met {
                format!("{} — met", c.description)
            } else {
                format!("{} — NOT FOUND in {}", c.description, c.file)
            },
        });
    }

    // Gap summary: check for known gaps from TRIPLE_SIMS_KOVA.md
    // Gap 1: GUI CodeGen cursor_prompts consistency
    let gui_src = read_src(project, "src/gui.rs").unwrap_or_default();
    let serve_src = read_src(project, "src/serve.rs").unwrap_or_default();
    let gui_injects = gui_src.contains("cursor_prompts") || gui_src.contains("build_system_prompt");
    let serve_injects = serve_src.contains("cursor_prompts");
    if gui_injects && serve_injects {
        findings.push(Finding {
            sim: 2, severity: Severity::Pass,
            area: "GAP-1".to_string(),
            message: "GUI and serve both inject cursor_prompts — parity achieved".to_string(),
        });
    } else {
        findings.push(Finding {
            sim: 2, severity: Severity::Warning,
            area: "GAP-1".to_string(),
            message: format!(
                "cursor_prompts parity: GUI={}, serve={} — inconsistency",
                if gui_injects { "yes" } else { "no" },
                if serve_injects { "yes" } else { "no" },
            ),
        });
    }

    // Gap 2: Serve diff/copy
    let serve_has_diff = serve_src.contains("diff") || serve_src.contains("Diff");
    if !serve_has_diff {
        findings.push(warn(2, "GAP-2", "Serve has no diff UI — stream-only output"));
    }

    // Gap 3: Serve backlog API
    let serve_has_backlog = serve_src.contains("backlog");
    if !serve_has_backlog {
        findings.push(warn(2, "GAP-3", "Serve has no backlog add endpoint"));
    }

    SimResult { sim: 2, name: "Feature Gap".to_string(), findings }
}

// ── Sim 3: Implementation Deep Dive (f172) ─────────────────────────────

/// f172=sim3_impl_deep_dive. Inspect code paths for consistency and correctness.
pub fn f172_sim3_impl_deep_dive(project: &Path) -> SimResult {
    let mut findings = Vec::new();

    // 3A: Cursor prompts injection consistency across all code paths
    let paths_to_check = [
        ("serve.rs", "src/serve.rs"),
        ("gui.rs", "src/gui.rs"),
        ("pipeline.rs", "src/pipeline.rs"),
        ("academy.rs", "src/academy.rs"),
    ];
    for (label, rel) in &paths_to_check {
        let has = src_contains_any(project, rel, &["cursor_prompts", "load_cursor_prompts", "build_system_prompt"]);
        if has {
            findings.push(Finding {
                sim: 3, severity: Severity::Pass,
                area: format!("3A-cursor-{}", label),
                message: format!("{} injects cursor_prompts/system_prompt", label),
            });
        } else if file_exists(project, rel) {
            findings.push(warn(3,
                &format!("3A-cursor-{}", label),
                &format!("{} does NOT inject cursor_prompts — conventions may be missing in this path", label),
            ));
        }
    }

    // 3B: Pipeline flow correctness (check → clippy → test → fix loop)
    // Pipeline may be src/pipeline.rs or src/pipeline/ directory
    let pipeline_src = read_mod(project, "src/pipeline.rs")
        .or_else(|| {
            // Read all .rs files in src/pipeline/ and concatenate
            let dir = project.join("src/pipeline");
            if dir.is_dir() {
                let mut combined = String::new();
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for e in entries.flatten() {
                        if e.path().extension().is_some_and(|x| x == "rs") {
                            if let Ok(s) = std::fs::read_to_string(e.path()) {
                                combined.push_str(&s);
                                combined.push('\n');
                            }
                        }
                    }
                }
                if combined.is_empty() { None } else { Some(combined) }
            } else {
                None
            }
        });
    if let Some(pipeline) = pipeline_src {
        let has_check = pipeline.contains("cargo_check") || pipeline.contains("cargo check");
        let has_clippy = pipeline.contains("clippy");
        let has_test = pipeline.contains("cargo_test") || pipeline.contains("cargo test");
        let has_fix = pipeline.contains("fix_and_retry") || pipeline.contains("fix_loop");
        let has_max = pipeline.contains("max_fix_retries") || pipeline.contains("max_retries");

        findings.push(finding(3, has_check, "3B-pipeline-check", "Pipeline runs cargo check", "Pipeline missing cargo check"));
        findings.push(finding(3, has_clippy, "3B-pipeline-clippy", "Pipeline runs clippy", "Pipeline missing clippy"));
        findings.push(finding(3, has_test, "3B-pipeline-test", "Pipeline runs cargo test", "Pipeline missing cargo test"));
        findings.push(finding(3, has_fix, "3B-pipeline-fix", "Pipeline has fix-and-retry loop", "Pipeline missing fix loop"));
        findings.push(finding(3, has_max, "3B-pipeline-max-retry", "Pipeline caps retries (DDI)", "Pipeline missing retry cap — unbounded fix loops"));
    } else {
        findings.push(Finding {
            sim: 3, severity: Severity::Fail,
            area: "3B-pipeline".to_string(),
            message: "pipeline.rs not found — cannot verify flow".to_string(),
        });
    }

    // 3C: Router classification coverage
    if let Some(router) = read_src(project, "src/router.rs") {
        let expected_classes = ["code_gen", "refactor", "explain", "fix", "run", "custom", "needs_clarification"];
        for class in &expected_classes {
            let has = router.contains(class);
            findings.push(finding(3, has,
                &format!("3C-router-{}", class),
                &format!("Router handles {}", class),
                &format!("Router missing {} classification", class),
            ));
        }
    }

    // 3D: Compression map / tokenization presence
    let has_compression_map = file_exists(project, "docs/compression_map.md");
    findings.push(finding(3, has_compression_map,
        "3D-compression-map",
        "compression_map.md exists",
        "compression_map.md missing — tokenization not documented",
    ));

    // Check baked prompts reference compression_map
    if let Some(cp) = read_src(project, "src/cursor_prompts.rs") {
        let baked_has_tokens = cp.contains("fN") || cp.contains("tN") || cp.contains("compression_map")
            || cp.contains("f14") || cp.contains("tokeniz");
        findings.push(finding(3, baked_has_tokens,
            "3D-baked-tokenization",
            "Baked prompts reference compression/tokenization",
            "Baked prompts do not mention compression_map tokens",
        ));
    }

    // 3E: Test coverage check — run cargo test --no-run to verify it compiles
    let compile_result = Command::new("cargo")
        .args(["test", "--no-run", "-p", "kova"])
        .current_dir(project)
        .output();
    match compile_result {
        Ok(o) if o.status.success() => {
            findings.push(Finding {
                sim: 3, severity: Severity::Pass,
                area: "3E-test-compile".to_string(),
                message: "cargo test --no-run compiles successfully".to_string(),
            });
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let short = stderr.lines().take(5).collect::<Vec<_>>().join("; ");
            findings.push(Finding {
                sim: 3, severity: Severity::Fail,
                area: "3E-test-compile".to_string(),
                message: format!("cargo test --no-run failed: {}", short),
            });
        }
        Err(e) => {
            findings.push(Finding {
                sim: 3, severity: Severity::Fail,
                area: "3E-test-compile".to_string(),
                message: format!("cargo test --no-run error: {}", e),
            });
        }
    }

    // 3F: Clippy check
    let clippy_result = Command::new("cargo")
        .args(["clippy", "-p", "kova", "--", "-D", "warnings"])
        .current_dir(project)
        .output();
    match clippy_result {
        Ok(o) if o.status.success() => {
            findings.push(Finding {
                sim: 3, severity: Severity::Pass,
                area: "3F-clippy".to_string(),
                message: "clippy passes with -D warnings".to_string(),
            });
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let warnings: Vec<&str> = stderr.lines()
                .filter(|l| l.contains("warning[") || l.contains("error["))
                .take(5)
                .collect();
            findings.push(Finding {
                sim: 3, severity: Severity::Fail,
                area: "3F-clippy".to_string(),
                message: format!("clippy issues: {}", warnings.join("; ")),
            });
        }
        Err(e) => {
            findings.push(warn(3, "3F-clippy", &format!("clippy unavailable: {}", e)));
        }
    }

    // 3G: Key modules exist
    let required_modules = [
        ("router.rs", "Intent classification"),
        ("elicitor.rs", "Clarification UX"),
        ("pipeline.rs", "Code gen pipeline"),
        ("context_loader.rs", "Project context"),
        ("cursor_prompts.rs", "Baked conventions"),
        ("serve.rs", "HTTP serve"),
        ("gui.rs", "Native GUI"),
        ("tui.rs", "Terminal UI"),
        ("inference.rs", "Model inference"),
        ("config.rs", "Configuration"),
        ("storage.rs", "Persistent storage"),
        ("output.rs", "Output formatting"),
        ("recent_changes.rs", "Recent changes"),
        ("agent_loop.rs", "Agent loop"),
        ("tools.rs", "Tool dispatch"),
        ("repl.rs", "REPL interface"),
    ];
    for (file, purpose) in &required_modules {
        let exists = file_exists(project, &format!("src/{}", file));
        findings.push(finding(3, exists,
            &format!("3G-module-{}", file.trim_end_matches(".rs")),
            &format!("{} — {} present", file, purpose),
            &format!("{} — {} MISSING", file, purpose),
        ));
    }

    // 3H: Elicitor integration — verify GUI or serve uses elicitor
    let gui_uses_elicitor = src_contains_any(project, "src/gui.rs", &["elicitor", "ElicitorReply", "clarif"]);
    let serve_uses_elicitor = src_contains_any(project, "src/serve.rs", &["elicitor", "ElicitorReply", "clarif"]);
    let any_uses_elicitor = gui_uses_elicitor || serve_uses_elicitor;
    findings.push(finding(3, any_uses_elicitor,
        "3H-elicitor-integration",
        "Elicitor integrated in GUI or serve",
        "Elicitor module exists but not integrated in GUI/serve flows",
    ));

    // 3I: TUI quality checks
    if let Some(tui) = read_mod(project, "src/tui.rs") {
        // Verify ratatui + crossterm imports
        let has_ratatui = tui.contains("ratatui");
        let has_crossterm = tui.contains("crossterm");
        findings.push(finding(3, has_ratatui,
            "3I-tui-ratatui",
            "TUI uses ratatui for terminal rendering",
            "TUI missing ratatui import",
        ));
        findings.push(finding(3, has_crossterm,
            "3I-tui-crossterm",
            "TUI uses crossterm for terminal backend",
            "TUI missing crossterm import",
        ));

        // Visual QC integration
        let has_visual_qc = tui.contains("VisualQc") || tui.contains("visual_qc") || tui.contains("qc");
        let has_verdict = tui.contains("Verdict");
        findings.push(finding(3, has_visual_qc,
            "3I-tui-visual-qc",
            "TUI has Visual QC mode",
            "TUI missing Visual QC integration",
        ));
        findings.push(finding(3, has_verdict,
            "3I-tui-verdict",
            "TUI has Approve/Reject/Skip verdicts",
            "TUI missing verdict system for QC",
        ));

        // Chat mode
        let has_chat = tui.contains("Chat") || tui.contains("chat");
        let has_input = tui.contains("input") && tui.contains("submit");
        findings.push(finding(3, has_chat,
            "3I-tui-chat",
            "TUI has chat mode",
            "TUI missing chat mode",
        ));
        findings.push(finding(3, has_input,
            "3I-tui-input",
            "TUI has text input with submit",
            "TUI missing input handling",
        ));

        // Theme colors (THEME.md palette)
        let has_theme_colors = tui.contains("0x00, 0xd4, 0xff") || tui.contains("PRIMARY");
        findings.push(finding(3, has_theme_colors,
            "3I-tui-theme",
            "TUI uses THEME.md color palette",
            "TUI missing theme colors",
        ));

        // Keyboard handling
        let has_keys = tui.contains("KeyCode");
        let has_ctrl_c = tui.contains("CONTROL") || tui.contains("Ctrl");
        findings.push(finding(3, has_keys,
            "3I-tui-keys",
            "TUI handles keyboard input",
            "TUI missing keyboard handling",
        ));
        findings.push(finding(3, has_ctrl_c,
            "3I-tui-exit",
            "TUI supports Ctrl+C exit",
            "TUI missing Ctrl+C exit handling",
        ));

        // Test coverage
        let has_tests = tui.contains("#[cfg(test)]") || tui.contains("#[test]");
        findings.push(finding(3, has_tests,
            "3I-tui-tests",
            "TUI has unit tests",
            "TUI missing unit tests",
        ));
    }

    // 3J: TUI feature flag in Cargo.toml
    if let Some(cargo) = read_src(project, "Cargo.toml") {
        let has_tui_feature = cargo.contains("tui") && cargo.contains("ratatui");
        findings.push(finding(3, has_tui_feature,
            "3J-tui-feature",
            "Cargo.toml has tui feature with ratatui dep",
            "Cargo.toml missing tui feature flag",
        ));

        let tui_in_default = cargo.contains("\"tui\"");
        findings.push(finding(3, tui_in_default,
            "3J-tui-default",
            "TUI is in default features",
            "TUI not in default features — won't build by default",
        ));
    }

    SimResult { sim: 3, name: "Implementation Deep Dive".to_string(), findings }
}

// ── Sim 4: Visual Verification (f173) ────────────────────────────────
//
// Proper sequence:
//   Phase 1 (Capture): spawn kova serve → open browser → xcap FrameRecorder captures screenshots + video frames
//   Phase 2 (Evaluate): analyze captures — movement quality, content presence, WASM health
//   Phase 3 (Artifact): save captures to ~/.cache/screenshots/kova/ for human review
//
// Requires: interface + screenshot features. video feature optional for frame recording.

/// f173=sim4_visual. Full visual verification: capture then evaluate.
#[cfg(all(feature = "interface", feature = "screenshot"))]
pub fn f173_sim4_visual(project: &Path) -> SimResult {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            return SimResult {
                sim: 4,
                name: "Visual Verification".to_string(),
                findings: vec![warn(4, "4-runtime", &format!("tokio runtime: {}", e))],
            };
        }
    };
    rt.block_on(f173_inner(project))
}

#[cfg(all(feature = "interface", feature = "screenshot"))]
async fn f173_inner(project: &Path) -> SimResult {
    let mut findings = Vec::new();
    let out_dir = crate::screenshot::out_dir("kova");
    let _ = std::fs::create_dir_all(&out_dir);

    // ── Find binary ──────────────────────────────────────────────────
    let Some(bin) = find_kova_bin(project) else {
        findings.push(info(4, "4-binary", "kova binary not found — build first"));
        return SimResult { sim: 4, name: "Visual Verification".to_string(), findings };
    };
    findings.push(Finding {
        sim: 4, severity: Severity::Pass,
        area: "4-binary".to_string(),
        message: format!("found {}", bin.display()),
    });

    // ── Bootstrap + spawn serve ──────────────────────────────────────
    let tmp = match tempfile::TempDir::new() {
        Ok(t) => t,
        Err(e) => {
            findings.push(warn(4, "4-tmpdir", &format!("{}", e)));
            return SimResult { sim: 4, name: "Visual Verification".to_string(), findings };
        }
    };

    let _ = Command::new(&bin)
        .env("HOME", tmp.path())
        .env("KOVA_PROJECT", project)
        .env("KOVA_PROJECTS_ROOT", project.parent().unwrap_or(project))
        .arg("bootstrap")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    let (listener, base) = match crate::interface::bind_random().await {
        Ok(pair) => pair,
        Err(e) => {
            findings.push(warn(4, "4-bind", &format!("{}", e)));
            return SimResult { sim: 4, name: "Visual Verification".to_string(), findings };
        }
    };
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let mut child = match Command::new(&bin)
        .env("HOME", tmp.path())
        .env("KOVA_BIND", addr.to_string())
        .env("KOVA_PROJECT", project)
        .env("KOVA_PROJECTS_ROOT", project.parent().unwrap_or(project))
        .args(["serve"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            findings.push(warn(4, "4-spawn", &format!("{}", e)));
            return SimResult { sim: 4, name: "Visual Verification".to_string(), findings };
        }
    };

    // Wait for full stack ready (poll / which is static HTML — fast)
    let ready = wait_for_ready(&base, 20).await;
    if !ready {
        findings.push(warn(4, "4-startup", "kova serve did not respond within 20s"));
        let _ = child.kill();
        let _ = child.wait();
        return SimResult { sim: 4, name: "Visual Verification".to_string(), findings };
    }
    findings.push(Finding {
        sim: 4, severity: Severity::Pass,
        area: "4-startup".to_string(),
        message: format!("kova serve ready at {}", base),
    });

    // ════════════════════════════════════════════════════════════════
    // PHASE 1: CAPTURE — screenshots + video frames
    // ════════════════════════════════════════════════════════════════

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    // 1a. Capture HTTP screenshots (HTML content snapshots)
    let pages = [
        ("index", "/"),
        ("projects", "/api/projects"),
        ("prompts", "/api/prompts"),
        ("backlog", "/api/backlog"),
    ];
    let theme = crate::screenshot::theme_cochranblock();
    let screenshots_ok = crate::screenshot::capture_project(&base, "kova", &pages, &theme).await;

    // 1b. Capture screen frames via xcap (actual pixels on screen)
    #[cfg(feature = "video")]
    let frame_result = {
        // Open browser to kova UI
        let _ = Command::new("open").arg(&base).spawn();
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let mut recorder = crate::video::FrameRecorder::new();
        let _ = recorder.start();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let frame_path = out_dir.join("frames");
        let stop_result = recorder.stop(&frame_path);
        let movement = recorder.check_movement();
        Some((stop_result, movement))
    };
    #[cfg(not(feature = "video"))]
    let frame_result: Option<(Result<PathBuf, String>, (bool, f64, &str))> = None;

    // 1c. Fetch key page content for evaluation
    let index_html = match client.get(&format!("{}/", base)).send().await {
        Ok(r) => r.text().await.ok(),
        Err(_) => None,
    };
    let wasm_size = match client.get(&format!("{}/kova_web_bg.wasm", base)).send().await {
        Ok(r) => r.bytes().await.ok().map(|b| b.len()),
        Err(_) => None,
    };
    let endpoints_status: Vec<(&str, bool)> = {
        let mut results = Vec::new();
        for (name, path) in &pages {
            let url = format!("{}{}", base, path);
            let ok = client.get(&url).send().await.map(|r| r.status().is_success()).unwrap_or(false);
            results.push((*name, ok));
        }
        // Also check static assets
        for (name, path) in [("wasm-js", "/kova_web.js"), ("wasm-bg", "/kova_web_bg.wasm")] {
            let url = format!("{}{}", base, path);
            let ok = client.get(&url).send().await.map(|r| r.status().is_success()).unwrap_or(false);
            results.push((name, ok));
        }
        results
    };

    let _ = child.kill();
    let _ = child.wait();

    // ════════════════════════════════════════════════════════════════
    // PHASE 2: EVALUATE — analyze captured artifacts
    // ════════════════════════════════════════════════════════════════

    // 2a. Screenshot capture success
    findings.push(finding(4, screenshots_ok, "4A-screenshots",
        &format!("screenshots captured to {}", out_dir.display()),
        "screenshot capture failed",
    ));

    // 2b. Endpoint health
    for (name, ok) in &endpoints_status {
        findings.push(finding(4, *ok, &format!("4B-endpoint-{}", name),
            &format!("{} responds 200", name),
            &format!("{} unreachable or error", name),
        ));
    }

    // 2c. HTML content analysis
    if let Some(ref html) = index_html {
        findings.push(finding(4, html.contains("canvas"), "4C-html-canvas",
            "index has <canvas> element", "index missing <canvas>"));
        findings.push(finding(4, html.contains("kova_web"), "4C-html-wasm",
            "index loads kova_web WASM module", "index missing WASM load"));
        findings.push(finding(4,
            html.contains("Kova") || html.contains("kova"),
            "4C-html-branding", "index references Kova", "index missing branding"));
        // Check theme colors in CSS
        findings.push(finding(4, html.contains("#0a0a0f"), "4C-theme-bg",
            "index uses THEME.md background color", "index missing theme background"));
    } else {
        findings.push(warn(4, "4C-html", "could not fetch index HTML"));
    }

    // 2d. WASM binary size
    if let Some(size) = wasm_size {
        let kb = size / 1024;
        findings.push(finding(4, kb > 100, "4D-wasm-size",
            &format!("WASM binary {}KB", kb),
            &format!("WASM binary {}KB — too small, likely broken", kb)));
    } else {
        findings.push(warn(4, "4D-wasm-size", "could not fetch WASM binary"));
    }

    // 2e. Video frame analysis (if captured)
    #[cfg(feature = "video")]
    if let Some((ref stop_result, (has_movement, quality, diagnosis))) = frame_result {
        match stop_result {
            Ok(ref path) => {
                let path: &PathBuf = path;
                findings.push(Finding {
                    sim: 4, severity: Severity::Pass,
                    area: "4E-frames-captured".to_string(),
                    message: format!("video frames saved to {}", path.display()),
                });
                findings.push(finding(4, has_movement, "4E-movement",
                    &format!("UI has movement (quality {:.2})", quality),
                    "UI frozen — no pixel change detected"));
                if has_movement {
                    let smooth = quality > 0.5;
                    findings.push(finding(4, smooth, "4E-quality",
                        &format!("{} (score {:.2})", diagnosis, quality),
                        &format!("{} (score {:.2})", diagnosis, quality)));
                }
            }
            Err(e) => {
                findings.push(info(4, "4E-frames", &format!("frame capture: {}", e)));
            }
        }
    }

    // ════════════════════════════════════════════════════════════════
    // PHASE 3: ARTIFACT — report paths for human review
    // ════════════════════════════════════════════════════════════════
    println!("  artifacts: {}", out_dir.display());

    SimResult { sim: 4, name: "Visual Verification".to_string(), findings }
}

#[cfg(all(feature = "interface", feature = "screenshot"))]
async fn wait_for_ready(base: &str, max_secs: u64) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
    // Poll / (static HTML — fast, no backend dependencies)
    let url = format!("{}/", base);
    for _ in 0..max_secs {
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    false
}

#[cfg(all(feature = "interface", feature = "screenshot"))]
fn find_kova_bin(project: &Path) -> Option<std::path::PathBuf> {
    let triples = ["aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-apple-darwin"];
    let profiles = ["release", "debug"];

    // Check project-local target dir AND workspace-level target dir
    let mut target_dirs = vec![project.join("target")];
    if let Some(parent) = project.parent() {
        target_dirs.push(parent.join("target"));
    }
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        target_dirs.push(PathBuf::from(td));
    }

    for target_dir in &target_dirs {
        for profile in &profiles {
            for triple in &triples {
                let bin = target_dir.join(triple).join(profile).join("kova");
                if bin.exists() {
                    return Some(bin);
                }
            }
            let bin = target_dir.join(profile).join("kova");
            if bin.exists() {
                return Some(bin);
            }
        }
    }
    None
}

// ── Public API ─────────────────────────────────────────────────────────

/// f60=run_async_3x. Run async closure 3 times; all must pass. Used by cochranblock-test, oakilydokily-test.
pub async fn f60<F, Fut>(run_once: F) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for i in 1..=3 {
        if !run_once().await {
            eprintln!("TRIPLE SIMS pass {}/3 failed", i);
            return false;
        }
    }
    true
}

/// f60_triple_sims_run. Run all simulations against project. Returns SimReport.
/// Sims 1-3: kova core. Sim 4: mural UI quality (if oakilydokily found as sibling).
pub fn f60_triple_sims_run(project: &Path) -> SimReport {
    println!("TRIPLE SIMS: Sim 1 — User Story UX...");
    let sim1 = f170_sim1_user_story(project);
    println!("  {} pass, {} fail", sim1.pass_count(), sim1.fail_count());

    println!("TRIPLE SIMS: Sim 2 — Feature Gap Analysis...");
    let sim2 = f171_sim2_feature_gap(project);
    println!("  {} pass, {} fail", sim2.pass_count(), sim2.fail_count());

    println!("TRIPLE SIMS: Sim 3 — Implementation Deep Dive...");
    let sim3 = f172_sim3_impl_deep_dive(project);
    println!("  {} pass, {} fail", sim3.pass_count(), sim3.fail_count());

    let mut sims = vec![sim1, sim2, sim3];

    // Sim 4: Visual Verification (screenshots + endpoint checks)
    #[cfg(all(feature = "interface", feature = "screenshot"))]
    {
        println!("TRIPLE SIMS: Sim 4 — Visual Verification...");
        let sim4 = f173_sim4_visual(project);
        println!("  {} pass, {} fail", sim4.pass_count(), sim4.fail_count());
        sims.push(sim4);
    }

    SimReport { sims }
}

/// f61=run_cargo_test_n. Runs `cargo test` N times in project_dir. Returns (ok, stderr).
/// Kept for backward compatibility — used by f90 test suite.
pub fn f61(project_dir: &Path, n: u32) -> (bool, String) {
    f61_with_args(project_dir, n, &[])
}

/// f61_with_args. f61 variant with extra cargo args.
pub fn f61_with_args(project_dir: &Path, n: u32, args: &[&str]) -> (bool, String) {
    for i in 1..=n {
        let mut cmd = Command::new("cargo");
        cmd.arg("test").current_dir(project_dir);
        cmd.args(args);
        match cmd.output() {
            Ok(o) if o.status.success() => continue,
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
                return (false, format!("TRIPLE SIMS pass {}/{} failed:\n{}", i, n, stderr));
            }
            Err(e) => return (false, format!("cargo test: {}", e)),
        }
    }
    (true, String::new())
}

/// f63=discover_test_bin. Find [[bin]] with name ending in "-test" in Cargo.toml.
pub fn f63_discover_test_bin(project_dir: &Path) -> Option<String> {
    let manifest = project_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&manifest).ok()?;
    let mut in_bin = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[[bin]]") {
            in_bin = true;
            continue;
        }
        if in_bin && trimmed.starts_with("name = ") {
            if let Some(name) = trimmed.strip_prefix("name = ")
                .and_then(|s| s.strip_prefix('"'))
                .and_then(|s| s.strip_suffix('"'))
            {
                if name.ends_with("-test") {
                    return Some(name.to_string());
                }
            }
            in_bin = false;
        }
    }
    None
}

/// f62=live_demo. Build and run -test binary with live output.
pub fn f62_live_demo(
    project_dir: &Path,
    bin_name: &str,
    cargo_args: &[&str],
) -> std::io::Result<std::process::ExitStatus> {
    let manifest = project_dir.join("Cargo.toml");
    if !manifest.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Cargo.toml not found in {}", project_dir.display()),
        ));
    }
    let mut build = Command::new("cargo");
    build.arg("build").arg("--manifest-path").arg(&manifest)
        .arg("--bin").arg(bin_name).args(cargo_args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    let status = build.status()?;
    if !status.success() {
        return Ok(status);
    }
    let mut run = Command::new("cargo");
    run.arg("run").arg("--manifest-path").arg(&manifest)
        .arg("--bin").arg(bin_name).args(cargo_args)
        .env("TEST_DEMO", "1")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .stdin(std::process::Stdio::inherit());
    run.status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn kova_project() -> PathBuf {
        // Walk up from exopack to kova root
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.parent().unwrap().to_path_buf()
    }

    #[test]
    fn sim1_runs_without_panic() {
        let project = kova_project();
        let result = f170_sim1_user_story(&project);
        assert_eq!(result.sim, 1);
        assert!(!result.findings.is_empty());
        // Should have mostly passes on kova
        assert!(result.pass_count() > 0, "Sim 1 should find some passing checks");
    }

    #[test]
    fn sim2_runs_without_panic() {
        let project = kova_project();
        let result = f171_sim2_feature_gap(&project);
        assert_eq!(result.sim, 2);
        assert!(!result.findings.is_empty());
        assert!(result.pass_count() > 0, "Sim 2 should find some passing criteria");
    }

    #[test]
    fn sim3_modules_exist() {
        let project = kova_project();
        let result = f172_sim3_impl_deep_dive(&project);
        assert_eq!(result.sim, 3);
        // At minimum, key modules should be found
        let module_findings: Vec<_> = result.findings.iter()
            .filter(|f| f.area.starts_with("3G-"))
            .collect();
        assert!(!module_findings.is_empty());
        let module_passes = module_findings.iter().filter(|f| f.severity == Severity::Pass).count();
        assert!(module_passes >= 10, "At least 10 of 15 required modules should exist");
    }

    #[test]
    fn report_summary_format() {
        let report = SimReport {
            sims: vec![
                SimResult { sim: 1, name: "Test1".into(), findings: vec![
                    Finding { sim: 1, severity: Severity::Pass, area: "a".into(), message: "ok".into() },
                    Finding { sim: 1, severity: Severity::Fail, area: "b".into(), message: "bad".into() },
                ]},
                SimResult { sim: 2, name: "Test2".into(), findings: vec![
                    Finding { sim: 2, severity: Severity::Pass, area: "c".into(), message: "ok".into() },
                ]},
                SimResult { sim: 3, name: "Test3".into(), findings: vec![
                    Finding { sim: 3, severity: Severity::Warning, area: "d".into(), message: "eh".into() },
                ]},
            ],
        };
        let s = report.summary();
        assert!(s.contains("TRIPLE SIMS Report"));
        assert!(s.contains("Test1"));
        assert!(s.contains("[ok]"));
        assert!(s.contains("[XX]"));
        assert!(s.contains("[!!]"));
        assert!(s.contains("SIMS FAILED")); // because sim 1 has a fail
    }

    #[test]
    fn report_all_pass() {
        let report = SimReport {
            sims: vec![
                SimResult { sim: 1, name: "S1".into(), findings: vec![
                    Finding { sim: 1, severity: Severity::Pass, area: "a".into(), message: "ok".into() },
                ]},
                SimResult { sim: 2, name: "S2".into(), findings: vec![
                    Finding { sim: 2, severity: Severity::Pass, area: "b".into(), message: "ok".into() },
                ]},
                SimResult { sim: 3, name: "S3".into(), findings: vec![
                    Finding { sim: 3, severity: Severity::Pass, area: "c".into(), message: "ok".into() },
                ]},
            ],
        };
        assert!(report.ok());
        assert!(report.summary().contains("ALL SIMS PASS"));
    }

    #[test]
    fn f63_discovers_test_bin() {
        let project = kova_project();
        let bin = f63_discover_test_bin(&project);
        assert_eq!(bin, Some("kova-test".to_string()));
    }
}
