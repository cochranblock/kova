// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! Pixel Forge panel — kova GUI integration.
//! Discovers pixel-forge binary, drives it via plugin protocol.
//! Renders generated sprites inline in the kova GUI.

use eframe::egui;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

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
    // Check common locations
    let candidates = [
        // Workspace target (cargo build)
        dirs::home_dir().map(|h| h.join("target/release/pixel-forge")),
        dirs::home_dir().map(|h| h.join("target/debug/pixel-forge")),
        // Direct install
        dirs::home_dir().map(|h| h.join("bin/pixel-forge")),
        dirs::home_dir().map(|h| h.join(".cargo/bin/pixel-forge")),
        // Project dir
        dirs::home_dir().map(|h| h.join("pixel-forge/target/release/pixel-forge")),
        dirs::home_dir().map(|h| h.join("pixel-forge/target/debug/pixel-forge")),
    ];

    for c in &candidates {
        if let Some(path) = c {
            if path.exists() {
                return Some(path.clone());
            }
        }
    }

    // Try PATH
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

    if let Some(ref mut stdin) = child.stdin {
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

/// State for the persistent plugin process (loop mode).
struct PluginProcess {
    child: Child,
}

impl PluginProcess {
    fn spawn(binary: &PathBuf) -> Result<Self, String> {
        let child = Command::new(binary)
            .args(["plugin", "--loop"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn: {e}"))?;
        Ok(Self { child })
    }

    fn call(&mut self, cmd: &str, args: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
        let request = serde_json::json!({
            "cmd": cmd,
            "args": args.unwrap_or(serde_json::Value::Null),
        });

        let stdin = self.child.stdin.as_mut().ok_or("no stdin")?;
        writeln!(stdin, "{}", serde_json::to_string(&request).unwrap())
            .map_err(|e| format!("write: {e}"))?;
        stdin.flush().map_err(|e| format!("flush: {e}"))?;

        // Read one line of response
        use std::io::BufRead;
        let stdout = self.child.stdout.as_mut().ok_or("no stdout")?;
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| format!("read: {e}"))?;

        let resp: serde_json::Value = serde_json::from_str(line.trim())
            .map_err(|e| format!("parse: {e}"))?;

        if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            Ok(resp.get("data").cloned().unwrap_or(serde_json::Value::Null))
        } else {
            let err = resp.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
            Err(err.to_string())
        }
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Generation result from background thread.
struct GenResult {
    sprites: Vec<SpriteData>,
    status: String,
}

struct SpriteData {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Pixel Forge panel state inside kova GUI.
pub struct T220 {
    binary: Option<PathBuf>,
    binary_error: Option<String>,
    /// Device profile info string.
    device_info: String,
    /// Available model tiers.
    models_info: String,

    selected_class: usize,
    selected_palette: usize,
    gen_count: u32,
    gen_steps: usize,

    /// Background generation state.
    generating: Arc<Mutex<bool>>,
    gen_result: Arc<Mutex<Option<GenResult>>>,

    /// Rendered sprite textures.
    textures: Vec<egui::TextureHandle>,
    status: String,
}

impl T220 {
    /// Create panel, discover binary, probe device.
    pub fn new() -> Self {
        let binary = find_binary();
        let mut device_info = String::new();
        let mut models_info = String::new();
        let mut binary_error = None;

        if let Some(ref bin) = binary {
            // Probe device
            match plugin_call(bin, "probe", None) {
                Ok(data) => {
                    let backend = data.get("backend").and_then(|v| v.as_str()).unwrap_or("?");
                    let ram = data.get("ram_mb").and_then(|v| v.as_u64()).unwrap_or(0);
                    let tier = data.get("tier").and_then(|v| v.as_str()).unwrap_or("?");
                    device_info = format!("{} | {} MB | {}", backend, ram, tier);
                }
                Err(e) => device_info = format!("probe failed: {e}"),
            }
            // Get models
            match plugin_call(bin, "models", None) {
                Ok(data) => {
                    if let Some(models) = data.get("models").and_then(|v| v.as_array()) {
                        let parts: Vec<String> = models.iter().filter_map(|m| {
                            let tier = m.get("tier").and_then(|v| v.as_str())?;
                            let exists = m.get("exists").and_then(|v| v.as_bool()).unwrap_or(false);
                            if exists {
                                let size = m.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
                                Some(format!("{} ({:.1}MB)", tier, size as f64 / 1_048_576.0))
                            } else {
                                None
                            }
                        }).collect();
                        models_info = parts.join(" | ");
                    }
                }
                Err(_) => {}
            }
        } else {
            binary_error = Some("pixel-forge binary not found".to_string());
        }

        Self {
            binary,
            binary_error,
            device_info,
            models_info,
            selected_class: 0,
            selected_palette: 0,
            gen_count: 4,
            gen_steps: 40,
            generating: Arc::new(Mutex::new(false)),
            gen_result: Arc::new(Mutex::new(None)),
            textures: Vec::new(),
            status: String::new(),
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

        let generating = Arc::clone(&self.generating);
        let gen_result = Arc::clone(&self.gen_result);
        let ctx = ctx.clone();

        *generating.lock().unwrap() = true;
        self.status = format!("forging {} {}s...", count, class);
        self.textures.clear();

        std::thread::spawn(move || {
            let args = serde_json::json!({
                "class": class,
                "count": count,
                "steps": steps,
                "palette": palette,
            });

            let result = match plugin_call(&bin, "generate", Some(args)) {
                Ok(data) => {
                    let sprites: Vec<SpriteData> = data
                        .get("sprites")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|s| {
                                let b64 = s.get("png_b64").and_then(|v| v.as_str())?;
                                use base64::Engine as _;
                                let engine = base64::engine::general_purpose::STANDARD;
                                let png_bytes = engine.decode(b64).ok()?;
                                let img = image::load_from_memory(&png_bytes).ok()?.to_rgba8();
                                Some(SpriteData {
                                    width: img.width(),
                                    height: img.height(),
                                    rgba: img.into_raw(),
                                })
                            }).collect()
                        })
                        .unwrap_or_default();

                    let n = sprites.len();
                    GenResult {
                        sprites,
                        status: format!("done — {} sprites", n),
                    }
                }
                Err(e) => GenResult {
                    sprites: Vec::new(),
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

        // Header
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Pixel Forge")
                    .color(colors::PRIMARY)
                    .size(20.0)
                    .strong(),
            );
            ui.add_space(layout::MARGIN);
            if !self.device_info.is_empty() {
                ui.label(egui::RichText::new(&self.device_info).color(colors::MUTED));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

        if let Some(ref err) = self.binary_error {
            ui.colored_label(egui::Color32::RED, err);
            return close;
        }

        if !self.models_info.is_empty() {
            ui.label(egui::RichText::new(&self.models_info).color(colors::MUTED).small());
        }

        ui.separator();
        ui.add_space(4.0);

        // Class selector
        ui.label("class:");
        egui::Grid::new("pf_class_grid")
            .num_columns(5)
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for (i, name) in CLASS_NAMES.iter().enumerate() {
                    let selected = self.selected_class == i;
                    let btn = egui::Button::new(*name)
                        .min_size(egui::vec2(60.0, 28.0))
                        .selected(selected);
                    if ui.add(btn).clicked() {
                        self.selected_class = i;
                    }
                    if (i + 1) % 5 == 0 {
                        ui.end_row();
                    }
                }
            });

        ui.add_space(8.0);

        // Palette
        ui.label("palette:");
        ui.horizontal_wrapped(|ui| {
            for (i, name) in PALETTE_NAMES.iter().enumerate() {
                let selected = self.selected_palette == i;
                let btn = egui::Button::new(*name)
                    .min_size(egui::vec2(52.0, 24.0))
                    .selected(selected);
                if ui.add(btn).clicked() {
                    self.selected_palette = i;
                }
            }
        });

        ui.add_space(8.0);

        // Count + steps
        ui.horizontal(|ui| {
            ui.label("count:");
            ui.add(egui::Slider::new(&mut self.gen_count, 1..=16));
            ui.add_space(8.0);
            ui.label("steps:");
            ui.add(egui::Slider::new(&mut self.gen_steps, 10..=100));
        });

        ui.add_space(8.0);

        // Generate button
        let is_generating = *self.generating.lock().unwrap();
        let can_gen = self.binary.is_some() && !is_generating;

        ui.horizontal(|ui| {
            let btn = egui::Button::new(if is_generating { "forging..." } else { "Forge" })
                .min_size(egui::vec2(120.0, 36.0));
            if ui.add_enabled(can_gen, btn).clicked() {
                self.start_generation(ctx);
            }
            ui.add_space(8.0);
            if !self.status.is_empty() {
                ui.label(egui::RichText::new(&self.status).color(colors::MUTED));
            }
        });

        // Collect results from background thread
        {
            let mut result_lock = self.gen_result.lock().unwrap();
            if let Some(result) = result_lock.take() {
                self.status = result.status;
                self.textures.clear();
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
                    self.textures.push(ctx.load_texture(
                        format!("pf_sprite_{}", self.textures.len()),
                        color_image,
                        opts,
                    ));
                }
            }
        }

        // Display sprites
        if !self.textures.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let scale = 6.0; // 16x16 → 96x96 display
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for tex in &self.textures {
                        let size = tex.size_vec2() * scale;
                        ui.image(egui::load::SizedTexture::new(tex.id(), size));
                    }
                });
            });
        }

        close
    }
}
