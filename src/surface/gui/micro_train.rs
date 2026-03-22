// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! Micro Train panel — kova GUI integration for candle fine-tuning.
//! Follows pixel_forge.rs (T220) pattern: config controls, async training,
//! progress display, trained model gallery.

use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::surface::gui::theme::{colors, layout};

const SPECIALIST_NAMES: &[&str] = &[
    "kova-rustfix", "kova-tokenizer", "kova-architect", "kova-reviewer", "custom",
];

const DATA_FORMATS: &[&str] = &["sft", "dpo"];

/// Progress update from training thread.
#[derive(Clone)]
struct TrainProgress {
    epoch: u32,
    total_epochs: u32,
    loss: f64,
    status: String,
}

/// Result from a completed training run.
struct TrainResult {
    name: String,
    output_dir: PathBuf,
    epochs: u32,
    final_loss: f64,
    elapsed_ms: u64,
    status: String,
    loss_history: Vec<f64>,
}

/// A trained model card in the gallery.
struct ModelCard {
    name: String,
    path: PathBuf,
    epochs: u32,
    final_loss: f64,
    elapsed_ms: u64,
    loss_history: Vec<f64>,
}

/// T222 — Micro Train panel state inside kova GUI.
pub struct T222 {
    /// Base model directory (safetensors).
    base_model: String,
    /// Training data directory.
    training_dir: String,
    /// Output directory for trained models.
    output_dir: String,

    selected_specialist: usize,
    custom_name: String,
    selected_format: usize,
    epochs: u32,
    lr_exp: i32,
    max_seq_len: usize,
    batch_size: usize,

    training: Arc<Mutex<bool>>,
    progress: Arc<Mutex<Option<TrainProgress>>>,
    result: Arc<Mutex<Option<TrainResult>>>,

    /// Gallery of trained models from this session.
    models: Vec<ModelCard>,
    /// Live loss history during training.
    live_loss: Arc<Mutex<Vec<f64>>>,
    status: String,
    total_trained: u32,
}

impl T222 {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let kova_dir = PathBuf::from(&home).join(".kova");

