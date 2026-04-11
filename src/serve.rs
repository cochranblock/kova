//! HTTP API. kova serve. POST /api/intent, GET /ws/stream (WebSocket). Web GUI at /.
//! f114=serve_run

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::CorsLayer;

/// t92=T92. Shared state: pipeline broadcast receiver for WebSocket clients.
#[derive(Clone)]
pub struct T92 {
    #[cfg(feature = "inference")]
    pipeline_rx: Arc<Mutex<Option<broadcast::Receiver<Arc<str>>>>>,
    #[cfg(feature = "inference")]
    last_trace: Arc<Mutex<Option<crate::trace::T93>>>,
}

impl Default for T92 {
    fn default() -> Self {
        Self {
            #[cfg(feature = "inference")]
            pipeline_rx: Arc::new(Mutex::new(None)),
            #[cfg(feature = "inference")]
            last_trace: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Serialize)]
struct Status {
    status: &'static str,
}

#[derive(Deserialize)]
struct RecentQuery {
    project: Option<PathBuf>,
    #[serde(default = "default_minutes")]
    minutes: u64,
}

fn default_minutes() -> u64 {
    30
}

#[derive(Deserialize)]
struct BuildCommandQuery {
    project: String,
    #[serde(default)]
    release: bool,
}

#[derive(Serialize)]
struct BuildCommandResponse {
    command: String,
}

#[derive(Deserialize)]
struct FileQuery {
    hint: Option<String>,
}

#[derive(Deserialize)]
struct DiffBody {
    hint: Option<String>,
    #[serde(rename = "new_content")]
    new_content: Option<String>,
}

#[derive(Deserialize)]
struct ApplyBody {
    hint: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize)]
struct BacklogRunBody {
    index: usize,
}

#[derive(Deserialize)]
struct TestRunQuery {
    #[serde(default)]
    project: String,
    #[serde(default)]
    node: Option<String>,
}

#[derive(Serialize)]
struct TestRunResponse {
    ok: bool,
    node: String,
    output: String,
}

async fn serve_index() -> impl IntoResponse {
    let html = include_str!("../wasm/dist/index.html");
    Html(html)
}

async fn serve_js() -> impl IntoResponse {
    let js = include_str!("../wasm/dist/kova_web.js");
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript")
        .body(Body::from(js))
        .unwrap()
}

async fn serve_wasm() -> impl IntoResponse {
    let wasm = include_bytes!("../wasm/dist/kova_web_bg.wasm");
    Response::builder()
        .header(header::CONTENT_TYPE, "application/wasm")
        .body(Body::from(wasm.as_slice()))
        .unwrap()
}

async fn api_webhook_github() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"received": true}))).into_response()
}

// ── OpenAI-compatible inference endpoints ────────────────────────
// Lets kova act as an inference server on bare metal nodes.
// T193 routes T129::OpenAiCompat requests here.

#[derive(Deserialize)]
#[allow(dead_code)]
struct OaiChatRequest {
    #[serde(default)]
    model: String,
    messages: Vec<OaiMessage>,
    #[serde(default = "default_temperature")]
    temperature: f32,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    max_tokens: Option<u32>,
}

fn default_temperature() -> f32 {
    0.2
}

#[derive(Deserialize, Serialize, Clone)]
struct OaiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OaiChatResponse {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: Vec<OaiChatChoice>,
    usage: OaiUsageOut,
}

#[derive(Serialize)]
struct OaiChatChoice {
    index: u32,
    message: OaiMessage,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct OaiUsageOut {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

#[derive(Serialize)]
struct OaiModelsResponse {
    object: &'static str,
    data: Vec<OaiModelEntry>,
}

#[derive(Serialize)]
struct OaiModelEntry {
    id: String,
    object: &'static str,
    owned_by: &'static str,
}

#[cfg(feature = "inference")]
async fn v1_chat_completions(Json(req): Json<OaiChatRequest>) -> impl IntoResponse {
    let model_path = match crate::config::inference_model_path() {
        Some(p) if p.exists() => p,
        _ => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": {"message": "no model loaded", "type": "server_error"}})),
            ).into_response()
        }
    };

