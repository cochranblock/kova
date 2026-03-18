//! Config. ~/.kova/config.toml. Paths. Bootstrap. Build presets.
//! f78=model_path_for_role, f94-f110, f207-f220

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use std::path::{Path, PathBuf};

#[derive(serde::Deserialize, Default)]
struct ServeSection {
    #[serde(default)]
    bind: Option<String>,
}

#[derive(serde::Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    paths: PathsSection,
    #[serde(default)]
    serve: ServeSection,
    #[serde(default)]
    models: ModelsSection,
    #[serde(default)]
    orchestration: OrchestrationSection,
    #[serde(default)]
    build: BuildSection,
    #[serde(default)]
    cursor: CursorSection,
    #[serde(default)]
    inference: InferenceSection,
    #[serde(default)]
    hive: HiveSection,
}

#[derive(serde::Deserialize, Default)]
struct HiveSection {
    #[serde(default)]
    local_base: Option<String>,
    #[serde(default)]
    shared_base: Option<String>,
}

#[derive(serde::Deserialize, Default)]
struct InferenceSection {
    #[serde(default = "default_model_cache_size")]
    model_cache_size: usize,
    #[serde(default)]
    model_idle_unload_secs: Option<u64>,
    #[serde(default = "default_router_structured")]
    router_structured: bool,
    #[serde(default)]
    code_gen_structured: bool,
}

fn default_router_structured() -> bool {
    true
}

fn default_model_cache_size() -> usize {
    2
}

#[derive(serde::Deserialize)]
struct CursorSection {
    #[serde(default = "default_prompts_enabled")]
    prompts_enabled: bool,
}

impl Default for CursorSection {
    fn default() -> Self {
        Self {
            prompts_enabled: default_prompts_enabled(),
        }
    }
}

fn default_prompts_enabled() -> bool {
    true
}

#[derive(serde::Deserialize, Default)]
struct BuildSection {
    #[serde(default)]
    workspace_root: Option<String>,
    #[serde(default)]
    presets: std::collections::HashMap<String, T88>,
}

/// t88=BuildPreset. Package, target, features for correct cargo invocations.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct T88 {
    pub package: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub target_dir_in_project: bool,
}

#[derive(serde::Deserialize, Default)]
struct PathsSection {
    project: Option<String>,
    /// Root for project discovery. KOVA_PROJECTS_ROOT overrides. Scans workspace members + sibling Cargo.toml dirs.
    #[serde(default)]
    projects_root: Option<String>,
}

#[derive(serde::Deserialize, Default)]
struct ModelsSection {
    router: Option<String>,
    coder: Option<String>,
    fix: Option<String>,
}

#[derive(serde::Deserialize)]
struct OrchestrationSection {
    #[serde(default = "default_router_resident")]
    router_resident: bool,
    #[serde(default = "default_idle_unload")]
    specialist_idle_unload_secs: u64,
    #[serde(default = "default_max_fix_retries")]
    max_fix_retries: u32,
    #[serde(default = "default_run_clippy")]
    run_clippy: bool,
}

impl Default for OrchestrationSection {
    fn default() -> Self {
        Self {
            router_resident: default_router_resident(),
            specialist_idle_unload_secs: default_idle_unload(),
            max_fix_retries: default_max_fix_retries(),
            run_clippy: default_run_clippy(),
        }
    }
}

fn default_router_resident() -> bool {
    true
}
fn default_idle_unload() -> u64 {
    300
}
fn default_max_fix_retries() -> u32 {
    2
}
fn default_run_clippy() -> bool {
    true
}

/// t89=ModelRole. Router, Coder, Fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T89 {
    Router,
    Coder,
    Fix,
}

fn expand_path(s: &str) -> PathBuf {
    let s = s.trim();
    if s == "~" {
        f97()
    } else if let Some(rest) = s.strip_prefix("~/") {
        f97().join(rest)
    } else {
        PathBuf::from(s)
    }
}

