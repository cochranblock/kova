//! egui GUI. kova gui. Professional theme per THEME.md.
//! f113=gui_run

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use eframe::egui;
#[cfg(feature = "inference")]
use std::sync::Arc;
use std::sync::mpsc;
#[cfg(feature = "inference")]
use tokio::sync::broadcast;

use crate::theme::{self, colors, layout};

// ── Tab Navigation ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Chat,
    Artifacts,
    Moe,
    Forge,
    Deploy,
}

impl Tab {
    fn label(&self) -> &'static str {
        match self {
            Tab::Chat => "Chat",
            Tab::Artifacts => "Artifacts",
            Tab::Moe => "MoE",
            Tab::Forge => "Forge",
            Tab::Deploy => "Deploy",
        }
    }
}

// ── Remote MoE types ──────────────────────────────────────────

/// MoE variant from remote /api/moe/run response.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MoeVariant {
    pub node_id: String,
    pub code: String,
    pub gen_ms: u64,
    pub compile_ok: bool,
    pub clippy_ok: bool,
    pub tests_ok: bool,
    pub total_score: u32,
}

/// MoE result from remote /api/moe/run response.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MoeResult {
    pub variants: Vec<MoeVariant>,
    pub winner: Option<MoeVariant>,
    pub prompt: String,
}

/// Load cluster URL from config or env.
fn load_cluster_url() -> String {
    std::env::var("KOVA_CLUSTER_URL").unwrap_or_else(|_| {
        let bind = crate::bind_addr();
        format!("http://{}", bind)
    })
}

/// Check if cluster is reachable (non-blocking, quick timeout).
fn check_cluster(url: &str) -> bool {
    let endpoint = format!("{}/api/status", url);
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok()
        .and_then(|c| c.get(&endpoint).send().ok())
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Send MoE request to remote cluster. Blocks.
fn remote_moe(url: &str, prompt: &str) -> Result<MoeResult, String> {
    let endpoint = format!("{}/api/moe/run", url);
    let body = serde_json::json!({
        "prompt": prompt,
        "num_experts": 3,
        "run_review": true,
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.post(&endpoint).json(&body).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("MoE failed: {}", resp.status()));
    }
    resp.json::<MoeResult>().map_err(|e| e.to_string())
}

/// Send chat to remote cluster via OpenAI-compat endpoint.
#[allow(dead_code)]
fn remote_chat(url: &str, system: &str, prompt: &str) -> Result<String, String> {
    let endpoint = format!("{}/v1/chat/completions", url);
    let body = serde_json::json!({
        "model": "default",
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.post(&endpoint).json(&body).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("chat failed: {}", resp.status()));
    }
    #[derive(serde::Deserialize)]
    struct OaiResp { choices: Vec<OaiChoice> }
    #[derive(serde::Deserialize)]
    struct OaiChoice { message: OaiMsg }
    #[derive(serde::Deserialize)]
    struct OaiMsg { content: String }
    let oai: OaiResp = resp.json().map_err(|e| e.to_string())?;
    Ok(oai.choices.into_iter().next().map(|c| c.message.content).unwrap_or_default())
}

/// f113=gui_run. Run native egui GUI. Demo mode records to ~/.kova/demos/.
pub fn run(demo: bool) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 560.0]),
        run_and_return: false, // Use blocking event loop; may help macOS window visibility
        ..Default::default()
    };
    eframe::run_native(
        "Kova",
        options,
        Box::new(move |cc| {
            theme::f320(&cc.egui_ctx);
            Ok(Box::new(KovaApp::new(cc, demo)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {}", e))
}

pub struct KovaApp {
    input: String,
    messages: Vec<crate::Message>,
    store: Option<crate::storage::t12>,
    system_prompt: String,
    persona: String,
    show_prompts: bool,
    show_backlog: bool,
    pending_intent: Option<crate::t0>,
    /// Current project for code gen. Discovered from projects_root; user selects from dropdown.
    current_project: std::path::PathBuf,
    #[cfg(feature = "inference")]
    llm_receiver: Option<mpsc::Receiver<std::sync::Arc<str>>>,
    #[cfg(feature = "inference")]
    router_receiver: Option<mpsc::Receiver<crate::T94>>,
    #[cfg(feature = "inference")]
    router_pending_user_msg: Option<String>,
    #[cfg(feature = "inference")]
    pipeline_receiver: Option<broadcast::Receiver<Arc<str>>>,
    #[cfg(feature = "inference")]
    last_applied: Option<String>,
    #[cfg(feature = "inference")]
    clarification_pending: bool,
    #[cfg(feature = "inference")]
    f364: Option<Vec<String>>,
    #[cfg(feature = "inference")]
    restatement_pending: bool,
    #[cfg(feature = "inference")]
    restatement_pending_msg: Option<String>,
    /// Demo mode: record actions to ~/.kova/demos/
    demo_recording: Option<DemoRecording>,
    /// Sprite QC panel state.
    sprite_qc: Option<crate::sprite_qc::T213>,
    /// Path input for sprite QC directory.
    sprite_qc_path: String,
    // ── Navigation ──
    active_tab: Tab,
    // ── Proof of Artifacts ──
    proof_git_line: String,
    proof_expanded: bool,
    /// Track which project the proof card was last fetched for.
    proof_project: std::path::PathBuf,
    // ── Remote MoE (works without inference feature) ──
    /// Cluster URL for remote inference (e.g. "http://192.168.1.44:3002").
    cluster_url: String,
    /// Whether remote cluster is reachable.
    cluster_online: bool,
    /// Show cluster config panel.
    show_cluster_config: bool,
    /// MoE result receiver (from background thread).
    moe_receiver: Option<std::sync::mpsc::Receiver<MoeResult>>,
    /// Last MoE result for display.
    moe_result: Option<MoeResult>,
    /// Remote chat receiver (from background thread).
    remote_chat_receiver: Option<std::sync::mpsc::Receiver<String>>,
    /// Startup cluster check receiver (non-blocking).
    cluster_check_rx: Option<std::sync::mpsc::Receiver<bool>>,
}

impl DemoRecording {
    fn push(&mut self, kind: &str, data: serde_json::Value) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let mut m = serde_json::Map::new();
        m.insert("kind".into(), serde_json::Value::String(kind.to_string()));
        m.insert("ts_ms".into(), serde_json::json!(ts));
        if let Some(obj) = data.as_object() {
            for (k, v) in obj {
                m.insert(k.clone(), v.clone());
            }
        }
        self.actions.push(serde_json::Value::Object(m));
    }
    fn save(&self) {
        let dir = std::env::var("KOVA_DEMO_DIR")
            .ok()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".kova")
                    .join("demos")
            });
        let _ = std::fs::create_dir_all(&dir);
        let safe = self.name.replace(['/', '\\', ':'], "_");
        let path = dir.join(format!("{}.json", safe));
        let payload = serde_json::json!({
            "name": self.name,
            "source": "egui",
            "actions": self.actions,
            "started_at": self.started_at
        });
        if let Ok(json) = serde_json::to_string_pretty(&payload) {
            let _ = std::fs::write(&path, json);
            eprintln!("Demo saved: {}", path.display());
        }
    }
}