    // Extract system prompt and user message from messages array
    let system = req.messages.iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let user = req.messages.iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let t0 = std::time::Instant::now();
    let system_len = system.len();
    let user_len = user.len();
    let result = tokio::task::spawn_blocking(move || {
        crate::inference::f80(&model_path, &system, &user)
            .map_err(|e| format!("inference: {}", e))
    }).await;

    let text = match result {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": {"message": e, "type": "server_error"}})),
            ).into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": {"message": format!("join: {}", e), "type": "server_error"}})),
            ).into_response()
        }
    };

    let elapsed = t0.elapsed();
    let est_completion = (text.len() / 4) as u64;
    let est_prompt = (system_len + user_len) / 4;
    let model_name = req.model.clone();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let resp = OaiChatResponse {
        id: format!("kova-{}", now),
        object: "chat.completion",
        created: now,
        model: model_name,
        choices: vec![OaiChatChoice {
            index: 0,
            message: OaiMessage {
                role: "assistant".into(),
                content: text,
            },
            finish_reason: "stop",
        }],
        usage: OaiUsageOut {
            prompt_tokens: est_prompt as u64,
            completion_tokens: est_completion,
            total_tokens: est_prompt as u64 + est_completion,
        },
    };

    eprintln!("[v1] {}ms, ~{} tokens", elapsed.as_millis(), est_completion);
    (StatusCode::OK, Json(resp)).into_response()
}

#[cfg(not(feature = "inference"))]
async fn v1_chat_completions(Json(_req): Json<OaiChatRequest>) -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error": {"message": "build with --features inference", "type": "server_error"}})),
    ).into_response()
}

async fn v1_models() -> impl IntoResponse {
    let mut models = Vec::new();

    // List GGUF models from the models directory
    if let Some(model_path) = crate::config::inference_model_path()
        && let Some(dir) = model_path.parent()
        && let Ok(entries) = std::fs::read_dir(dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("gguf")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
            {
                models.push(OaiModelEntry {
                    id: name.to_string(),
                    object: "model",
                    owned_by: "kova",
                });
            }
        }
    }

    // Always report at least one model if inference is available
    if models.is_empty() {
        models.push(OaiModelEntry {
            id: "kova-local".into(),
            object: "model",
            owned_by: "kova",
        });
    }

    Json(OaiModelsResponse {
        object: "list",
        data: models,
    })
}

async fn api_test_run(Query(q): Query<TestRunQuery>) -> impl IntoResponse {
    let project = if q.project.is_empty() {
        "cochranblock"
    } else {
        q.project.as_str()
    };
    let nodes: Vec<String> = if let Some(ref n) = q.node {
        vec![n.clone()]
    } else {
        let all: Vec<String> = crate::c2::f350()
            .into_iter()
            .map(String::from)
            .collect();
        match crate::node_cmd::pick_idlest(&all) {
            Some(host) => vec![host],
            None => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(TestRunResponse {
                        ok: false,
                        node: String::new(),
                        output: "No reachable nodes".into(),
                    }),
                )
                    .into_response()
            }
        }
    };
    let results = crate::node_cmd::f133_sync(&nodes, project);
    let (ok, node, output) = results
        .first()
        .map(|r| {
            (
                r.s15,
                r.s14.clone(),
                r.s16
                    .iter()
                    .find(|(k, _)| *k == "o11")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default(),
            )
        })
        .unwrap_or((false, String::new(), "no results".into()));
    (StatusCode::OK, Json(TestRunResponse { ok, node, output })).into_response()
}

/// Validate hint is a safe filename: word.rs only. Returns hint or "lib.rs".
fn safe_hint(hint: Option<&str>) -> String {
    let h = hint.unwrap_or("lib.rs").trim();
    if let Some(stem) = h.strip_suffix(".rs")
        && !stem.is_empty()
        && stem.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        return h.to_string();
    }
    "lib.rs".to_string()
}

async fn api_projects() -> Json<Vec<String>> {
    let projects = tokio::task::spawn_blocking(crate::discover_projects)
        .await
        .unwrap_or_default();
    Json(
        projects
            .iter()
            .filter_map(|p| p.to_str().map(String::from))
            .collect(),
    )
}