fn load_config() -> ConfigFile {
    let path = f98().join("config.toml");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

/// f78=model_path_for_role. Path for a model role. Env > config > default.
pub fn f78(role: T89) -> Option<PathBuf> {
    let env_key = match role {
        T89::Router => "KOVA_MODEL_ROUTER",
        T89::Coder => "KOVA_MODEL_CODER",
        T89::Fix => "KOVA_MODEL_FIX",
    };
    if let Ok(p) = std::env::var(env_key) {
        let path = expand_path(&p);
        if path.exists() {
            return Some(path);
        }
    }
    let cfg = load_config();
    if let Some(name) = match role {
        T89::Router => cfg.models.router.as_deref(),
        T89::Coder => cfg.models.coder.as_deref(),
        T89::Fix => cfg.models.fix.as_deref(),
    } {
        let path = expand_path(name);
        if path.exists() {
            return Some(path);
        }
    }
    let default = f101().join(default_filename(role));
    if default.exists() {
        Some(default)
    } else {
        None
    }
}

fn default_filename(role: T89) -> &'static str {
    match role {
        T89::Router | T89::Fix => "Qwen2.5-Coder-0.5B-Instruct-Q4_K_M.gguf",
        T89::Coder => "Qwen2.5-Coder-0.5B-Instruct-Q4_K_M.gguf",
    }
}

/// f207=orchestration_router_resident
pub fn f207() -> bool {
    load_config().orchestration.router_resident
}

/// f208=orchestration_specialist_idle_unload_secs
pub fn f208() -> u64 {
    load_config().orchestration.specialist_idle_unload_secs
}