        // Find base model
        let base_model = crate::mobile_llm::find_model()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| kova_dir.join("models").display().to_string());

        let training_dir = kova_dir.join("micro").join("training").display().to_string();
        let output_dir = kova_dir.join("models").display().to_string();

        // Scan for existing trained models
        let models = scan_trained_models(&PathBuf::from(&output_dir));

        let total = models.len() as u32;

        Self {
            base_model,
            training_dir,
            output_dir,
            selected_specialist: 0,
            custom_name: String::new(),
            selected_format: 0,
            epochs: 3,
            lr_exp: -5,
            max_seq_len: 512,
            batch_size: 1,
            training: Arc::new(Mutex::new(false)),
            progress: Arc::new(Mutex::new(None)),
            result: Arc::new(Mutex::new(None)),
            models,
            live_loss: Arc::new(Mutex::new(Vec::new())),
            status: String::new(),
            total_trained: total,
        }
    }

    fn specialist_name(&self) -> String {
        if self.selected_specialist == SPECIALIST_NAMES.len() - 1 {
            if self.custom_name.is_empty() {
                "kova-custom".into()
            } else {
                self.custom_name.clone()
            }
        } else {
            SPECIALIST_NAMES[self.selected_specialist].to_string()
        }
    }

    fn data_file(&self) -> PathBuf {
        let dir = PathBuf::from(&self.training_dir);
        match DATA_FORMATS[self.selected_format] {
            "dpo" => dir.join("dpo_chatml.jsonl"),
            _ => dir.join("sft_chatml.jsonl"),
        }
    }

    fn start_training(&mut self, ctx: &egui::Context) {
        let base_model = PathBuf::from(&self.base_model);
        let data_path = self.data_file();
        let output_dir = PathBuf::from(&self.output_dir);
        let name = self.specialist_name();
        let epochs = self.epochs;
        let lr = 10.0_f64.powi(self.lr_exp);
        let max_seq_len = self.max_seq_len;
        let batch_size = self.batch_size;

        if !data_path.exists() {
            self.status = format!("no data: {} — run kova micro export --format all", data_path.display());
            return;
        }
        if !base_model.is_dir() {
            self.status = format!("no base model at {} — run kova model install", base_model.display());
            return;
        }

        let training = Arc::clone(&self.training);
        let progress = Arc::clone(&self.progress);
        let result = Arc::clone(&self.result);
        let live_loss = Arc::clone(&self.live_loss);
        let ctx = ctx.clone();

        *training.lock().unwrap() = true;
        *live_loss.lock().unwrap() = Vec::new();
        self.status = format!("training {} — {} epochs...", name, epochs);

        std::thread::spawn(move || {
            let t0 = Instant::now();

            let config = crate::micro::candle_train::TrainConfig {
                base_model,
                data_path,
                output_dir: output_dir.clone(),
                name: name.clone(),
                epochs,
                lr,
                max_seq_len,
                batch_size,
            };

            let train_result = match crate::micro::candle_train::train_sft(&config) {
                Ok(out_dir) => {
                    let elapsed = t0.elapsed().as_millis() as u64;
                    let losses = live_loss.lock().unwrap().clone();
                    let final_loss = losses.last().copied().unwrap_or(0.0);
                    TrainResult {
                        name: name.clone(),
                        output_dir: out_dir,
                        epochs,
                        final_loss,
                        elapsed_ms: elapsed,
                        status: format!("{} trained in {:.1}s — loss {:.4}", name, elapsed as f64 / 1000.0, final_loss),
                        loss_history: losses,
                    }
                }
                Err(e) => TrainResult {
                    name: name.clone(),
                    output_dir: output_dir.join(&name),
                    epochs,
                    final_loss: 0.0,
                    elapsed_ms: t0.elapsed().as_millis() as u64,
                    status: format!("error: {}", e),
                    loss_history: Vec::new(),
                },
            };

            *result.lock().unwrap() = Some(train_result);
            *training.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }

    /// Render the Micro Train panel. Returns true if user wants to close.
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
        let mut close = false;

        // ── Header ──
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Micro Train")
                    .color(colors::TERTIARY)
                    .size(20.0)
                    .strong(),
            );
            ui.add_space(layout::MARGIN);
            ui.label(
                egui::RichText::new("candle fine-tuning")
                    .color(colors::MUTED)
                    .small(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
                if self.total_trained > 0 {
                    ui.label(
                        egui::RichText::new(format!("{} models", self.total_trained))
                            .color(colors::MUTED)
                            .small(),
                    );
                }
            });
        });

        ui.separator();

        // ── Paths ──
        ui.add_space(layout::PADDING_SM);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("base model").color(colors::MUTED));
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(&mut self.base_model).desired_width(400.0));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("training data").color(colors::MUTED));
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(&mut self.training_dir).desired_width(400.0));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("output").color(colors::MUTED));
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(&mut self.output_dir).desired_width(400.0));
        });

        ui.add_space(layout::GAP);

        // ── Specialist grid ──
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("specialist").color(colors::MUTED));
            ui.add_space(4.0);
            for (i, name) in SPECIALIST_NAMES.iter().enumerate() {
                let selected = self.selected_specialist == i;
                let color = if selected { colors::TERTIARY } else { colors::TEXT };
                let btn = egui::Button::new(egui::RichText::new(*name).color(color).small())
                    .min_size(egui::vec2(80.0, 22.0))
                    .selected(selected);
                if ui.add(btn).clicked() {
                    self.selected_specialist = i;
                }
            }
        });

        if self.selected_specialist == SPECIALIST_NAMES.len() - 1 {
            ui.horizontal(|ui| {
                ui.add_space(80.0);
                ui.label(egui::RichText::new("name").color(colors::MUTED));
                ui.add(egui::TextEdit::singleline(&mut self.custom_name).desired_width(200.0));
            });
        }

        ui.add_space(4.0);

        // ── Format + hyperparams ──
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("format").color(colors::MUTED));
            ui.add_space(4.0);
            for (i, name) in DATA_FORMATS.iter().enumerate() {
                let selected = self.selected_format == i;
                let color = if selected { colors::SECONDARY } else { colors::TEXT };
                let btn = egui::Button::new(egui::RichText::new(*name).color(color).small())
                    .min_size(egui::vec2(44.0, 20.0))
                    .selected(selected);
                if ui.add(btn).clicked() {
                    self.selected_format = i;
                }
            }
            ui.add_space(layout::MARGIN);
            ui.label(egui::RichText::new("epochs").color(colors::MUTED));
            ui.add(egui::DragValue::new(&mut self.epochs).range(1..=50).speed(0.2));
            ui.add_space(4.0);
            ui.label(egui::RichText::new("lr 1e").color(colors::MUTED));
            ui.add(egui::DragValue::new(&mut self.lr_exp).range(-8..=-2).speed(0.1));
            ui.add_space(4.0);
            ui.label(egui::RichText::new("seq").color(colors::MUTED));
            ui.add(egui::DragValue::new(&mut self.max_seq_len).range(64..=2048).speed(4.0));
            ui.add_space(4.0);
            ui.label(egui::RichText::new("batch").color(colors::MUTED));
            ui.add(egui::DragValue::new(&mut self.batch_size).range(1..=16).speed(0.2));
        });

        ui.add_space(layout::GAP);

        // ── Train button + status ──
        let is_training = *self.training.lock().unwrap();
        let can_train = !is_training;

        ui.horizontal(|ui| {
            let label = if is_training {
                egui::RichText::new("training...").color(colors::SECONDARY)
            } else {
                egui::RichText::new("Train").color(colors::BG).strong()
            };
            let btn = egui::Button::new(label)
                .min_size(egui::vec2(100.0, 32.0))
                .fill(if can_train { colors::TERTIARY } else { colors::SURFACE_ELEVATED });
            if ui.add_enabled(can_train, btn).clicked() {
                self.start_training(ctx);
            }

            // Train All button
            let all_label = egui::RichText::new("Train All").color(if can_train { colors::TEXT } else { colors::MUTED }).small();
            let all_btn = egui::Button::new(all_label).min_size(egui::vec2(72.0, 28.0));
            if ui.add_enabled(can_train, all_btn).clicked() {
                self.start_train_all(ctx);
            }

            ui.add_space(layout::GAP);
            if !self.status.is_empty() {
                ui.label(egui::RichText::new(&self.status).color(colors::MUTED));
            }
        });

        // ── Progress (live loss) ──
        if is_training {
            let losses = self.live_loss.lock().unwrap().clone();
            if !losses.is_empty() {
                ui.add_space(layout::PADDING_SM);
                draw_loss_sparkline(ui, &losses);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        // ── Collect results ──
        {
            let mut result_lock = self.result.lock().unwrap();
            if let Some(result) = result_lock.take() {
                self.status = result.status;
                self.total_trained += 1;

                self.models.push(ModelCard {
                    name: result.name,
                    path: result.output_dir,
                    epochs: result.epochs,
                    final_loss: result.final_loss,
                    elapsed_ms: result.elapsed_ms,
                    loss_history: result.loss_history,
                });
            }
        }

        // ── Model Gallery ──
        if !self.models.is_empty() {
            ui.add_space(layout::GAP);
            ui.separator();
            ui.add_space(layout::PADDING_SM);

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, model) in self.models.iter().rev().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&model.name)
                                .color(colors::TERTIARY)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{}ep loss={:.4} {:.1}s",
                                model.epochs,
                                model.final_loss,
                                model.elapsed_ms as f64 / 1000.0,
                            ))
                            .color(colors::MUTED)
                            .small(),
                        );
                    });

                    // Mini loss sparkline per model
                    if !model.loss_history.is_empty() {
                        draw_loss_sparkline(ui, &model.loss_history);
                    }

                    ui.label(
                        egui::RichText::new(model.path.display().to_string())
                            .color(colors::MUTED)
                            .small(),
                    );

                    if i < self.models.len() - 1 {
                        ui.add_space(layout::GAP);
                    }
                }
            });
        }

        close
    }

    fn start_train_all(&mut self, ctx: &egui::Context) {
        let base_model = PathBuf::from(&self.base_model);
        let training_dir = PathBuf::from(&self.training_dir);
        let output_dir = PathBuf::from(&self.output_dir);
        let epochs = self.epochs;
        let lr = 10.0_f64.powi(self.lr_exp);
        let max_seq_len = self.max_seq_len;
        let batch_size = self.batch_size;

        if !base_model.is_dir() {
            self.status = format!("no base model at {}", base_model.display());
            return;
        }

        let training = Arc::clone(&self.training);
        let result = Arc::clone(&self.result);
        let live_loss = Arc::clone(&self.live_loss);
        let ctx = ctx.clone();

        *training.lock().unwrap() = true;
        *live_loss.lock().unwrap() = Vec::new();
        self.status = "training all specialists...".into();

        std::thread::spawn(move || {
            let t0 = Instant::now();

            match crate::micro::candle_train::train_all_specialists(
                &base_model,
                &training_dir,
                &output_dir,
            ) {
                Ok(paths) => {
                    let elapsed = t0.elapsed().as_millis() as u64;
                    let n = paths.len();
                    *result.lock().unwrap() = Some(TrainResult {
                        name: format!("{} specialists", n),
                        output_dir: paths.first().cloned().unwrap_or_default(),
                        epochs,
                        final_loss: 0.0,
                        elapsed_ms: elapsed,
                        status: format!("{} specialists trained in {:.1}s", n, elapsed as f64 / 1000.0),
                        loss_history: Vec::new(),
                    });
                }
                Err(e) => {
                    *result.lock().unwrap() = Some(TrainResult {
                        name: "train_all".into(),
                        output_dir: PathBuf::new(),
                        epochs,
                        final_loss: 0.0,
                        elapsed_ms: t0.elapsed().as_millis() as u64,
                        status: format!("error: {}", e),
                        loss_history: Vec::new(),
                    });
                }
            }

            *training.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }
}

/// Scan output directory for existing trained models.
fn scan_trained_models(output_dir: &PathBuf) -> Vec<ModelCard> {
    let mut models = Vec::new();
    let Ok(entries) = std::fs::read_dir(output_dir) else { return models };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }

        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Only show kova-* specialist dirs that have model.safetensors
        if !name.starts_with("kova-") { continue; }
        if !path.join("model.safetensors").exists() { continue; }

        models.push(ModelCard {
            name,
            path: path.clone(),
            epochs: 0,
            final_loss: 0.0,
            elapsed_ms: 0,
            loss_history: Vec::new(),
        });
    }

    models
}