struct DemoRecording {
    name: String,
    actions: Vec<serde_json::Value>,
    started_at: String,
}

#[cfg(feature = "inference")]
fn response_for_input(input: &str) -> (String, Option<crate::t0>) {
    match crate::f62(input) {
        Some(intent) => {
            let name = crate::f325(&intent.s0);
            (format!("Run {}? (y/n)", name), Some(intent))
        }
        None => ("".into(), None),
    }
}

fn is_confirm(s: &str) -> bool {
    let t = s.trim().to_lowercase();
    t == "y" || t == "yes"
}

/// Shared logic for building system prompt with Cursor rules. Testable.
#[cfg(feature = "inference")]
fn build_system_prompt_impl(system: &str, persona: &str, project: &std::path::Path) -> String {
    let cursor = crate::cursor_prompts::f111(project);
    if cursor.is_empty() {
        format!("{}\n\n{}", system, persona)
    } else {
        format!(
            "{}\n\n{}\n\n--- Cursor rules ---\n{}",
            system, persona, cursor
        )
    }
}

impl KovaApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, demo: bool) -> Self {
        let system_prompt = crate::load_prompt("system");
        let persona = crate::load_prompt("persona");
        let store = crate::storage::t12::f39(crate::sled_path()).ok();
        let messages = store
            .as_ref()
            .and_then(|s| crate::f74(s).ok())
            .unwrap_or_default();
        let demo_recording = if demo {
            Some(DemoRecording {
                name: format!(
                    "egui-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                ),
                actions: Vec::new(),
                started_at: format!(
                    "{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                ),
            })
        } else {
            None
        };
        Self {
            input: String::new(),
            messages,
            store,
            system_prompt,
            persona,
            show_prompts: false,
            show_backlog: false,
            pending_intent: None,
            current_project: crate::default_project(),
            #[cfg(feature = "inference")]
            llm_receiver: None,
            #[cfg(feature = "inference")]
            router_receiver: None,
            #[cfg(feature = "inference")]
            router_pending_user_msg: None,
            #[cfg(feature = "inference")]
            pipeline_receiver: None,
            #[cfg(feature = "inference")]
            last_applied: None,
            #[cfg(feature = "inference")]
            clarification_pending: false,
            #[cfg(feature = "inference")]
            f364: None,
            #[cfg(feature = "inference")]
            restatement_pending: false,
            #[cfg(feature = "inference")]
            restatement_pending_msg: None,
            demo_recording,
            sprite_qc: None,
            sprite_qc_path: String::new(),
            active_tab: Tab::Chat,
            proof_git_line: String::new(),
            proof_expanded: true,
            proof_project: std::path::PathBuf::new(),
            cluster_url: load_cluster_url(),
            cluster_online: false,
            show_cluster_config: false,
            moe_receiver: None,
            moe_result: None,
            remote_chat_receiver: None,
            cluster_check_rx: {
                let url = load_cluster_url();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(check_cluster(&url));
                });
                Some(rx)
            },
        }
    }

    #[cfg(feature = "inference")]
    fn f311(&self) -> String {
        build_system_prompt_impl(&self.system_prompt, &self.persona, &self.current_project)
    }

    fn run_intent(
        &mut self,
        intent: crate::t0,
        project: std::path::PathBuf,
        approuter_dir: Option<std::path::PathBuf>,
    ) {
        if let Some(ref mut rec) = self.demo_recording {
            rec.push(
                "api_call",
                serde_json::json!({
                    "method": "run_intent",
                    "path": crate::f325(&intent.s0),
                    "project": project.display().to_string()
                }),
            );
        }
        let plan = crate::t3::f14(&intent, project, approuter_dir);
        let exec = crate::t6;
        match exec.f15(&plan) {
            Ok(results) => {
                let all_ok = results.iter().all(|r| r.s11);
                let summary: Vec<String> = results
                    .iter()
                    .map(|r| {
                        let mark = if r.s11 { "✓" } else { "✗" };
                        format!("{} {} {}", mark, r.s10, r.s13)
                    })
                    .collect();
                let response = if results.is_empty() {
                    "No actions for this intent.".into()
                } else if all_ok {
                    format!("Done.\n{}", summary.join("\n"))
                } else {
                    format!("Failed.\n{}", summary.join("\n"))
                };
                self.messages.push(crate::Message {
                    role: "assistant".into(),
                    content: response.clone(),
                });
                self.persist("assistant", &response);
            }
            Err(e) => {
                let response = format!("Error: {}", e);
                self.messages.push(crate::Message {
                    role: "assistant".into(),
                    content: response.clone(),
                });
                self.persist("assistant", &response);
            }
        }
    }

    fn persist(&self, role: &str, content: &str) {
        if let Some(ref s) = self.store {
            let _ = crate::f73(s, role, content);
        }
    }
}

