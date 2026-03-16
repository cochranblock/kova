// Unlicense — cochranblock.org
//! Kova web app. API client UI — projects, prompt, backlog, intent.
//! Uses gloo_net (async) via spawn_local; results shown next frame.

use eframe::egui;
use serde::Deserialize;

#[derive(Deserialize)]
struct BacklogResponse {
    items: Vec<BacklogItem>,
}

#[derive(Deserialize)]
struct BacklogItem {
    intent: Option<String>,
    project: Option<String>,
}

pub struct KovaWebApp {
    input: String,
    messages: Vec<String>,
    show_backlog: bool,
    show_prompts: bool,
    projects: Vec<String>,
    current_project: String,
    system_prompt: String,
    persona: String,
    backlog: Vec<BacklogItem>,
    status: String,
    status_ok: Option<bool>,
    stream_output: String,
    output_code: Option<String>,
    output_hint: Option<String>,
    projects_pending: Option<std::rc::Rc<std::cell::RefCell<Vec<String>>>>,
}

impl KovaWebApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            input: String::new(),
            messages: vec!["Kova — connect to kova serve.".into()],
            show_backlog: false,
            show_prompts: false,
            projects: Vec::new(),
            current_project: String::new(),
            system_prompt: String::new(),
            persona: String::new(),
            backlog: Vec::new(),
            status: String::new(),
            status_ok: None,
            stream_output: String::new(),
            output_code: None,
            output_hint: None,
            projects_pending: None,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>, ok: Option<bool>) {
        self.status = msg.into();
        self.status_ok = ok;
    }

    fn load_projects(&mut self, ctx: &egui::Context) {
        self.set_status("Loading…", None);
        let projects = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
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
}

impl eframe::App for KovaWebApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(ref rc) = self.projects_pending {
            let list = rc.borrow().clone();
            if !list.is_empty() {
                self.projects = list;
                if self.current_project.is_empty() && !self.projects.is_empty() {
                    self.current_project = self.projects[0].clone();
                }
                self.set_status("", None);
                self.projects_pending = None;
            }
        }
        let is_narrow = ctx.content_rect().width() < 600.0;

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Kova");
                ui.add_space(8.0);
                if ui.button(if is_narrow { "☰" } else { "Backlog" }).clicked() {
                    self.show_backlog = !self.show_backlog;
                }
                if ui.button("Prompts").clicked() {
                    self.show_prompts = !self.show_prompts;
                }
                ui.add_space(8.0);
                if self.projects.is_empty() && ui.button("Load projects").clicked() {
                    self.load_projects(ctx);
                }
                egui::ComboBox::from_id_salt("project")
                    .selected_text(if self.current_project.is_empty() {
                        "(select)".into()
                    } else {
                        self.current_project
                            .rsplit('/')
                            .next()
                            .unwrap_or(&self.current_project)
                            .to_string()
                    })
                    .show_ui(ui, |ui| {
                        for p in &self.projects.clone() {
                            let name = p.rsplit('/').next().unwrap_or(p);
                            if ui.selectable_label(p == &self.current_project, name).clicked() {
                                self.current_project = p.clone();
                                ui.close();
                            }
                        }
                    });
            });
        });

        if self.show_backlog {
            egui::SidePanel::left("backlog")
                .resizable(false)
                .width_range(200.0..=400.0)
                .show(ctx, |ui| {
                    ui.heading("Backlog");
                    ui.separator();
                    for item in &self.backlog {
                        ui.label(item.intent.as_deref().unwrap_or("-"));
                    }
                });
        }

        if self.show_prompts {
            egui::SidePanel::right("prompts")
                .resizable(false)
                .width_range(200.0..=400.0)
                .show(ctx, |ui| {
                    ui.heading("Prompts");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(&self.system_prompt);
                        ui.separator();
                        ui.label(&self.persona);
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for m in &self.messages {
                    ui.label(m);
                }
                if !self.stream_output.is_empty() {
                    ui.add_space(8.0);
                    ui.monospace(&self.stream_output);
                }
            });
            if !self.status.is_empty() {
                ui.add_space(4.0);
                let color = match self.status_ok {
                    Some(true) => egui::Color32::from_rgb(0x14, 0xb8, 0xa6),
                    Some(false) => egui::Color32::from_rgb(0xa8, 0x55, 0xf7),
                    None => egui::Color32::GRAY,
                };
                ui.colored_label(color, &self.status);
            }
            if let (Some(ref code), Some(ref _hint)) = (&self.output_code, &self.output_hint) {
                let code = code.clone();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Copy").clicked() {
                        ui.ctx().output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(code)));
                        self.set_status("Copied.", Some(true));
                    }
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.input);
                if ui.button("Send").clicked() {
                    let prompt = std::mem::take(&mut self.input);
                    if !prompt.is_empty() {
                        self.messages.push(format!("You: {}", prompt));
                        self.set_status("Use kova serve for full API.", None);
                    }
                }
            });
        });
    }
}