/// f209=model_cache_size. Max models in memory. KOVA_MODEL_CACHE_SIZE or config [inference] model_cache_size.
pub fn f209() -> usize {
    std::env::var("KOVA_MODEL_CACHE_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|n: &usize| *n > 0)
        .unwrap_or_else(|| load_config().inference.model_cache_size.max(1))
}

/// f210=code_gen_structured. Grammar-constrained output for Coder. Experimental. Config [inference] code_gen_structured.
pub fn f210() -> bool {
    std::env::var("KOVA_CODE_GEN_STRUCTURED")
        .ok()
        .map(|s| s != "0" && s != "false" && s != "no")
        .unwrap_or_else(|| load_config().inference.code_gen_structured)
}

/// f211=router_structured. Grammar-constrained output for Router. Config [inference] router_structured.
pub fn f211() -> bool {
    std::env::var("KOVA_ROUTER_STRUCTURED")
        .ok()
        .map(|s| s != "0" && s != "false" && s != "no")
        .unwrap_or_else(|| load_config().inference.router_structured)
}

/// f212=model_idle_unload_secs. Secs before evicting. Config [inference] model_idle_unload_secs or orchestration.
pub fn f212() -> u64 {
    load_config()
        .inference
        .model_idle_unload_secs
        .unwrap_or_else(f208)
}

/// f213=orchestration_max_fix_retries
pub fn f213() -> u32 {
    load_config().orchestration.max_fix_retries
}

/// f214=orchestration_run_clippy
pub fn f214() -> bool {
    load_config().orchestration.run_clippy
}

/// f215=cursor_prompts_enabled. When false, no Cursor rules/skills injected.
pub fn f215() -> bool {
    load_config().cursor.prompts_enabled
}

/// f216=ollama_url. OLLAMA_HOST or http://localhost:11434.
pub fn f216() -> String {
    std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
}

/// f217=default_model. Default model for review/feedback. OLLAMA_MODEL or qwen2.5-coder:1.5b.
pub fn f217() -> String {
    std::env::var("OLLAMA_MODEL")
        .unwrap_or_else(|_| "qwen2.5-coder:1.5b".to_string())
}

/// f97=home. HOME env or /.
pub fn f97() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

/// f98=kova_dir. ~/.kova.
pub fn f98() -> PathBuf {
    f97().join(".kova")
}

/// f218=hive_local_base. Hive paths from config. Fallback to defaults if not set.
pub fn f218() -> String {
    load_config()
        .hive
        .local_base
        .unwrap_or_else(|| "/tmp/hive-build".to_string())
}

/// f219=hive_shared_base. Hive shared (NFS) base path.
pub fn f219() -> String {
    load_config()
        .hive
        .shared_base
        .unwrap_or_else(|| "/mnt/hive".to_string())
}

/// f99=prompts_dir. ~/.kova/prompts.
pub fn f99() -> PathBuf {
    f98().join("prompts")
}

/// f100=sled_path. ~/.kova/sled.db.
pub fn f100() -> PathBuf {
    f98().join("sled.db")
}

/// f101=models_dir. ~/.kova/models.
pub fn f101() -> PathBuf {
    f98().join("models")
}

/// f102=inference_model_path. Model path for inference. KOVA_INFERENCE_MODEL or f78(Coder) or default.
pub fn f102() -> Option<PathBuf> {
    std::env::var("KOVA_INFERENCE_MODEL")
        .ok()
        .map(|s| expand_path(&s))
        .filter(|p| p.exists())
        .or_else(|| f78(T89::Coder))
}

/// f94=default_project. Default project for execution. KOVA_PROJECT > config [paths] project > cwd.
pub fn f94() -> PathBuf {
    if let Ok(p) = std::env::var("KOVA_PROJECT") {
        let path = expand_path(&p);
        if path.exists() {
            return path;
        }
    }
    if let Some(name) = load_config().paths.project.as_deref() {
        let path = expand_path(name);
        if path.exists() {
            return path;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| f97())
}

/// f96=projects_root. Root for project discovery. KOVA_PROJECTS_ROOT > config [paths] projects_root > home.
fn f96() -> PathBuf {
    if let Ok(p) = std::env::var("KOVA_PROJECTS_ROOT") {
        let path = expand_path(&p);
        if path.exists() {
            return path;
        }
    }
    if let Some(name) = load_config().paths.projects_root.as_deref() {
        let path = expand_path(name);
        if path.exists() {
            return path;
        }
    }
    f97()
}

/// f95=discover_projects. Discover all projects under projects_root. Workspace members + sibling dirs with Cargo.toml. Sorted by name.
pub fn f95() -> Vec<PathBuf> {
    let root = f96();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();

    // 1. Parse root Cargo.toml for [workspace] members
    let manifest = root.join("Cargo.toml");
    if manifest.exists()
        && let Ok(content) = std::fs::read_to_string(&manifest)
        && let Ok(toml) = content.parse::<toml::Value>()
        && let Some(ws) = toml.get("workspace")
        && let Some(members) = ws.get("members")
    {
        let members = match members {
                            toml::Value::Array(a) => a
                                .iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect::<Vec<_>>(),
                            toml::Value::String(s) => vec![s.clone()],
                            _ => vec![],
                        };
        for m in members {
            let full = root.join(&m);
            if full.exists()
                && full.join("Cargo.toml").exists()
                && seen.insert(full.clone())
            {
                out.push(full);
            }
        }
    }

    // 2. Fast scan: use `ls` to get directory names, then check for Cargo.toml.
    // Avoids slow read_dir + stat on large home directories.
    if let Ok(ls_out) = std::process::Command::new("ls")
        .arg("-1")
        .current_dir(&root)
        .output()
        && ls_out.status.success()
    {
        let names = String::from_utf8_lossy(&ls_out.stdout);
        for name in names.lines() {
            if name.starts_with('.') || name == "target" || name == "vendor" {
                continue;
            }
            let path = root.join(name);
            if path.join("Cargo.toml").exists() && seen.insert(path.clone()) {
                out.push(path);
            }
        }
    }

    out.sort_by(|a, b| {
        a.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .cmp(b.file_name().and_then(|n| n.to_str()).unwrap_or(""))
    });
    out
}

/// f103=backlog_path. KOVA_BACKLOG_PATH or ~/.kova/backlog.json.
pub fn f103() -> PathBuf {
    std::env::var("KOVA_BACKLOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| f98().join("backlog.json"))
}

/// f104=workspace_root. Workspace root for builds. KOVA_WORKSPACE_ROOT > config [build] workspace_root > detect from project.
pub fn f104(project: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("KOVA_WORKSPACE_ROOT") {
        let path = expand_path(&p);
        if path.exists() {
            return path;
        }
    }
    if let Some(s) = load_config().build.workspace_root.as_deref() {
        let path = expand_path(s);
        if path.exists() {
            return path;
        }
    }
    workspace_root_from_project(project)
}

/// Walk up from project to find Cargo.toml with [workspace]. Fallback: project.
fn workspace_root_from_project(project: &Path) -> PathBuf {
    let mut dir = project.to_path_buf();
    if dir.is_file() {
        dir = dir.parent().unwrap_or(&dir).to_path_buf();
    }
    while !dir.as_os_str().is_empty() {
        let manifest = dir.join("Cargo.toml");
        if manifest.exists()
            && let Ok(content) = std::fs::read_to_string(&manifest)
            && content.contains("[workspace]")
        {
            return dir;
        }
        dir = match dir.parent() {
            Some(p) => p.to_path_buf(),
            None => break,
        };
    }
    project.to_path_buf()
}

/// f105=load_build_preset. Load build preset by project name (e.g. "oakilydokily", "mural-wasm").
pub fn f105(project_name: &str) -> Option<T88> {
    load_config().build.presets.get(project_name).cloned()
}

/// f106=all_build_presets. All build presets for API.
pub fn f106() -> std::collections::HashMap<String, T88> {
    load_config().build.presets
}

/// f107=infer_preset_name. Infer preset name from project path. e.g. .../oakilydokily -> "oakilydokily".
pub fn f107(project: &Path) -> Option<String> {
    let name = project.file_name()?.to_str()?;
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    Some(name.to_string())
}

/// Type aliases for ergonomics. Token names: T88, T89.
pub use T88 as BuildPreset;
pub use T89 as ModelRole;

/// Human-readable aliases for tokenized functions.
pub use f94 as default_project;
pub use f95 as discover_projects;
pub use f97 as home;
pub use f98 as kova_dir;
pub use f99 as prompts_dir;
pub use f100 as sled_path;
pub use f101 as models_dir;
pub use f102 as inference_model_path;
pub use f103 as backlog_path;
pub use f104 as workspace_root;
pub use f105 as load_build_preset;
pub use f106 as all_build_presets;
pub use f107 as infer_preset_name;
pub use f108 as bind_addr;
pub use f109 as bootstrap;
pub use f110 as load_prompt;
pub use f207 as orchestration_router_resident;
pub use f208 as orchestration_specialist_idle_unload_secs;
pub use f209 as model_cache_size;
pub use f210 as code_gen_structured;
pub use f211 as router_structured;
pub use f212 as model_idle_unload_secs;
pub use f213 as orchestration_max_fix_retries;
pub use f214 as orchestration_run_clippy;
pub use f215 as cursor_prompts_enabled;
pub use f216 as ollama_url;
pub use f217 as default_model;
pub use f218 as hive_local_base;
pub use f219 as hive_shared_base;
pub use f220 as fast_localhost;

/// f108=bind_addr. KOVA_BIND or config [serve] bind or 127.0.0.1:3002.
pub fn f108() -> std::net::SocketAddr {
    std::env::var("KOVA_BIND")
        .ok()
        .or_else(|| load_config().serve.bind.clone())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::net::SocketAddr::from(([127, 0, 0, 1], 3002u16)))
}

/// f220=fast_localhost. Skip TLS when binding to loopback. Localhost = no TLS = fastest path.
/// Set KOVA_FAST_LOCALHOST=false to disable (e.g. when adding TLS for remote).
pub fn f220() -> bool {
    std::env::var("KOVA_FAST_LOCALHOST")
        .ok()
        .map(|s| s != "0" && s != "false" && s != "no")
        .unwrap_or(true)
}

/// f109=bootstrap. Bootstrap ~/.kova, prompts/, config.toml, default prompts.
pub fn f109() -> anyhow::Result<()> {
    let dir = f98();
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(f99())?;
    std::fs::create_dir_all(f101())?;

    let config_path = dir.join("config.toml");
    if !config_path.exists() {
        let default = r#"[paths]
backlog = "~/.kova/backlog.json"
# project = "~/my-project"  # default project for code gen; KOVA_PROJECT overrides
# projects_root = "~"  # root for discovery; KOVA_PROJECTS_ROOT overrides. Scans workspace + sibling Cargo.toml dirs.

[serve]
bind = "127.0.0.1:3002"

[models]
router = "~/.kova/models/Qwen2.5-Coder-0.5B-Instruct-Q4_K_M.gguf"
coder = "~/.kova/models/Qwen2.5-Coder-0.5B-Instruct-Q4_K_M.gguf"
fix = "~/.kova/models/Qwen2.5-Coder-0.5B-Instruct-Q4_K_M.gguf"

[orchestration]
router_resident = true
specialist_idle_unload_secs = 300
# max_fix_retries: DDI (Debugging Decay Index) — fix loop loses effectiveness after 2–3 attempts
max_fix_retries = 2
run_clippy = true

[cursor]
prompts_enabled = true

[inference]
# model_cache_size = 2        # Max models in memory
# model_idle_unload_secs = 300  # Unload after N sec idle
# router_structured = true    # Use grammar for Router (Phase 2)
# code_gen_structured = false # Experimental: structured code output

[build]
# workspace_root = "~"  # KOVA_WORKSPACE_ROOT overrides

[hive]
# local_base = "/tmp/hive-build"   # Sync-to-local path on workers
# shared_base = "/mnt/hive"        # NFS path on workers

[build.presets.oakilydokily]
package = "oakilydokily"
features = ["approuter"]

[build.presets.mural-wasm]
package = "mural-wasm"
target = "wasm32-unknown-unknown"

[build.presets.kova]
package = "kova"
features = ["serve"]
target = "aarch64-apple-darwin"
target_dir_in_project = true

[build.presets.approuter]
package = "approuter"

[build.presets.cochranblock]
package = "cochranblock"

[build.presets.rogue-repo]
package = "rogue-repo"
"#;
        std::fs::write(&config_path, default)?;
    }

    let prompts = f99();
    let system_path = prompts.join("system.md");
    if !system_path.exists() {
        let default = r#"# System prompt

You are Kova, an augment engine. Execute intent. Build fast, ship fast.

## Style (P12)
- Voice: short, active, concrete
- Avoid: utilize, leverage, facilitate, enhance, optimize, comprehensive, holistic, robust, seamlessly, empower, streamline, synergy, paradigm
- Use: use, apply, let, enable, improve, tune, full, drop, simplify

## Principles
- Intent-first. Why not how.
- Verify. No fire-and-forget.
- Project separation. One rebuild at a time.
"#;
        std::fs::write(&system_path, default)?;
    }

    let persona_path = prompts.join("persona.md");
    if !persona_path.exists() {
        let default = r#"# Persona

You are a Senior Systems Architect. Execute my intent. Direct, precise, efficient.
"#;
        std::fs::write(&persona_path, default)?;
    }

    let backlog_path = dir.join("backlog.json");
    if !backlog_path.exists() {
        std::fs::write(&backlog_path, r#"{"items":[]}"#)?;
    }

    Ok(())
}

/// f110=load_prompt
pub fn f110(name: &str) -> String {
    let path = f99().join(format!("{}.md", name));
    std::fs::read_to_string(&path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// f78=model_path_for_role
    #[test]
    fn model_role_default_filename() {
        assert_eq!(
            default_filename(T89::Router),
            "Qwen2.5-Coder-0.5B-Instruct-Q4_K_M.gguf"
        );
        assert_eq!(
            default_filename(T89::Coder),
            "Qwen2.5-Coder-0.5B-Instruct-Q4_K_M.gguf"
        );
        assert_eq!(
            default_filename(T89::Fix),
            "Qwen2.5-Coder-0.5B-Instruct-Q4_K_M.gguf"
        );
    }

    /// expand_path (f97 home)
    #[test]
    fn expand_path_tilde() {
        let h = f97();
        let p = super::expand_path("~/foo/bar");
        assert_eq!(p, h.join("foo/bar"));
    }
}