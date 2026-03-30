//! Kova web app. Themed API client — projects, backlog, intent, WebSocket streaming.
//! THEME.md palette: neon electric blue, teal, purple. Dark cosmic.

#![allow(non_camel_case_types, dead_code, clippy::type_complexity)]

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use crate::theme;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Deserialize, Clone)]
struct BacklogItem {
    intent: Option<String>,
    project: Option<String>,
}

#[derive(Deserialize)]
struct PromptsResponse {
    system: String,
    persona: String,
}

#[derive(Serialize)]
struct IntentPayload {
    s0: String,
    s1: String,
    project: Option<String>,
}

#[derive(Serialize)]
struct RoutePayload {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prior_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
}

#[derive(Deserialize, Clone, Default)]
struct RouteResult {
    #[serde(default)]
    classification: String,
    #[serde(default)]
    needs_clarification: Option<bool>,
    #[serde(default)]
    suggested_question: Option<String>,
    #[serde(default)]
    choices: Option<Vec<String>>,
    #[serde(default)]
    enriched_message: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Clone)]
#[allow(dead_code)] // Code + Stream used when pipeline connected
enum Msg {
    User(String),
    System(String),
    Code(String),
    Stream(String),
    Clarification { question: String, choices: Vec<String> },
    Restatement(String),
}

/// t135=KovaWebApp
pub struct t135 {
    input: String,
    messages: Vec<Msg>,
    show_backlog: bool,
    show_prompts: bool,
    projects: Vec<String>,
    current_project: String,
    system_prompt: String,
    persona: String,
    backlog: Vec<BacklogItem>,
    status: String,
    status_ok: Option<bool>,
    stream_buf: Rc<RefCell<String>>,
    streaming: bool,
    projects_loaded: bool,
    prompts_loaded: bool,
    backlog_loaded: bool,
    theme_applied: bool,
    projects_pending: Option<Rc<RefCell<Vec<String>>>>,
    prompts_pending: Option<Rc<RefCell<Option<(String, String)>>>>,
    backlog_pending: Option<Rc<RefCell<Vec<BacklogItem>>>>,
    /// Clarification flow state.
    clarification_pending: bool,
    clarification_prior: String,
    clarification_choices: Vec<String>,
    /// Restatement flow state.
    restatement_pending: bool,
    restatement_msg: String,
    /// Route result from async call.
    route_pending: Option<Rc<RefCell<Option<RouteResult>>>>,
    /// Deferred route result (processed in update where ctx is available).
    deferred_route: Option<RouteResult>,
}

