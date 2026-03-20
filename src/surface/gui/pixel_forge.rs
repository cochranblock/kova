// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! Pixel Forge panel — kova GUI integration.
//! Discovers pixel-forge binary, drives it via plugin protocol.
//! Renders generated sprites inline in the kova GUI.
//! Structured layout for exopack screenshot testing.

use eframe::egui;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::surface::gui::theme::{colors, layout};

const CLASS_NAMES: &[&str] = &[
    "character", "weapon", "potion", "terrain", "enemy",
    "tree", "building", "animal", "effect", "food",
    "armor", "tool", "vehicle", "ui", "misc",
];

const PALETTE_NAMES: &[&str] = &[
    "stardew", "starbound", "snes", "nes", "gameboy", "pico8", "endesga",
];

/// Discover the pixel-forge binary.
fn find_binary() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join("target/release/pixel-forge"),
        home.join("target/debug/pixel-forge"),
        home.join("bin/pixel-forge"),
        home.join(".cargo/bin/pixel-forge"),
        home.join("pixel-forge/target/release/pixel-forge"),
        home.join("pixel-forge/target/debug/pixel-forge"),
    ];

    for c in &candidates {
        if c.exists() {
            return Some(c.clone());
        }
    }

    if let Ok(output) = Command::new("which").arg("pixel-forge").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    None
}

/// Send a plugin command and get the response.
fn plugin_call(binary: &PathBuf, cmd: &str, args: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let request = serde_json::json!({
        "cmd": cmd,
        "args": args.unwrap_or(serde_json::Value::Null),
    });

    let mut child = Command::new(binary)
        .args(["plugin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = writeln!(stdin, "{}", serde_json::to_string(&request).unwrap());
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let resp: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("parse: {e}"))?;

    if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        Ok(resp.get("data").cloned().unwrap_or(serde_json::Value::Null))
    } else {
        let err = resp.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
        Err(err.to_string())
    }
}

/// A single generated sprite with metadata.
struct SpriteCard {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    class: String,
    palette: String,
}

/// Generation batch — a group of sprites from one Forge invocation.
struct ForgeBatch {
    sprites: Vec<SpriteCard>,
    class: String,
    palette: String,
    tier: String,
    elapsed_ms: u64,
    textures: Vec<egui::TextureHandle>,
}

/// Generation result from background thread.
struct GenResult {
    sprites: Vec<SpriteCard>,
    elapsed_ms: u64,
    tier: String,
    status: String,
}

/// Pixel Forge panel state inside kova GUI.
pub struct T220 {
    binary: Option<PathBuf>,
    binary_error: Option<String>,
    device_info: String,
    tier: String,
    models_info: String,
    version_info: String,

    selected_class: usize,
    selected_palette: usize,
    gen_count: u32,
    gen_steps: usize,

    generating: Arc<Mutex<bool>>,
    gen_result: Arc<Mutex<Option<GenResult>>>,

    /// Gallery of all batches from this session.
    batches: Vec<ForgeBatch>,
    status: String,
    total_generated: u32,
}

impl T220 {
    pub fn new() -> Self {
        let binary = find_binary();
        let mut device_info = String::new();
        let mut models_info = String::new();
        let mut version_info = String::new();
        let mut tier = String::new();
        let mut binary_error = None;

        if let Some(bin) = &binary {
            match plugin_call(bin, "version", None) {
                Ok(data) => {
                    let v = data.get("version").and_then(|v| v.as_str()).unwrap_or("?");
                    version_info = format!("v{v}");
                }
                Err(_) => {}
            }
            match plugin_call(bin, "probe", None) {
                Ok(data) => {
                    let backend = data.get("backend").and_then(|v| v.as_str()).unwrap_or("?");
                    let ram = data.get("ram_mb").and_then(|v| v.as_u64()).unwrap_or(0);
                    let t = data.get("tier").and_then(|v| v.as_str()).unwrap_or("?");
                    tier = t.to_string();
                    device_info = format!("{} | {}MB | {}", backend, ram, t);
                }
                Err(e) => device_info = format!("probe failed: {e}"),
            }
            match plugin_call(bin, "models", None) {
                Ok(data) => {
                    if let Some(models) = data.get("models").and_then(|v| v.as_array()) {
                        let parts: Vec<String> = models.iter().filter_map(|m| {
                            let t = m.get("tier").and_then(|v| v.as_str())?;
                            let exists = m.get("exists").and_then(|v| v.as_bool()).unwrap_or(false);
                            if exists {
                                let size = m.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                Some(format!("{} {:.1}MB", t, size as f64 / 1_048_576.0))
                            } else {
                                None
                            }
                        }).collect();
                        models_info = parts.join("  ");
                    }
                }
                Err(_) => {}
            }
        } else {
            binary_error = Some("pixel-forge not found — cargo build -p pixel-forge".to_string());
        }

        Self {
            binary,
            binary_error,
            device_info,
            tier,
            models_info,
            version_info,
            selected_class: 0,
            selected_palette: 0,
            gen_count: 4,
            gen_steps: 40,
            generating: Arc::new(Mutex::new(false)),
            gen_result: Arc::new(Mutex::new(None)),
            batches: Vec::new(),
            status: String::new(),
            total_generated: 0,
        }
    }