async fn api_prompts() -> impl IntoResponse {
    let system = crate::load_prompt("system");
    let persona = crate::load_prompt("persona");
    (
        StatusCode::OK,
        Json(serde_json::json!({"system": system, "persona": persona})),
    )
        .into_response()
}

async fn recent_changes(Query(q): Query<RecentQuery>) -> String {
    let project = q
        .project
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let changes =
        crate::recent_changes::f86(&project, std::time::Duration::from_secs(q.minutes * 60));
    crate::recent_changes::f87(&changes)
}

async fn build_presets() -> Json<std::collections::HashMap<String, crate::BuildPreset>> {
    Json(crate::all_build_presets())
}

#[cfg(feature = "inference")]
async fn api_route(Json(req): Json<RouteRequest>) -> impl IntoResponse {
    let router_path = match crate::f78(crate::ModelRole::Router) {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(RouteResponse {
                    classification: String::new(),
                    needs_clarification: None,
                    suggested_question: None,
                    choices: None,
                    enriched_message: None,
                    error: Some("No router model. Run: kova model install".into()),
                }),
            )
                .into_response()
        }
    };
    let to_route = if let Some(ref prior) = req.prior_message {
        let reply = req.message.trim();
        if reply.contains(".rs") {
            format!("{} in {}", prior, reply)
        } else {
            format!("{} {}", prior, reply)
        }
    } else {
        req.message.clone()
    };
    let path = router_path.clone();
    let msg = to_route.clone();
    let prior = req.prior_message.clone();
    let result = tokio::task::spawn_blocking(move || {
        let rx = crate::f79(&path, &msg);
        rx.recv().ok()
    })
    .await
    .ok()
    .flatten();
    let Some(result) = result else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RouteResponse {
                classification: String::new(),
                needs_clarification: None,
                suggested_question: None,
                choices: None,
                enriched_message: None,
                error: Some("Router did not respond".into()),
            }),
        )
            .into_response();
    };
    let (classification, needs_clarification, suggested_question, choices, _) = match &result {
        crate::T94::CodeGen => ("code_gen".into(), None, None, None, None::<String>),
        crate::T94::Refactor => ("refactor".into(), None, None, None, None::<String>),
        crate::T94::Explain => ("explain".into(), None, None, None, None::<String>),
        crate::T94::Fix => ("fix".into(), None, None, None, None::<String>),
        crate::T94::Run => ("run".into(), None, None, None, None::<String>),
        crate::T94::Custom => ("custom".into(), None, None, None, None::<String>),
        crate::T94::NeedsClarification { .. } => {
            let q = result.f363(&req.message);
            let ch = result.f364().map(|s| s.to_vec());
            (
                "needs_clarification".into(),
                Some(true),
                Some(q),
                ch,
                None::<String>,
            )
        }
        crate::T94::Error(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RouteResponse {
                    classification: "error".into(),
                    needs_clarification: None,
                    suggested_question: None,
                    choices: None,
                    enriched_message: None,
                    error: Some(e.clone()),
                }),
            )
                .into_response();
        }
    };
    let enriched =
        if prior.is_some() && !matches!(&result, crate::T94::NeedsClarification { .. }) {
            Some(to_route)
        } else {
            None
        };
    (
        StatusCode::OK,
        Json(RouteResponse {
            classification,
            needs_clarification,
            suggested_question,
            choices,
            enriched_message: enriched,
            error: None,
        }),
    )
        .into_response()
}

#[derive(Serialize)]
struct IntentResponse {
    accepted: bool,
    f325: String,
    /// When FullPipeline runs plan path (no inference), summary of results.
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RouteRequest {
    message: String,
    #[serde(rename = "prior_message")]
    prior_message: Option<String>,
    #[allow(dead_code)]
    project: Option<String>,
}

#[derive(Serialize)]
struct RouteResponse {
    classification: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    needs_clarification: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    choices: Option<Vec<String>>,
    /// When prior_message was provided and we routed on enriched context.
    #[serde(skip_serializing_if = "Option::is_none")]
    enriched_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize)]
struct IntentRequest {
    #[serde(flatten)]
    intent: crate::t0,
    /// Override project path (default: server default_project)
    project: Option<String>,
}