impl t135 {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            input: String::new(),
            messages: vec![Msg::System("Kova — augment engine. Ready.".into())],
            show_backlog: false,
            show_prompts: false,
            projects: Vec::new(),
            current_project: String::new(),
            system_prompt: String::new(),
            persona: String::new(),
            backlog: Vec::new(),
            status: String::new(),
            status_ok: None,
            stream_buf: Rc::new(RefCell::new(String::new())),
            streaming: false,
            projects_loaded: false,
            prompts_loaded: false,
            backlog_loaded: false,
            theme_applied: false,
            projects_pending: None,
            prompts_pending: None,
            backlog_pending: None,
            clarification_pending: false,
            clarification_prior: String::new(),
            clarification_choices: Vec::new(),
            restatement_pending: false,
            restatement_msg: String::new(),
            route_pending: None,
            deferred_route: None,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>, ok: Option<bool>) {
        self.status = msg.into();
        self.status_ok = ok;
    }

    fn load_projects(&mut self, ctx: &egui::Context) {
        self.set_status("Loading projects...", None);
        let projects = Rc::new(RefCell::new(Vec::<String>::new()));
        let projects2 = projects.clone();
        let ctx2 = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(resp) = gloo_net::http::Request::get("/api/projects").send().await {
                if resp.ok() {
                    if let Ok(list) = resp.json::<Vec<String>>().await {
                        *projects2.borrow_mut() = list;
                        ctx2.request_repaint();
                    }
                }
            }
        });
        self.projects_pending = Some(projects);
    }

    fn load_prompts(&mut self, ctx: &egui::Context) {
        let sys = Rc::new(RefCell::new(None::<(String, String)>));
        let sys2 = sys.clone();
        let ctx2 = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(resp) = gloo_net::http::Request::get("/api/prompts").send().await {
                if resp.ok() {
                    if let Ok(p) = resp.json::<PromptsResponse>().await {
                        *sys2.borrow_mut() = Some((p.system, p.persona));
                        ctx2.request_repaint();
                    }
                }
            }
        });
        self.prompts_pending = Some(sys);
    }

    fn load_backlog(&mut self, ctx: &egui::Context) {
        let bl = Rc::new(RefCell::new(Vec::<BacklogItem>::new()));
        let bl2 = bl.clone();
        let ctx2 = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(resp) = gloo_net::http::Request::get("/api/backlog").send().await {
                if resp.ok() {
                    if let Ok(items) = resp.json::<Vec<BacklogItem>>().await {
                        *bl2.borrow_mut() = items;
                        ctx2.request_repaint();
                    }
                }
            }
        });
        self.backlog_pending = Some(bl);
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let input = std::mem::take(&mut self.input);
        if input.is_empty() {
            return;
        }
        self.messages.push(Msg::User(input.clone()));

        // Restatement confirm/cancel
        if self.restatement_pending {
            self.restatement_pending = false;
            let s = input.trim().to_lowercase();
            if matches!(s.as_str(), "y" | "yes") {
                self.fire_pipeline(ctx, &self.restatement_msg.clone());
            } else {
                self.messages.push(Msg::System("Cancelled.".into()));
            }
            self.restatement_msg.clear();
            return;
        }

        // Clarification reply
        if self.clarification_pending {
            self.clarification_pending = false;
            let s = input.trim().to_lowercase();
            if matches!(s.as_str(), "cancel" | "stop" | "abort") {
                self.messages.push(Msg::System("Cancelled.".into()));
                self.clarification_prior.clear();
                self.clarification_choices.clear();
                return;
            }
            // Re-route with prior context
            let prior = std::mem::take(&mut self.clarification_prior);
            self.clarification_choices.clear();
            self.route_message(ctx, &input, Some(prior));
            return;
        }

        // Normal: route the message
        self.route_message(ctx, &input, None);
    }

    fn route_message(&mut self, ctx: &egui::Context, message: &str, prior: Option<String>) {
        self.set_status("Routing...", None);
        let result_rc = Rc::new(RefCell::new(None::<RouteResult>));
        let result_rc2 = result_rc.clone();
        let ctx2 = ctx.clone();
        let project = if self.current_project.is_empty() {
            None
        } else {
            Some(self.current_project.clone())
        };
        let payload = RoutePayload {
            message: message.to_string(),
            prior_message: prior,
            project,
        };
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(req) = gloo_net::http::Request::post("/api/route").json(&payload) {
                if let Ok(resp) = req.send().await {
                    if let Ok(r) = resp.json::<RouteResult>().await {
                        *result_rc2.borrow_mut() = Some(r);
                        ctx2.request_repaint();
                    }
                }
            }
        });
        self.route_pending = Some(result_rc);
    }

    fn process_route_result(&mut self, ctx: &egui::Context, result: RouteResult) {
        if let Some(err) = &result.error {
            // No router model — fall back to direct pipeline
            if err.contains("No router model") {
                let last_user = self.messages.iter().rev().find_map(|m| {
                    if let Msg::User(t) = m { Some(t.clone()) } else { None }
                }).unwrap_or_default();
                self.fire_pipeline(ctx, &last_user);
                return;
            }
            self.messages.push(Msg::System(format!("Error: {}", err)));
            self.set_status("", None);
            return;
        }

        if result.needs_clarification == Some(true) {
            let question = result.suggested_question.unwrap_or_else(|| "Could you clarify?".into());
            let choices = result.choices.unwrap_or_default();
            self.clarification_pending = true;
            let last_user = self.messages.iter().rev().find_map(|m| {
                if let Msg::User(t) = m { Some(t.clone()) } else { None }
            }).unwrap_or_default();
            self.clarification_prior = last_user;
            self.clarification_choices = choices.clone();
            self.messages.push(Msg::Clarification { question, choices });
            self.set_status("Waiting for clarification...", None);
            return;
        }

        // Actionable classification — show restatement
        let msg = result.enriched_message.unwrap_or_else(|| {
            self.messages.iter().rev().find_map(|m| {
                if let Msg::User(t) = m { Some(t.clone()) } else { None }
            }).unwrap_or_default()
        });
        let target = self.current_project.rsplit('/').next().unwrap_or("").to_string();
        let restatement = format!(
            "I'll {} in {}. Proceed? (y/n)",
            msg, if target.is_empty() { "current project" } else { &target }
        );
        self.restatement_pending = true;
        self.restatement_msg = msg;
        self.messages.push(Msg::Restatement(restatement));
        self.set_status("Confirm to proceed...", None);
    }

    fn fire_pipeline(&mut self, ctx: &egui::Context, prompt: &str) {
        self.set_status("Generating...", None);
        self.streaming = true;

        let buf = self.stream_buf.clone();
        *buf.borrow_mut() = String::new();
        let ctx2 = ctx.clone();
        let project = if self.current_project.is_empty() {
            None
        } else {
            Some(self.current_project.clone())
        };
        let prompt = prompt.to_string();

        wasm_bindgen_futures::spawn_local(async move {
            use futures::StreamExt;
            use gloo_net::websocket::futures::WebSocket;
            use gloo_net::websocket::Message;

            let host = web_sys::window()
                .and_then(|w| w.location().host().ok())
                .unwrap_or_else(|| "127.0.0.1:3002".into());
            let proto = web_sys::window()
                .and_then(|w| w.location().protocol().ok())
                .map(|p| if p == "https:" { "wss" } else { "ws" })
                .unwrap_or("ws");
            let ws_url = format!("{}://{}/ws/stream", proto, host);

            let payload = IntentPayload {
                s0: "FullPipeline".into(),
                s1: prompt,
                project,
            };
            if let Ok(req) = gloo_net::http::Request::post("/api/intent").json(&payload) {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = req.send().await;
                });
            }

            if let Ok(ws) = WebSocket::open(&ws_url) {
                let (_write, mut read) = ws.split();
                while let Some(Ok(msg)) = read.next().await {
                    match msg {
                        Message::Text(t) => {
                            buf.borrow_mut().push_str(&t);
                            buf.borrow_mut().push('\n');
                            ctx2.request_repaint();
                        }
                        Message::Bytes(_) => {}
                    }
                }
            }
        });
    }

    fn poll_pending(&mut self) {
        if let Some(ref rc) = self.projects_pending {
            let list = rc.borrow().clone();
            if !list.is_empty() {
                self.projects = list;
                if self.current_project.is_empty() {
                    if let Some(first) = self.projects.first() {
                        self.current_project = first.clone();
                    }
                }
                self.projects_loaded = true;
                self.set_status("", None);
                self.projects_pending = None;
            }
        }
        if let Some(ref rc) = self.prompts_pending {
            let val = rc.borrow().clone();
            if let Some((sys, per)) = val {
                self.system_prompt = sys;
                self.persona = per;
                self.prompts_loaded = true;
                self.prompts_pending = None;
            }
        }
        if let Some(ref rc) = self.backlog_pending {
            let items = rc.borrow().clone();
            if !items.is_empty() {
                self.backlog = items;
                self.backlog_loaded = true;
                self.backlog_pending = None;
            }
        }
        // Route result
        if let Some(ref rc) = self.route_pending.clone() {
            let val = rc.borrow_mut().take();
            if let Some(result) = val {
                self.route_pending = None;
                // Need to clone ctx from the update loop — handled via deferred processing
                self.deferred_route = Some(result);
            }
        }
    }

    fn draw_header(&mut self, ctx: &egui::Context) {
        let is_narrow = ctx.content_rect().width() < 600.0;
        egui::TopBottomPanel::top("header")
            .frame(theme::f222())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        theme::colors::PRIMARY,
                        egui::RichText::new("KOVA").size(22.0).strong(),
                    );
                    ui.add_space(16.0);

                    let backlog_label = if is_narrow { "B" } else { "Backlog" };
                    let btn = ui.add(egui::Button::new(egui::RichText::new(backlog_label).color(
                        if self.show_backlog {
                            theme::colors::PRIMARY
                        } else {
                            theme::colors::TEXT
                        },
                    )));
                    if btn.clicked() {
                        self.show_backlog = !self.show_backlog;
                        if self.show_backlog && !self.backlog_loaded {
                            self.load_backlog(ctx);
                        }
                    }

                    let prompts_label = if is_narrow { "P" } else { "Prompts" };
                    let btn = ui.add(egui::Button::new(egui::RichText::new(prompts_label).color(
                        if self.show_prompts {
                            theme::colors::SECONDARY
                        } else {
                            theme::colors::TEXT
                        },
                    )));
                    if btn.clicked() {
                        self.show_prompts = !self.show_prompts;
                        if self.show_prompts && !self.prompts_loaded {
                            self.load_prompts(ctx);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("project_selector")
                            .selected_text(if self.current_project.is_empty() {
                                egui::RichText::new("select project").color(theme::colors::MUTED)
                            } else {
                                egui::RichText::new(
                                    self.current_project
                                        .rsplit('/')
                                        .next()
                                        .unwrap_or(&self.current_project),
                                )
                                .color(theme::colors::TERTIARY)
                            })
                            .show_ui(ui, |ui| {
                                for p in &self.projects.clone() {
                                    let name = p.rsplit('/').next().unwrap_or(p);
                                    if ui
                                        .selectable_label(p == &self.current_project, name)
                                        .clicked()
                                    {
                                        self.current_project = p.clone();
                                    }
                                }
                            });
                    });
                });
            });
    }

    fn draw_sidebars(&mut self, ctx: &egui::Context) {
        if self.show_backlog {
            egui::SidePanel::left("backlog_panel")
                .frame(theme::f227())
                .resizable(true)
                .default_width(240.0)
                .width_range(180.0..=400.0)
                .show(ctx, |ui| {
                    ui.colored_label(
                        theme::colors::PRIMARY,
                        egui::RichText::new("Backlog").size(16.0).strong(),
                    );
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    if self.backlog.is_empty() {
                        ui.colored_label(theme::colors::MUTED, "No items.");
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (i, item) in self.backlog.iter().enumerate() {
                                let text = item.intent.as_deref().unwrap_or("-");
                                let proj = item.project.as_deref().unwrap_or("");
                                theme::f223().show(ui, |ui| {
                                    ui.colored_label(theme::colors::MUTED, format!("#{}", i + 1));
                                    ui.label(text);
                                    if !proj.is_empty() {
                                        ui.colored_label(
                                            theme::colors::TERTIARY,
                                            egui::RichText::new(proj).small(),
                                        );
                                    }
                                });
                                ui.add_space(4.0);
                            }
                        });
                    }
                });
        }

        if self.show_prompts {
            egui::SidePanel::right("prompts_panel")
                .frame(theme::f227())
                .resizable(true)
                .default_width(280.0)
                .width_range(200.0..=400.0)
                .show(ctx, |ui| {
                    ui.colored_label(
                        theme::colors::SECONDARY,
                        egui::RichText::new("Prompts").size(16.0).strong(),
                    );
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if !self.system_prompt.is_empty() {
                            ui.colored_label(theme::colors::MUTED, "System");
                            theme::f225().show(ui, |ui| {
                                ui.monospace(&self.system_prompt);
                            });
                            ui.add_space(8.0);
                        }
                        if !self.persona.is_empty() {
                            ui.colored_label(theme::colors::MUTED, "Persona");
                            theme::f225().show(ui, |ui| {
                                ui.monospace(&self.persona);
                            });
                        }
                        if self.system_prompt.is_empty() && self.persona.is_empty() {
                            ui.colored_label(theme::colors::MUTED, "Loading...");
                        }
                    });
                });
        }
    }

    fn draw_messages(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_height() - 80.0; // Reserve for input
        egui::ScrollArea::vertical()
            .max_height(available.max(100.0))
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(8.0);
                for msg in &self.messages {
                    match msg {
                        Msg::User(text) => {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.allocate_ui(
                                    egui::Vec2::new(ui.available_width() * 0.75, 0.0),
                                    |ui| {
                                        theme::f224().show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(text)
                                                    .color(theme::colors::TEXT),
                                            );
                                        });
                                    },
                                );
                            });
                        }
                        Msg::System(text) => {
                            theme::f223().show(ui, |ui| {
                                ui.colored_label(theme::colors::TERTIARY, text);
                            });
                        }
                        Msg::Code(code) => {
                            let c = code.clone();
                            theme::f225().show(ui, |ui| {
                                ui.monospace(code);
                                if ui.small_button("Copy").clicked() {
                                    ui.ctx().output_mut(|o| {
                                        o.commands.push(egui::OutputCommand::CopyText(c.clone()))
                                    });
                                }
                            });
                        }
                        Msg::Stream(text) => {
                            theme::f223().show(ui, |ui| {
                                ui.monospace(text);
                            });
                        }
                        Msg::Clarification { question, choices } => {
                            theme::f223().show(ui, |ui| {
                                ui.colored_label(theme::colors::SECONDARY, question);
                                if !choices.is_empty() {
                                    ui.add_space(4.0);
                                    let letters = ['a', 'b', 'c', 'd', 'e'];
                                    for (i, choice) in choices.iter().enumerate() {
                                        let letter = letters.get(i).copied().unwrap_or('?');
                                        ui.horizontal(|ui| {
                                            ui.colored_label(
                                                theme::colors::PRIMARY,
                                                format!("({})", letter),
                                            );
                                            ui.label(choice.as_str());
                                        });
                                    }
                                }
                            });
                        }
                        Msg::Restatement(text) => {
                            theme::f223().show(ui, |ui| {
                                ui.colored_label(theme::colors::TERTIARY, text);
                            });
                        }
                    }
                    ui.add_space(6.0);
                }

                // Live stream buffer
                let buf = self.stream_buf.borrow().clone();
                if !buf.is_empty() {
                    theme::f225().show(ui, |ui| {
                        ui.colored_label(
                            theme::colors::PRIMARY,
                            egui::RichText::new("streaming...").small(),
                        );
                        ui.monospace(&buf);
                    });
                }
            });
    }

    fn draw_input(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.add_space(4.0);

        // Status bar
        if !self.status.is_empty() {
            let color = match self.status_ok {
                Some(true) => theme::colors::TERTIARY,
                Some(false) => theme::colors::SECONDARY,
                None => theme::colors::MUTED,
            };
            ui.colored_label(color, egui::RichText::new(&self.status).small());
            ui.add_space(2.0);
        }

        // Input
        theme::f226().show(ui, |ui| {
            ui.horizontal(|ui| {
                let input_width = ui.available_width() - 80.0;
                let resp = ui.add_sized(
                    [input_width.max(100.0), 28.0],
                    egui::TextEdit::singleline(&mut self.input)
                        .hint_text(
                            egui::RichText::new("augment...").color(theme::colors::MUTED),
                        )
                        .text_color(theme::colors::TEXT),
                );
                let enter =
                    resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let send = ui
                    .add(egui::Button::new(
                        egui::RichText::new("Send")
                            .color(theme::colors::PRIMARY)
                            .strong(),
                    ))
                    .clicked();
                if enter || send {
                    self.handle_input(ctx);
                }
            });
        });
    }
}

impl eframe::App for t135 {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme once
        if !self.theme_applied {
            theme::f221(ctx);
            self.theme_applied = true;
            // Auto-load projects on start
            self.load_projects(ctx);
        }

        // Poll async results
        self.poll_pending();

        // Process deferred route result (needs ctx)
        if let Some(result) = self.deferred_route.take() {
            self.process_route_result(ctx, result);
        }

        // Layout
        self.draw_header(ctx);
        self.draw_sidebars(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(theme::colors::BG)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                self.draw_messages(ui);
                self.draw_input(ctx, ui);
            });
    }
}