impl eframe::App for KovaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(feature = "inference")]
        {
            if let Some(rx) = &mut self.router_receiver {
                match rx.try_recv() {
                    Ok(result) => {
                        self.router_receiver = None;
                        let to_persist = if let Some(m) = self.messages.last_mut() {
                            match &result {
                                crate::T94::NeedsClarification { .. } => {
                                    let orig =
                                        self.router_pending_user_msg.as_deref().unwrap_or("");
                                    let q = result.f363(orig);
                                    m.content = q.clone();
                                    self.clarification_pending = true;
                                    self.f364 =
                                        result.f364().map(|s| s.to_vec());
                                    Some(m.content.clone())
                                }
                                crate::T94::Error(e) => {
                                    m.content = format!("Router error: {}", e);
                                    Some(m.content.clone())
                                }
                                crate::T94::CodeGen => {
                                    let user_msg =
                                        self.router_pending_user_msg.take().unwrap_or_default();
                                    let (action, target) =
                                        if let Some(in_pos) = user_msg.find(" in ") {
                                            let (a, t) = user_msg.split_at(in_pos);
                                            (a.trim(), t[" in ".len()..].trim())
                                        } else if let Some(to_pos) = user_msg.find(" to ") {
                                            let (a, t) = user_msg.split_at(to_pos);
                                            (a.trim(), t[" to ".len()..].trim())
                                        } else {
                                            (user_msg.as_str(), "")
                                        };
                                    let restatement =
                                        crate::elicitor::f304(action, target);
                                    m.content = restatement.clone();
                                    self.restatement_pending = true;
                                    self.restatement_pending_msg = Some(user_msg);
                                    Some(restatement)
                                }
                                _ if result.f365()
                                    || matches!(result, crate::T94::Run) =>
                                {
                                    m.content.clear();
                                    let user_msg =
                                        self.router_pending_user_msg.take().unwrap_or_default();
                                    if let Some(path) = crate::f78(crate::ModelRole::Coder) {
                                        let system = self.f311();
                                        let hist: Vec<(String, String)> = Vec::new();
                                        let rx =
                                            crate::inference::f76(&path, &system, &hist, &user_msg);
                                        self.llm_receiver = Some(rx);
                                        None
                                    } else {
                                        m.content =
                                            "No coder model. Run: kova model install".into();
                                        Some(m.content.clone())
                                    }
                                }
                                _ => {
                                    m.content = "Unexpected classification.".into();
                                    Some(m.content.clone())
                                }
                            }
                        } else {
                            None
                        };
                        if let Some(c) = to_persist {
                            self.persist("assistant", &c);
                        }
                        ctx.request_repaint();
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.router_receiver = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if self.router_receiver.is_some() {
                ctx.request_repaint();
            }

            if let Some(rx) = &mut self.pipeline_receiver {
                loop {
                    match rx.try_recv() {
                        Ok(content) => {
                            if let Some(m) = self.messages.last_mut() {
                                m.content = (*content).to_string();
                            }
                            ctx.request_repaint();
                        }
                        Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                        Err(broadcast::error::TryRecvError::Closed) => {
                            if let Some(m) = self.messages.last() {
                                self.persist("assistant", &m.content);
                            }
                            self.pipeline_receiver = None;
                            break;
                        }
                        Err(broadcast::error::TryRecvError::Empty) => break,
                    }
                }
                ctx.request_repaint();
            }
            if self.pipeline_receiver.is_some() {
                ctx.request_repaint();
            }

            while let Some(rx) = &mut self.llm_receiver {
                match rx.try_recv() {
                    Ok(token) => {
                        if let Some(m) = self.messages.last_mut() {
                            m.content.push_str(&token);
                        }
                        ctx.request_repaint();
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        let content = self
                            .messages
                            .last()
                            .map(|m| m.content.clone())
                            .unwrap_or_default();
                        if !content.is_empty() {
                            self.persist("assistant", &content);
                        }
                        self.llm_receiver = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
            if self.llm_receiver.is_some() {
                ctx.request_repaint();
            }
        }

        // ── Poll MoE receiver ──
        if let Some(rx) = &self.moe_receiver {
            if let Ok(result) = rx.try_recv() {
                let summary = if let Some(ref w) = result.winner {
                    format!("[MoE] Winner: {} (score {})", w.node_id, w.total_score)
                } else {
                    "[MoE] No winner".into()
                };
                self.messages.push(crate::Message {
                    role: "assistant".into(),
                    content: summary.clone(),
                });
                self.persist("assistant", &summary);
                self.moe_result = Some(result);
                self.moe_receiver = None;
                ctx.request_repaint();
            }
        }
        if self.moe_receiver.is_some() {
            ctx.request_repaint();
        }

        // ── Poll remote chat receiver ──
        if let Some(rx) = &self.remote_chat_receiver {
            if let Ok(response) = rx.try_recv() {
                self.messages.push(crate::Message {
                    role: "assistant".into(),
                    content: response.clone(),
                });
                self.persist("assistant", &response);
                self.remote_chat_receiver = None;
                ctx.request_repaint();
            }
        }
        if self.remote_chat_receiver.is_some() {
            ctx.request_repaint();
        }

        // ── Poll startup cluster check ──
        if let Some(rx) = &self.cluster_check_rx {
            if let Ok(online) = rx.try_recv() {
                self.cluster_online = online;
                self.cluster_check_rx = None;
            }
        }

        // ── Refresh proof card when project changes ──
        if self.proof_project != self.current_project {
            self.proof_project = self.current_project.clone();
            self.proof_git_line = match std::process::Command::new("git")
                .args(["log", "-1", "--oneline"])
                .current_dir(&self.current_project)
                .output()
            {
                Ok(o) if o.status.success() => {
                    String::from_utf8_lossy(&o.stdout).trim().to_string()
                }
                _ => "(no git info)".into(),
            };
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(colors::BG).inner_margin(egui::Margin::same(layout::MARGIN_I8)))
            .show(ctx, |ui| {
            // ════════════════════════════════════════════════════════════
            // ZONE 1: Header + Proof Card
            // ════════════════════════════════════════════════════════════
            ui.add_space(layout::GAP);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Kova").color(colors::PRIMARY).size(22.0).strong());
                ui.add_space(layout::MARGIN);
                let projects = crate::discover_projects();
                let current_name = self.current_project.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?").to_string();
                egui::ComboBox::from_id_salt("project_selector")
                    .selected_text(&current_name)
                    .show_ui(ui, |ui| {
                        for p in &projects {
                            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                            if ui.selectable_label(name == current_name, name).clicked() {
                                self.current_project = p.clone();
                                ui.close();
                            }
                        }
                        if projects.is_empty() {
                            ui.label(egui::RichText::new("(none)").color(colors::MUTED));
                        }
                    });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (dot, tip) = if self.cluster_online {
                        ("\u{25CF}", "Cluster online")
                    } else {
                        ("\u{25CB}", "Cluster offline")
                    };
                    let color = if self.cluster_online { colors::TERTIARY } else { colors::MUTED };
                    ui.label(egui::RichText::new(dot).color(color).size(14.0))
                        .on_hover_text(tip);
                });
            });

            // ── Proof of Artifacts card ──
            ui.add_space(layout::PADDING_SM);
            let proof_header = if self.proof_expanded { "Proof of Artifacts  \u{25B4}" } else { "Proof of Artifacts  \u{25BE}" };
            if ui.add(egui::Label::new(
                egui::RichText::new(proof_header).color(colors::PRIMARY).strong().size(13.0),
            ).sense(egui::Sense::click())).clicked() {
                self.proof_expanded = !self.proof_expanded;
            }
            if self.proof_expanded {
                let project_name = self.current_project.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                let is_remote = crate::config::is_remote_only(project_name);
                let build_loc = if is_remote {
                    format!("builds on {}", crate::config::remote_build_node())
                } else {
                    "builds locally".into()
                };
                let deploy_url = match project_name {
                    "cochranblock" => "cochranblock.org",
                    "oakilydokily" => "oakilydokily.com",
                    "rogue-repo" => "rogue-repo (localhost:3001)",
                    "ronin-sites" => "ronin-sites (localhost:8000)",
                    "approuter" => "approuter (localhost:8080)",
                    _ => "",
                };
                theme::f321().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Build:").color(colors::MUTED));
                        ui.label(egui::RichText::new(&build_loc).color(colors::TEXT));
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Last commit:").color(colors::MUTED));
                        ui.label(egui::RichText::new(&self.proof_git_line).color(colors::TEXT).monospace());
                    });
                    if !deploy_url.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Deploy:").color(colors::MUTED));
                            ui.label(egui::RichText::new(deploy_url).color(colors::TERTIARY));
                        });
                    }
                });
            }
            ui.add_space(layout::GAP);
            ui.separator();

            // ════════════════════════════════════════════════════════════
            // ZONE 2: Chat (scrollable, takes remaining space minus input)
            // ════════════════════════════════════════════════════════════
            let input_height = 40.0;
            let available = ui.available_height() - input_height - layout::GAP * 2.0;
            egui::ScrollArea::vertical()
                .max_height(available.max(80.0))
                .stick_to_bottom(true)
                .show(ui, |ui| {
                for (i, m) in self.messages.iter().enumerate() {
                    let (prefix, color) = if m.role == "user" {
                        ("You", colors::PRIMARY)
                    } else {
                        ("Kova", colors::SECONDARY)
                    };
                    theme::f321().show(ui, |ui| {
                    ui.label(egui::RichText::new(prefix).color(color).strong());
                    let content = {
                        #[cfg(feature = "inference")]
                        let (llm_waiting, router_waiting) =
                            (self.llm_receiver.is_some(), self.router_receiver.is_some());
                        #[cfg(not(feature = "inference"))]
                        let (llm_waiting, router_waiting) = (false, false);
                        if m.role == "assistant" && i == self.messages.len() - 1 {
                            #[cfg(feature = "inference")]
                            let pipeline_waiting = self.pipeline_receiver.is_some();
                            #[cfg(not(feature = "inference"))]
                            let pipeline_waiting = false;
                            if router_waiting {
                                "Classifying…"
                            } else if pipeline_waiting {
                                "Generating…"
                            } else if m.content.is_empty() && llm_waiting {
                                "Thinking…"
                            } else {
                                &m.content
                            }
                        } else {
                            &m.content
                        }
                    };
                    ui.add_space(layout::PADDING_SM);
                    ui.label(egui::RichText::new(content).color(colors::TEXT).monospace());

                    #[cfg(feature = "inference")]
                    if m.role == "assistant"
                        && let Some(code) = crate::pipeline::extract_rust_block(&m.content)
                    {
                            ui.add_space(layout::PADDING_SM);
                            let user_msg = self
                                .messages
                                .iter()
                                .enumerate()
                                .rev()
                                .find(|(j, x)| *j < i && x.role == "user")
                                .map(|(_, x)| x.content.as_str())
                                .unwrap_or("");
                            let hint = crate::context_loader::f83(user_msg);
                            let project = &self.current_project;
                            let target = crate::output::f85(project, hint.as_deref());
                            let current = std::fs::read_to_string(&target).unwrap_or_default();

                            ui.horizontal(|ui| {
                                if ui.button("Copy").clicked() {
                                    ui.ctx().output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(code.clone())));
                                }
                                if ui.button("Apply").clicked() {
                                    let parent = target.parent().unwrap_or(project);
                                    if let Err(e) = std::fs::create_dir_all(parent) {
                                        self.last_applied =
                                            Some(format!("Apply failed: {}", e));
                                    } else if let Err(e) = std::fs::write(&target, &code) {
                                        self.last_applied =
                                            Some(format!("Apply failed: {}", e));
                                    } else {
                                        self.last_applied =
                                            Some(format!("Applied to {}", target.display()));
                                    }
                                }
                                if let Some(ref msg) = self.last_applied {
                                    let c = if msg.starts_with("Applied") {
                                        colors::TERTIARY
                                    } else {
                                        colors::SECONDARY
                                    };
                                    ui.label(egui::RichText::new(msg).color(c));
                                }
                            });

                            egui::CollapsingHeader::new(format!("Show diff ({})", i))
                                .default_open(false)
                                .show(ui, |ui| {
                                    let diff = crate::output::f84(&current, &code);
                                    egui::ScrollArea::vertical()
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new(diff).color(colors::TEXT).monospace());
                                        });
                                });
                    }
                    });
                    ui.add_space(layout::GAP);
                }

                // ── MoE results inline in chat ──
                if let Some(result) = self.moe_result.clone() {
                    theme::f321().show(ui, |ui| {
                        ui.label(egui::RichText::new("MoE Results").color(colors::SECONDARY).strong());
                        ui.add_space(layout::PADDING_SM);
                        egui::Grid::new("moe_grid").striped(true).show(ui, |ui| {
                            ui.label(egui::RichText::new("Node").color(colors::MUTED).strong());
                            ui.label(egui::RichText::new("Compile").color(colors::MUTED).strong());
                            ui.label(egui::RichText::new("Clippy").color(colors::MUTED).strong());
                            ui.label(egui::RichText::new("Tests").color(colors::MUTED).strong());
                            ui.label(egui::RichText::new("Score").color(colors::MUTED).strong());
                            ui.end_row();
                            for v in &result.variants {
                                let is_winner = result.winner.as_ref().map(|w| w.node_id == v.node_id).unwrap_or(false);
                                let nc = if is_winner { colors::TERTIARY } else { colors::TEXT };
                                ui.label(egui::RichText::new(&v.node_id).color(nc));
                                ui.label(if v.compile_ok { "ok" } else { "FAIL" });
                                ui.label(if v.clippy_ok { "ok" } else { "FAIL" });
                                ui.label(if v.tests_ok { "ok" } else { "FAIL" });
                                ui.label(egui::RichText::new(format!("{}", v.total_score)).color(nc));
                                ui.end_row();
                            }
                        });
                        if let Some(ref w) = result.winner {
                            ui.add_space(layout::PADDING_SM);
                            ui.label(egui::RichText::new(format!("Winner: {} (score {})", w.node_id, w.total_score)).color(colors::TERTIARY));
                            ui.add_space(layout::PADDING_SM);
                            egui::ScrollArea::vertical().max_height(200.0).id_salt("moe_code").show(ui, |ui| {
                                ui.label(egui::RichText::new(&w.code).color(colors::TEXT).monospace());
                            });
                            ui.horizontal(|ui| {
                                if ui.button("Copy").clicked() {
                                    ui.ctx().output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(w.code.clone())));
                                }
                            });
                        } else if !result.variants.is_empty() {
                            ui.label(egui::RichText::new("No winner — all variants failed").color(colors::SECONDARY));
                        }
                    });
                    ui.add_space(layout::GAP);
                }
            });

            // ════════════════════════════════════════════════════════════
            // ZONE 3: Prompt (bottom, always visible)
            // ════════════════════════════════════════════════════════════
            ui.add_space(layout::GAP);
            ui.horizontal(|ui| {
                let mut send = false;
                let input_resp = ui.add_sized(
                    [ui.available_width() - 60.0, input_height],
                    egui::TextEdit::singleline(&mut self.input).hint_text("Ask Kova..."),
                );
                if input_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.input.is_empty() {
                    send = true;
                    input_resp.request_focus();
                }
                if ui.add_sized([55.0, input_height], egui::Button::new("Send")).clicked() && !self.input.is_empty() {
                    send = true;
                }
                if send {
                    let msg = std::mem::take(&mut self.input);
                    if !msg.is_empty() {
                        if let Some(ref mut rec) = self.demo_recording {
                            if is_confirm(&msg) {
                                let intent = self.pending_intent.as_ref().map(|i| crate::f325(&i.s0).to_string()).unwrap_or_default();
                                rec.push("egui_confirm", serde_json::json!({ "intent": intent }));
                            } else {
                                rec.push("egui_send", serde_json::json!({ "text": msg }));
                            }
                        }
                        self.messages.push(crate::Message {
                            role: "user".into(),
                            content: msg.clone(),
                        });
                        self.persist("user", &msg);

                        if is_confirm(&msg) {
                            if let Some(intent) = self.pending_intent.take() {
                                let project = self.current_project.clone();
                                let approuter_dir = std::env::var("HOME")
                                    .ok()
                                    .map(|h| std::path::PathBuf::from(h).join("approuter"));
                                self.run_intent(intent, project, approuter_dir);
                            } else {
                                let response: String = "No pending intent to run.".into();
                                self.messages.push(crate::Message {
                                    role: "assistant".into(),
                                    content: response.clone(),
                                });
                                self.persist("assistant", &response);
                            }
                        } else {
                            #[cfg(feature = "inference")]
                            let is_restatement = self.restatement_pending;

                            #[cfg(feature = "inference")]
                            if is_restatement {
                                self.restatement_pending = false;
                                let pending = self.restatement_pending_msg.take().unwrap_or_default();
                                let reply = crate::elicitor::f303(&msg, None);
                                match reply {
                                    crate::elicitor::T177::Confirm(true) => {
                                        let coder = crate::f78(crate::ModelRole::Coder);
                                        let fix = crate::f78(crate::ModelRole::Fix)
                                            .or_else(|| crate::f78(crate::ModelRole::Coder));
                                        if let (Some(cp), Some(fp)) = (coder, fix) {
                                            let system = self.f311();
                                            let project = self.current_project.clone();
                                            let max_retries = crate::orchestration_max_fix_retries();
                                            let run_clippy = crate::orchestration_run_clippy();
                                            let rx = crate::pipeline::f81(
                                                &cp, &fp, &system, &pending,
                                                &project, max_retries, run_clippy, None,
                                            );
                                            self.pipeline_receiver = Some(rx);
                                            if let Some(m) = self.messages.last_mut() {
                                                m.content = "Generating…".into();
                                            }
                                        } else {
                                            self.messages.push(crate::Message {
                                                role: "assistant".into(),
                                                content: "No coder model. Run: kova model install".into(),
                                            });
                                        }
                                    }
                                    _ => {
                                        self.messages.push(crate::Message {
                                            role: "assistant".into(),
                                            content: "Cancelled.".into(),
                                        });
                                        self.persist("assistant", "Cancelled.");
                                    }
                                }
                            } else {
                            #[cfg(feature = "inference")]
                            let is_clarification = self.clarification_pending;

                            #[cfg(feature = "inference")]
                            let to_route_opt = if is_clarification {
                                let orig = self.router_pending_user_msg.take().unwrap_or_default();
                                self.clarification_pending = false;
                                let choices = self.f364.take();
                                let reply = crate::elicitor::f303(
                                    &msg,
                                    choices.as_ref().map(|c| c.len()),
                                );
                                match reply {
                                    crate::elicitor::T177::Cancel => {
                                        self.messages.push(crate::Message {
                                            role: "assistant".into(),
                                            content: "Cancelled.".into(),
                                        });
                                        self.persist("assistant", "Cancelled.");
                                        None
                                    }
                                    crate::elicitor::T177::Choice(idx) => {
                                        let s = choices
                                            .as_ref()
                                            .and_then(|ch| ch.get(idx))
                                            .map(|s| s.as_str())
                                            .unwrap_or(&msg);
                                        Some(format!("{} in {}", orig, s))
                                    }
                                    _ => {
                                        let s = if msg.contains(".rs") {
                                            format!("{} in {}", orig, msg)
                                        } else {
                                            format!("{} {}", orig, msg)
                                        };
                                        Some(s)
                                    }
                                }
                            } else {
                                Some(msg.clone())
                            };
                            #[cfg(not(feature = "inference"))]
                            let to_route_opt = Some(msg.clone());

                            #[cfg(feature = "inference")]
                            let skip_keyword = is_clarification;
                            #[cfg(not(feature = "inference"))]
                            let skip_keyword = false;

                            if let Some(to_route) = to_route_opt {
                            let (response, intent) = if skip_keyword {
                                (String::new(), None)
                            } else {
                                response_for_input(&msg)
                            };
                            self.pending_intent = intent;
                            if !response.is_empty() {
                                self.messages.push(crate::Message {
                                    role: "assistant".into(),
                                    content: response.clone(),
                                });
                                self.persist("assistant", &response);
                            } else if let Some(router_path) = crate::f78(crate::ModelRole::Router) {
                                #[cfg(feature = "inference")]
                                {
                                    let rx = crate::f79(&router_path, &to_route);
                                    self.messages.push(crate::Message {
                                        role: "assistant".into(),
                                        content: "Classifying…".into(),
                                    });
                                    self.router_receiver = Some(rx);
                                    self.router_pending_user_msg = Some(to_route);
                                }
                            } else if let Some(model_path) = crate::inference_model_path() {
                                #[cfg(feature = "inference")]
                                {
                                    let system = self.f311();
                                    let hist: Vec<(String, String)> = Vec::new();
                                    let rx = crate::inference::f76(
                                        &model_path,
                                        &system,
                                        &hist,
                                        &msg,
                                    );
                                    self.messages.push(crate::Message {
                                        role: "assistant".into(),
                                        content: String::new(),
                                    });
                                    self.llm_receiver = Some(rx);
                                }
                            } else {
                                // Try remote chat if cluster is online
                                if self.cluster_online && self.remote_chat_receiver.is_none() {
                                    let url = self.cluster_url.clone();
                                    let system = self.system_prompt.clone();
                                    let user_msg = msg.clone();
                                    let (tx, rx) = mpsc::channel();
                                    self.remote_chat_receiver = Some(rx);
                                    std::thread::spawn(move || {
                                        match remote_chat(&url, &system, &user_msg) {
                                            Ok(resp) => { let _ = tx.send(resp); }
                                            Err(e) => { let _ = tx.send(format!("Error: {}", e)); }
                                        }
                                    });
                                    self.messages.push(crate::Message {
                                        role: "assistant".into(),
                                        content: "Thinking...".into(),
                                    });
                                } else if !self.cluster_online {
                                    // Try mobile-llm (bundled GGUF) if available
                                    #[cfg(feature = "mobile-llm")]
                                    {
                                        if let Some(model_path) = crate::mobile_llm::find_model() {
                                            let system = self.system_prompt.clone();
                                            let user_msg = msg.clone();
                                            let (tx, rx) = mpsc::channel();
                                            self.remote_chat_receiver = Some(rx);
                                            std::thread::spawn(move || {
                                                match crate::mobile_llm::generate(&model_path, &system, &user_msg) {
                                                    Ok(resp) => { let _ = tx.send(resp); }
                                                    Err(e) => { let _ = tx.send(format!("Error: {}", e)); }
                                                }
                                            });
                                            self.messages.push(crate::Message {
                                                role: "assistant".into(),
                                                content: "Thinking (local)...".into(),
                                            });
                                        } else {
                                            self.messages.push(crate::Message {
                                                role: "assistant".into(),
                                                content: "Offline. No bundled model found in ~/.kova/models/".into(),
                                            });
                                        }
                                    }
                                    #[cfg(not(feature = "mobile-llm"))]
                                    {
                                        #[cfg(feature = "inference")]
                                        let fallback = "No local model. Run: kova model install";
                                        #[cfg(not(feature = "inference"))]
                                        let fallback = "Cluster offline. No inference available.";
                                        self.messages.push(crate::Message {
                                            role: "assistant".into(),
                                            content: fallback.into(),
                                        });
                                        self.persist("assistant", fallback);
                                    }
                                }
                            }
                            }
                            // else: cancelled - already pushed message above
                        }
                        }
                    }
                }
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(rec) = self.demo_recording.take() {
            rec.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prove GUI f311 includes baked Cursor rules when enabled.
    /// Uses temp dir for KOVA_PROJECTS_ROOT to avoid slow discover_projects on home.
    #[test]
    fn gui_build_system_prompt_includes_baked() {
        let tmp = tempfile::TempDir::new().unwrap();
        unsafe { std::env::set_var("KOVA_PROJECTS_ROOT", tmp.path()) };
        let out = build_system_prompt_impl("System", "Persona", tmp.path());
        unsafe { std::env::remove_var("KOVA_PROJECTS_ROOT") };
        if crate::cursor_prompts_enabled() {
            assert!(
                out.contains("--- Cursor rules ---"),
                "GUI prompt must include Cursor rules when enabled"
            );
            assert!(
                out.contains("Blocking Only") || out.contains("f81"),
                "GUI prompt must include baked content"
            );
        }
    }
}