    fn start_generation(&mut self, ctx: &egui::Context) {
        let bin = match &self.binary {
            Some(b) => b.clone(),
            None => return,
        };

        let class = CLASS_NAMES[self.selected_class].to_string();
        let palette = PALETTE_NAMES[self.selected_palette].to_string();
        let count = self.gen_count;
        let steps = self.gen_steps;
        let tier = self.tier.clone();

        let generating = Arc::clone(&self.generating);
        let gen_result = Arc::clone(&self.gen_result);
        let ctx = ctx.clone();

        *generating.lock().unwrap() = true;
        self.status = format!("forging {} {}s via {}...", count, class, tier);

        std::thread::spawn(move || {
            let t0 = Instant::now();
            let args = serde_json::json!({
                "class": class,
                "count": count,
                "steps": steps,
                "palette": palette,
            });

            let result = match plugin_call(&bin, "generate", Some(args)) {
                Ok(data) => {
                    let sprites: Vec<SpriteCard> = data
                        .get("sprites")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|s| {
                                let b64 = s.get("png_b64").and_then(|v| v.as_str())?;
                                use base64::Engine as _;
                                let engine = base64::engine::general_purpose::STANDARD;
                                let png_bytes = engine.decode(b64).ok()?;
                                let img = image::load_from_memory(&png_bytes).ok()?.to_rgba8();
                                Some(SpriteCard {
                                    width: img.width(),
                                    height: img.height(),
                                    rgba: img.into_raw(),
                                    class: class.clone(),
                                    palette: palette.clone(),
                                })
                            }).collect()
                        })
                        .unwrap_or_default();

                    let elapsed = t0.elapsed().as_millis() as u64;
                    let n = sprites.len();
                    GenResult {
                        sprites,
                        elapsed_ms: elapsed,
                        tier: tier.clone(),
                        status: format!("{} sprites in {:.1}s via {}", n, elapsed as f64 / 1000.0, tier),
                    }
                }
                Err(e) => GenResult {
                    sprites: Vec::new(),
                    elapsed_ms: t0.elapsed().as_millis() as u64,
                    tier: tier.clone(),
                    status: format!("error: {e}"),
                },
            };