/// Draw a simple loss sparkline using egui painter.
fn draw_loss_sparkline(ui: &mut egui::Ui, losses: &[f64]) {
    if losses.is_empty() { return; }

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(200.0, 32.0),
        egui::Sense::hover(),
    );

    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, layout::RADIUS_SM, colors::SURFACE);

    let max_loss = losses.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_loss = losses.iter().cloned().fold(f64::INFINITY, f64::min);
    let range = (max_loss - min_loss).max(1e-6);

    let n = losses.len();
    if n < 2 { return; }

    let points: Vec<egui::Pos2> = losses.iter().enumerate().map(|(i, &loss)| {
        let x = rect.left() + (i as f32 / (n - 1) as f32) * rect.width();
        let y = rect.bottom() - ((loss - min_loss) as f32 / range as f32) * (rect.height() - 4.0) - 2.0;
        egui::pos2(x, y)
    }).collect();

    for w in points.windows(2) {
        painter.line_segment([w[0], w[1]], egui::Stroke::new(1.5, colors::TERTIARY));
    }

    // Labels
    painter.text(
        rect.left_top() + egui::vec2(2.0, 1.0),
        egui::Align2::LEFT_TOP,
        format!("{:.4}", max_loss),
        egui::FontId::proportional(9.0),
        colors::MUTED,
    );
    painter.text(
        rect.right_bottom() + egui::vec2(-2.0, -1.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.4}", min_loss),
        egui::FontId::proportional(9.0),
        colors::MUTED,
    );
}