async fn api_intent(
    State(state): State<T92>,
    Json(req): Json<IntentRequest>,
) -> Json<IntentResponse> {
    let intent = req.intent;
    let name = crate::f325(&intent.s0);
    let mut summary = None;
    if matches!(intent.s0, crate::t1::FullPipeline) {
        #[cfg(feature = "inference")]
        {
            if let (Some(coder), Some(fix)) = (
                crate::f78(crate::ModelRole::Coder),
                crate::f78(crate::ModelRole::Fix).or_else(|| crate::f78(crate::ModelRole::Coder)),
            ) {
                let project = req
                    .project
                    .as_ref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(crate::default_project);
                let system = crate::load_prompt("system");
                let persona = crate::load_prompt("persona");
                let cursor = crate::cursor_prompts::f111(&project);
                let system_prompt = if cursor.is_empty() {
                    format!("{}\n\n{}", system, persona)
                } else {
                    format!(
                        "{}\n\n{}\n\n--- Cursor rules ---\n{}",
                        system, persona, cursor
                    )
                };
                let user_msg = intent.s1.as_deref().unwrap_or("Generate code.");
                let rx = crate::pipeline::f81(
                    &coder,
                    &fix,
                    &system_prompt,
                    user_msg,
                    &project,
                    crate::orchestration_max_fix_retries(),
                    crate::orchestration_run_clippy(),
                    Some(state.last_trace.clone()),
                );
                let mut guard = state.pipeline_rx.lock().await;
                *guard = Some(rx);
                return Json(IntentResponse {
                    accepted: true,
                    f325: name.to_string(),
                    summary: None,
                });
            }
        }
        // Fallback: no inference — run plan path (cargo check, cargo test)
        let project = req
            .project
            .as_ref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(crate::default_project);
        let approuter_dir = std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join("approuter"));
        let plan = crate::t3::f14(&intent, project, approuter_dir);
        let exec = crate::t6;
        if let Ok(results) = tokio::task::block_in_place(|| exec.f15(&plan)) {
            summary = Some(
                results
                    .iter()
                    .map(|r| format!("{} {} {}", if r.s11 { "✓" } else { "✗" }, r.s10, r.s13))
                    .collect(),
            );
        }
    }
    Json(IntentResponse {
        accepted: true,
        f325: name.to_string(),
        summary,
    })
}

async fn ws_stream(
    State(state): State<T92>,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| ws_handler(state, socket))
}

async fn ws_handler(state: T92, mut socket: WebSocket) {
    #[cfg(feature = "inference")]
    {
        let mut rx_opt = state.pipeline_rx.lock().await;
        if let Some(mut rx) = rx_opt.take() {
            drop(rx_opt);
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        if socket
                            .send(Message::Text((*msg).to_string()))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            return;
        }
    }
    for chunk in ["Mock ", "stream ", "data.\n"].iter() {
        if socket.send(Message::Text(chunk.to_string())).await.is_err() {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

#[cfg(feature = "inference")]
async fn api_explain(State(state): State<T92>) -> impl IntoResponse {
    let guard = state.last_trace.lock().await;
    match guard.as_ref() {
        Some(t) => Json(serde_json::to_value(t).unwrap_or_default()).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"no trace"})),
        )
            .into_response(),
    }
}

async fn api_file(Query(q): Query<FileQuery>) -> impl IntoResponse {
    let project = crate::default_project();
    let hint = safe_hint(q.hint.as_deref());
    let target = crate::output::f85(&project, Some(&hint));
    match tokio::fs::read_to_string(&target).await {
        Ok(s) => (StatusCode::OK, s).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            format!("File not found: {}", target.display()),
        )
            .into_response(),
    }
}

async fn api_diff(Json(body): Json<DiffBody>) -> impl IntoResponse {
    let project = crate::default_project();
    let hint = safe_hint(body.hint.as_deref());
    let target = crate::output::f85(&project, Some(&hint));
    let new_content = body.new_content.as_deref().unwrap_or("");
    let current = tokio::fs::read_to_string(&target).await.unwrap_or_default();
    let diff = crate::output::f84(&current, new_content);
    (StatusCode::OK, diff).into_response()
}