            *gen_result.lock().unwrap() = Some(result);
            *generating.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }

    /// Render the Pixel Forge panel. Returns true if user wants to close.
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
        let mut close = false;

        // ── Header ──
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Pixel Forge")
                    .color(colors::PRIMARY)
                    .size(20.0)
                    .strong(),
            );
            if !self.version_info.is_empty() {
                ui.label(egui::RichText::new(&self.version_info).color(colors::MUTED).small());
            }
            ui.add_space(layout::MARGIN);
            if !self.device_info.is_empty() {
                ui.label(egui::RichText::new(&self.device_info).color(colors::TERTIARY).small());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
                if self.total_generated > 0 {
                    ui.label(egui::RichText::new(format!("{} total", self.total_generated)).color(colors::MUTED).small());
                }
            });
        });

        if let Some(err) = &self.binary_error {
            ui.add_space(layout::GAP);
            ui.colored_label(egui::Color32::from_rgb(0xff, 0x66, 0x66), err);
            return close;
        }

        if !self.models_info.is_empty() {
            ui.label(egui::RichText::new(&self.models_info).color(colors::MUTED).small());
        }

        ui.separator();

        // ── Controls ──
        ui.add_space(layout::PADDING_SM);

        // Class grid
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("class").color(colors::MUTED));
            ui.add_space(4.0);
            for (i, name) in CLASS_NAMES.iter().enumerate() {
                let selected = self.selected_class == i;
                let color = if selected { colors::PRIMARY } else { colors::TEXT };
                let btn = egui::Button::new(egui::RichText::new(*name).color(color).small())
                    .min_size(egui::vec2(48.0, 22.0))
                    .selected(selected);
                if ui.add(btn).clicked() {
                    self.selected_class = i;
                }
            }
        });

        ui.add_space(4.0);

        // Palette + count + steps row
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("palette").color(colors::MUTED));
            ui.add_space(4.0);
            for (i, name) in PALETTE_NAMES.iter().enumerate() {
                let selected = self.selected_palette == i;
                let color = if selected { colors::SECONDARY } else { colors::TEXT };
                let btn = egui::Button::new(egui::RichText::new(*name).color(color).small())
                    .min_size(egui::vec2(44.0, 20.0))
                    .selected(selected);
                if ui.add(btn).clicked() {
                    self.selected_palette = i;
                }
            }
            ui.add_space(layout::MARGIN);
            ui.label(egui::RichText::new("n").color(colors::MUTED));
            ui.add(egui::DragValue::new(&mut self.gen_count).range(1..=32).speed(0.2));
            ui.add_space(4.0);
            ui.label(egui::RichText::new("steps").color(colors::MUTED));
            ui.add(egui::DragValue::new(&mut self.gen_steps).range(10..=100).speed(1.0));
        });

        ui.add_space(layout::GAP);

        // ── Forge button + status ──
        let is_generating = *self.generating.lock().unwrap();
        let can_gen = self.binary.is_some() && !is_generating;

        ui.horizontal(|ui| {
            let label = if is_generating {
                egui::RichText::new("forging...").color(colors::SECONDARY)
            } else {
                egui::RichText::new("Forge").color(colors::BG).strong()
            };
            let btn = egui::Button::new(label)
                .min_size(egui::vec2(100.0, 32.0))
                .fill(if can_gen { colors::PRIMARY } else { colors::SURFACE_ELEVATED });
            if ui.add_enabled(can_gen, btn).clicked() {
                self.start_generation(ctx);
            }
            ui.add_space(layout::GAP);
            if !self.status.is_empty() {
                ui.label(egui::RichText::new(&self.status).color(colors::MUTED));
            }
        });

        // ── Collect results ──
        {
            let mut result_lock = self.gen_result.lock().unwrap();
            if let Some(result) = result_lock.take() {
                self.status = result.status;
                let n = result.sprites.len() as u32;
                self.total_generated += n;

                let class = result.sprites.first().map(|s| s.class.clone()).unwrap_or_default();
                let palette = result.sprites.first().map(|s| s.palette.clone()).unwrap_or_default();

                let mut textures = Vec::new();
                for sprite in &result.sprites {
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [sprite.width as usize, sprite.height as usize],
                        &sprite.rgba,
                    );
                    let opts = egui::TextureOptions {
                        magnification: egui::TextureFilter::Nearest,
                        minification: egui::TextureFilter::Nearest,
                        ..Default::default()
                    };
                    textures.push(ctx.load_texture(
                        format!("pf_{}_{}", self.batches.len(), textures.len()),
                        color_image,
                        opts,
                    ));
                }

                self.batches.push(ForgeBatch {
                    sprites: result.sprites,
                    class,
                    palette,
                    tier: result.tier,
                    elapsed_ms: result.elapsed_ms,
                    textures,
                });
            }
        }

        // ── Gallery ──
        if !self.batches.is_empty() {
            ui.add_space(layout::GAP);
            ui.separator();
            ui.add_space(layout::PADDING_SM);

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Most recent batch first
                for (batch_idx, batch) in self.batches.iter().rev().enumerate() {
                    // Batch header
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&batch.class)
                                .color(colors::PRIMARY)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(&batch.palette)
                                .color(colors::SECONDARY)
                                .small(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} via {} in {:.1}s",
                                batch.textures.len(),
                                batch.tier,
                                batch.elapsed_ms as f64 / 1000.0
                            ))
                            .color(colors::MUTED)
                            .small(),
                        );
                    });

                    ui.add_space(4.0);

                    // Sprite row — each sprite in a bordered card
                    ui.horizontal_wrapped(|ui| {
                        let scale = 5.0; // 16x16 → 80x80 display
                        for tex in &batch.textures {
                            let size = tex.size_vec2() * scale;
                            egui::Frame::NONE
                                .fill(colors::BG)
                                .corner_radius(egui::CornerRadius::same(layout::RADIUS_SM_U8))
                                .stroke(egui::Stroke::new(1.0, colors::SURFACE_ELEVATED))
                                .inner_margin(egui::Margin::same(4))
                                .show(ui, |ui| {
                                    ui.image(egui::load::SizedTexture::new(tex.id(), size));
                                });
                        }
                    });

                    if batch_idx < self.batches.len() - 1 {
                        ui.add_space(layout::GAP);
                    }
                }
            });
        }

        close
    }
}
