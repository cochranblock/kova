// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3
//! Kova WASM thin client. egui in browser, connects to kova serve.
//! Embedded from former kova-web crate.

use eframe::egui;

pub struct KovaWebApp {
    input: String,
    messages: Vec<String>,
    show_backlog: bool,
}

impl KovaWebApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            input: String::new(),
            messages: vec!["Kova thin client. Connect to kova serve.".into()],
            show_backlog: false,
        }
    }
}

impl eframe::App for KovaWebApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let is_narrow = ctx.screen_rect().width() < 600.0;

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Kova");
                ui.add_space(8.0);
                let btn_label = if is_narrow { "☰" } else { "Backlog" };
                if ui.button(btn_label).clicked() {
                    self.show_backlog = !self.show_backlog;
                }
            });
        });

        if self.show_backlog {
            egui::SidePanel::left("backlog")
                .resizable(false)
                .width_range(200.0..=400.0)
                .show(ctx, |ui| {
                    ui.heading("Backlog");
                    ui.separator();
                    ui.label("Fetch from API when connected.");
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for m in &self.messages {
                    ui.label(m);
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.input);
                if ui.button("Send").clicked() {
                    let input = std::mem::take(&mut self.input);
                    if !input.is_empty() {
                        if let Some(intent) = kova_core::f62(&input) {
                            let name = kova_core::intent_name(&intent.s0);
                            self.messages.push(format!("Intent: {}", name));
                        } else {
                            self.messages.push(format!("No match: {}", input));
                        }
                    }
                }
            });
        });
    }
}