async fn api_apply(Json(body): Json<ApplyBody>) -> impl IntoResponse {
    let project = crate::default_project();
    let hint = safe_hint(body.hint.as_deref());
    let target = crate::output::f85(&project, Some(&hint));
    let content = body.content.as_deref().unwrap_or("");
    if let Err(e) = tokio::fs::create_dir_all(target.parent().unwrap_or(&project)).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("create dir: {}", e)})),
        )
            .into_response();
    }
    match tokio::fs::write(&target, content).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "path": target.display().to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("write: {}", e)})),
        )
            .into_response(),
    }
}

async fn api_backlog_get() -> impl IntoResponse {
    let path = crate::backlog_path();
    match crate::f25(&path) {
        Ok(b) => (StatusCode::OK, Json(b)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_backlog_run(
    State(state): State<T92>,
    Json(body): Json<BacklogRunBody>,
) -> impl IntoResponse {
    let path = crate::backlog_path();
    let backlog = match crate::f25(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    let entry = match backlog.items.get(body.index) {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "index out of range"})),
            )
                .into_response()
        }
    };
    let intent = match crate::f293(&entry) {
        Some(i) => i,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "unsupported intent"})),
            )
                .into_response()
        }
    };
    let project = entry
        .project
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(crate::default_project);
    let approuter_dir = entry
        .approuter_dir
        .as_ref()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| std::path::PathBuf::from(h).join("approuter"))
        });

    if matches!(intent.s0, crate::t1::FullPipeline) {
        #[cfg(feature = "inference")]
        {
            if let (Some(coder), Some(fix)) = (
                crate::f78(crate::ModelRole::Coder),
                crate::f78(crate::ModelRole::Fix).or_else(|| crate::f78(crate::ModelRole::Coder)),
            ) {
                let system = crate::load_prompt("system");
                let persona = crate::load_prompt("persona");
                let cursor = crate::cursor_prompts::f111(&project);
                let system_prompt = if cursor.is_empty() {
                    format!("{}\n\n{}", system, persona)
                } else {
                    format!(
                        "{}\n\n{}\n\n--- Cursor rules ---\n{}",
                        system, persona, cursor
                    )
                };
                let user_msg = intent.s1.as_deref().unwrap_or("Generate code.");
                let rx = crate::pipeline::f81(
                    &coder,
                    &fix,
                    &system_prompt,
                    user_msg,
                    &project,
                    crate::orchestration_max_fix_retries(),
                    crate::orchestration_run_clippy(),
                    Some(state.last_trace.clone()),
                );
                {
                    let mut guard = state.pipeline_rx.lock().await;
                    *guard = Some(rx);
                }
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({"message": "Pipeline started.", "stream": true})),
                )
                    .into_response();
            }
        }
        // Fallback: no inference — run plan path (cargo check, cargo test)
        let plan = crate::t3::f14(&intent, project.clone(), approuter_dir);
        let exec = crate::t6;
        match tokio::task::block_in_place(|| exec.f15(&plan)) {
            Ok(results) => {
                let summary: Vec<String> = results
                    .iter()
                    .map(|r| format!("{} {} {}", if r.s11 { "✓" } else { "✗" }, r.s10, r.s13))
                    .collect();
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({"message": "Done.", "summary": summary})),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
                    .into_response();
            }
        }
    }
    let plan = crate::t3::f14(&intent, project.clone(), approuter_dir);
    let exec = crate::t6;
    match exec.f15(&plan) {
        Ok(results) => {
            let summary: Vec<String> = results
                .iter()
                .map(|r| format!("{} {} {}", if r.s11 { "✓" } else { "✗" }, r.s10, r.s13))
                .collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({"message": "Done.", "summary": summary})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_backlog_post(Json(entry): Json<crate::t8>) -> impl IntoResponse {
    let path = crate::backlog_path();
    let mut backlog = crate::f25(&path).unwrap_or_default();
    backlog.items.push(entry);
    let json = match serde_json::to_string_pretty(&backlog) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    match std::fs::write(&path, json) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[cfg(feature = "inference")]
async fn api_explain_run(State(state): State<T92>) -> impl IntoResponse {
    let trace = state.last_trace.lock().await.clone();
    let trace = match trace {
        Some(t) => t,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error":"No trace. Run a pipeline first."})),
            )
                .into_response()
        }
    };
    let model = match crate::f78(crate::ModelRole::Coder) {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"no model"})),
            )
                .into_response()
        }
    };
    match crate::academy::explain_trace(&trace, &model).await {
        Ok(s) => (StatusCode::OK, Json(serde_json::json!({"explanation": s}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

// ── MoE endpoint ────────────────────────────────────────────────

#[cfg(feature = "inference")]
#[derive(Deserialize)]
struct MoeRunRequest {
    prompt: String,
    #[serde(default = "default_num_experts")]
    num_experts: usize,
    #[serde(default = "default_true")]
    run_clippy: bool,
    #[serde(default = "default_true")]
    run_tests: bool,
    #[serde(default = "default_true")]
    run_review: bool,
    #[serde(default)]
    save_winner: bool,
}

#[cfg(feature = "inference")]
fn default_num_experts() -> usize { 3 }
fn default_true() -> bool { true }

#[cfg(feature = "inference")]
#[derive(Serialize)]
struct MoeVariantResponse {
    node_id: String,
    code: String,
    gen_ms: u64,
    compile_ok: bool,
    clippy_ok: bool,
    tests_ok: bool,
    compile_ms: u64,
    review_score: Option<u8>,
    total_score: u32,
}

#[cfg(feature = "inference")]
#[derive(Serialize)]
struct MoeRunResponse {
    variants: Vec<MoeVariantResponse>,
    winner: Option<MoeVariantResponse>,
    prompt: String,
}

#[cfg(feature = "inference")]
async fn api_moe_run(Json(req): Json<MoeRunRequest>) -> impl IntoResponse {
    let config = crate::moe::T196 {
        num_experts: req.num_experts,
        run_clippy: req.run_clippy,
        run_tests: req.run_tests,
        run_review: req.run_review,
        num_ctx: 8192,
        save_winner: req.save_winner,
    };

    // Run MoE on a blocking thread (it does SSH + cargo).
    let prompt = req.prompt.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::moe::f341(&prompt, config)
    })
    .await;

    match result {
        Ok(report) => {
            let variants: Vec<MoeVariantResponse> = report
                .variants
                .iter()
                .map(|v| MoeVariantResponse {
                    node_id: v.node_id.clone(),
                    code: v.code.clone(),
                    gen_ms: v.gen_ms,
                    compile_ok: v.compile_ok,
                    clippy_ok: v.clippy_ok,
                    tests_ok: v.tests_ok,
                    compile_ms: v.compile_ms,
                    review_score: v.review_score,
                    total_score: v.total_score,
                })
                .collect();

            let winner = report.winner.map(|i| {
                let v = &report.variants[i];
                MoeVariantResponse {
                    node_id: v.node_id.clone(),
                    code: v.code.clone(),
                    gen_ms: v.gen_ms,
                    compile_ok: v.compile_ok,
                    clippy_ok: v.clippy_ok,
                    tests_ok: v.tests_ok,
                    compile_ms: v.compile_ms,
                    review_score: v.review_score,
                    total_score: v.total_score,
                }
            });

            (StatusCode::OK, Json(MoeRunResponse {
                variants,
                winner,
                prompt: report.prompt,
            })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("moe: {}", e)})),
        ).into_response(),
    }
}

async fn api_demo_record(Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed");
    let safe = name.replace(['/', '\\', ':', ' '], "_");
    let demo_dir = std::env::var("KOVA_DEMO_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".kova")
                .join("demos")
        });
    if let Err(e) = std::fs::create_dir_all(&demo_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }
    let path = demo_dir.join(format!("{}.json", safe));
    let json = match serde_json::to_string_pretty(&payload) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    match std::fs::write(&path, json) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "path": path.display().to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

fn app_router() -> Router<T92> {
    let r = Router::new()
        .route("/", get(serve_index))
        .route("/kova_web.js", get(serve_js))
        .route("/kova_web_bg.wasm", get(serve_wasm))
        .route(
            "/api/status",
            get(|| async { Json(Status { status: "ok" }) }),
        )
        .route(
            "/api/project",
            get(|| async {
                Json(serde_json::json!({"project": crate::default_project().display().to_string()}))
            }),
        )
        .route("/api/projects", get(api_projects))
        .route("/api/prompts", get(api_prompts))
        .route("/api/intent", post(api_intent))
        .route("/ws/stream", get(ws_stream))
        .route("/context/recent", get(recent_changes))
        .route("/build/presets", get(build_presets))
        .route("/build/command", get(build_command))
        .route("/api/file", get(api_file))
        .route("/api/diff", post(api_diff))
        .route("/api/apply", post(api_apply))
        .route("/api/backlog", get(api_backlog_get).post(api_backlog_post))
        .route("/api/backlog/run", post(api_backlog_run))
        .route("/api/demo/record", post(api_demo_record))
        .route("/api/test/run", get(api_test_run))
        .route("/api/webhook/github", post(api_webhook_github))
        // OpenAI-compat inference — kova as inference server
        .route("/v1/chat/completions", post(v1_chat_completions))
        .route("/v1/models", get(v1_models));
    #[cfg(feature = "inference")]
    let r = r
        .route("/api/route", post(api_route))
        .route("/api/explain", get(api_explain))
        .route("/api/explain/run", post(api_explain_run))
        .route("/api/moe/run", post(api_moe_run));
    r
}

async fn build_command(Query(q): Query<BuildCommandQuery>) -> Json<BuildCommandResponse> {
    let preset = crate::load_build_preset(&q.project);
    let project_path = crate::default_project();
    let root = crate::workspace_root(&project_path);
    let command = if let Some(p) = preset {
        let mut args = vec!["cargo", "build", "-p", &p.package];
        if q.release {
            args.push("--release");
        }
        if let Some(t) = &p.target {
            args.push("--target");
            args.push(t);
        }
        for f in &p.features {
            args.push("--features");
            args.push(f);
        }
        format!("cd {} && {}", root.display(), args.join(" "))
    } else {
        let mut args = vec!["cargo", "build"];
        if q.release {
            args.push("--release");
        }
        format!("cd {} && {}", project_path.display(), args.join(" "))
    };
    Json(BuildCommandResponse { command })
}

pub async fn run(addr: SocketAddr) -> anyhow::Result<()> {
    let state = T92::default();
    let app = app_router()
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Kova API at http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Run server and open browser after bind.
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_app() -> Router {
        crate::bootstrap().unwrap();
        app_router()
            .layer(CorsLayer::permissive())
            .with_state(T92::default())
    }

    #[tokio::test]
    async fn api_status_returns_ok() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn api_project_returns_path() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/project")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["project"].is_string());
    }

    #[tokio::test]
    async fn api_projects_returns_array() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/projects")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.is_array());
    }

    #[tokio::test]
    async fn api_prompts_returns_system_and_persona() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/prompts")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("system").is_some());
        assert!(json.get("persona").is_some());
    }

    #[tokio::test]
    async fn api_build_presets_returns_map() {
        let app = test_app();
        let req = Request::builder()
            .uri("/build/presets")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.is_object());
    }

    #[tokio::test]
    async fn api_file_404_for_missing() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/file?hint=nonexistent_xyz.rs")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn root_returns_html() {
        let app = test_app();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(
            html.contains("<html") || html.contains("<!DOCTYPE"),
            "got: {}...",
            &html[..100.min(html.len())]
        );
    }

    #[test]
    fn safe_hint_sanitizes() {
        assert_eq!(safe_hint(Some("lib.rs")), "lib.rs");
        assert_eq!(safe_hint(Some("my_module.rs")), "my_module.rs");
        assert_eq!(safe_hint(Some("../../etc/passwd")), "lib.rs");
        assert_eq!(safe_hint(Some("/root/.ssh/id_rsa")), "lib.rs");
        assert_eq!(safe_hint(Some("")), "lib.rs");
        assert_eq!(safe_hint(None), "lib.rs");
    }

    #[tokio::test]
    async fn context_recent_returns_string() {
        let app = test_app();
        let req = Request::builder()
            .uri("/context/recent?minutes=1")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

pub async fn run_with_open(addr: SocketAddr, url: &str) -> anyhow::Result<()> {
    let state = T92::default();
    let app = app_router()
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Kova API at {}", url);

    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", url])
    } else {
        ("xdg-open", vec![url])
    };
    if let Err(e) = std::process::Command::new(cmd).args(&args).spawn() {
        tracing::warn!("Could not open browser: {}", e);
    }

    axum::serve(listener, app).await?;
    Ok(())
}

