// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! Product discovery — kova scans for binaries that speak the plugin protocol,
//! builds dynamic GUI tabs from their self-described capabilities.
//! Any binary that responds to `{"cmd":"capabilities"}` gets a tab.

use eframe::egui;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::surface::gui::theme::{colors, layout};

/// Known product binary names to scan for.
const PRODUCT_NAMES: &[&str] = &[
    "pixel-forge",
    // Future products just add a name here. Or scan PATH.
];

/// Directories to search for product binaries.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("bin"));
        dirs.push(home.join(".cargo/bin"));
        dirs.push(home.join("target/release"));
        dirs.push(home.join("target/debug"));
    }
    // Also check PATH entries
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let p = PathBuf::from(dir);
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }
    dirs
}

/// Find a binary by name across search dirs.
fn find_binary(name: &str) -> Option<PathBuf> {
    for dir in search_dirs() {
        let candidate = dir.join(name);
        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Send a plugin command to a binary, get JSON response.
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
        let err = resp.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
        Err(err.to_string())
    }
}

// ── UI Descriptor (from capabilities response) ──

#[derive(Clone)]
struct SelectorDef {
    id: String,
    label: String,
    values: Vec<String>,
    default_idx: usize,
}

#[derive(Clone)]
struct SliderDef {
    id: String,
    label: String,
    min: u32,
    max: u32,
    default: u32,
}

/// A discovered product with its capabilities.
struct DiscoveredProduct {
    name: String,
    version: String,
    description: String,
    binary: PathBuf,
    device_info: String,

    // UI schema from capabilities
    selectors: Vec<SelectorDef>,
    sliders: Vec<SliderDef>,
    action_cmd: String,
    output_type: String, // "sprites", etc.

    // Runtime state
    selector_state: Vec<usize>,   // selected index per selector
    slider_state: Vec<u32>,       // current value per slider
    generating: Arc<Mutex<bool>>,
    gen_result: Arc<Mutex<Option<GenResult>>>,
    batches: Vec<Batch>,
    status: String,
    total_generated: u32,
}

struct GenResult {
    sprites: Vec<SpriteData>,
    elapsed_ms: u64,
    label: String,
    status: String,
}

struct SpriteData {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

struct Batch {
    label: String,
    elapsed_ms: u64,
    textures: Vec<egui::TextureHandle>,
}

impl DiscoveredProduct {
    fn from_binary(binary: PathBuf) -> Option<Self> {
        // Query capabilities
        let caps = plugin_call(&binary, "capabilities", None).ok()?;

        let name = caps.get("name")?.as_str()?.to_string();
        let version = caps.get("version").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        let description = caps.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let ui = caps.get("ui")?;
        let action_cmd = ui.get("action").and_then(|v| v.as_str()).unwrap_or("generate").to_string();
        let output_type = ui.get("output").and_then(|v| v.as_str()).unwrap_or("sprites").to_string();

        let selectors: Vec<SelectorDef> = ui.get("selectors")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| {
                let id = s.get("id")?.as_str()?.to_string();
                let label = s.get("label").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
                let values: Vec<String> = s.get("values")?.as_array()?
                    .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                let default = s.get("default").and_then(|v| v.as_str()).unwrap_or("");
                let default_idx = values.iter().position(|v| v == default).unwrap_or(0);
                Some(SelectorDef { id, label, values, default_idx })
            }).collect())
            .unwrap_or_default();

        let sliders: Vec<SliderDef> = ui.get("sliders")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| {
                let id = s.get("id")?.as_str()?.to_string();
                let label = s.get("label").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
                let min = s.get("min").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let max = s.get("max").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
                let default = s.get("default").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                Some(SliderDef { id, label, min, max, default })
            }).collect())
            .unwrap_or_default();

        let selector_state: Vec<usize> = selectors.iter().map(|s| s.default_idx).collect();
        let slider_state: Vec<u32> = sliders.iter().map(|s| s.default).collect();

        // Probe device info
        let device_info = plugin_call(&binary, "probe", None).ok()
            .map(|data| {
                let backend = data.get("backend").and_then(|v| v.as_str()).unwrap_or("?");
                let ram = data.get("ram_mb").and_then(|v| v.as_u64()).unwrap_or(0);
                let tier = data.get("tier").and_then(|v| v.as_str()).unwrap_or("?");
                format!("{} | {}MB | {}", backend, ram, tier)
            })
            .unwrap_or_default();

        Some(Self {
            name,
            version,
            description,
            binary,
            device_info,
            selectors,
            sliders,
            action_cmd,
            output_type,
            selector_state,
            slider_state,
            generating: Arc::new(Mutex::new(false)),
            gen_result: Arc::new(Mutex::new(None)),
            batches: Vec::new(),
            status: String::new(),
            total_generated: 0,
        })
    }

    fn start_action(&mut self, ctx: &egui::Context) {
        let bin = self.binary.clone();
        let cmd = self.action_cmd.clone();

        // Build args from current selector/slider state
        let mut args = serde_json::Map::new();
        for (i, sel) in self.selectors.iter().enumerate() {
            let idx = self.selector_state[i];
            if idx < sel.values.len() {
                args.insert(sel.id.clone(), serde_json::Value::String(sel.values[idx].clone()));
            }
        }
        for (i, sl) in self.sliders.iter().enumerate() {
            args.insert(sl.id.clone(), serde_json::json!(self.slider_state[i]));
        }

        let label = self.selectors.iter().enumerate()
            .map(|(i, sel)| {
                let idx = self.selector_state[i];
                sel.values.get(idx).cloned().unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" / ");

        let generating = Arc::clone(&self.generating);
        let gen_result = Arc::clone(&self.gen_result);
        let ctx = ctx.clone();

        *generating.lock().unwrap() = true;
        self.status = format!("forging {}...", label);

        std::thread::spawn(move || {
            let t0 = Instant::now();
            let result = match plugin_call(&bin, &cmd, Some(serde_json::Value::Object(args))) {
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

                    let elapsed = t0.elapsed().as_millis() as u64;
                    let n = sprites.len();
                    GenResult {
                        sprites,
                        elapsed_ms: elapsed,
                        label: label.clone(),
                        status: format!("{} sprites in {:.1}s", n, elapsed as f64 / 1000.0),
                    }
                }
                Err(e) => GenResult {
                    sprites: Vec::new(),
                    elapsed_ms: t0.elapsed().as_millis() as u64,
                    label,
                    status: format!("error: {e}"),
                },
            };

            *gen_result.lock().unwrap() = Some(result);
            *generating.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }
}

// ── Product Hub (manages all discovered products) ──

/// T221 = ProductHub. Manages discovered product tabs.
pub struct T221 {
    products: Vec<DiscoveredProduct>,
    active_tab: usize,
    scan_done: bool,
}

impl T221 {
    /// Scan for products on disk.
    pub fn new() -> Self {
        let mut products = Vec::new();

        for name in PRODUCT_NAMES {
            if let Some(binary) = find_binary(name) {
                if let Some(product) = DiscoveredProduct::from_binary(binary) {
                    products.push(product);
                }
            }
        }

        // Also scan ~/bin for anything that responds to plugin protocol
        if let Some(home) = dirs::home_dir() {
            let bin_dir = home.join("bin");
            if bin_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if !path.is_file() { continue; }
                        let name = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("");
                        // Skip known system binaries and already-discovered
                        if PRODUCT_NAMES.contains(&name) { continue; }
                        if name.starts_with('.') || name == "kova" { continue; }

                        // Quick check: does it speak plugin?
                        if let Some(product) = DiscoveredProduct::from_binary(path) {
                            products.push(product);
                        }
                    }
                }
            }
        }

        Self {
            products,
            active_tab: 0,
            scan_done: true,
        }
    }

    pub fn product_count(&self) -> usize {
        self.products.len()
    }

    /// Render the product hub. Returns true if user wants to close.
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
        let mut close = false;

        if self.products.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Products")
                        .color(colors::PRIMARY)
                        .size(20.0)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() { close = true; }
                });
            });
            ui.add_space(layout::GAP);
            ui.label(egui::RichText::new("no products found").color(colors::MUTED));
            ui.label(egui::RichText::new("products must have a `plugin` subcommand that speaks JSON").color(colors::MUTED).small());
            return close;
        }

        // ── Tab bar ──
        ui.horizontal(|ui| {
            for (i, product) in self.products.iter().enumerate() {
                let active = self.active_tab == i;
                let color = if active { colors::PRIMARY } else { colors::MUTED };
                let label = egui::RichText::new(&product.name)
                    .color(color)
                    .size(if active { 18.0 } else { 14.0 })
                    .strong();
                if ui.add(egui::Button::new(label)
                    .min_size(egui::vec2(80.0, 28.0))
                    .selected(active)
                ).clicked() {
                    self.active_tab = i;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() { close = true; }
            });
        });

        ui.separator();

        // ── Active product panel ──
        if self.active_tab < self.products.len() {
            let product = &mut self.products[self.active_tab];
            show_product(product, ui, ctx);
        }

        close
    }
}

/// Render a single product tab — fully dynamic from capabilities.
fn show_product(product: &mut DiscoveredProduct, ui: &mut egui::Ui, ctx: &egui::Context) {
    // Header
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("v{}", product.version)).color(colors::MUTED).small());
        if !product.description.is_empty() {
            ui.label(egui::RichText::new(&product.description).color(colors::MUTED).small());
        }
        if !product.device_info.is_empty() {
            ui.add_space(layout::MARGIN);
            ui.label(egui::RichText::new(&product.device_info).color(colors::TERTIARY).small());
        }
    });

    ui.add_space(layout::PADDING_SM);

    // Dynamic selectors
    for (i, sel) in product.selectors.clone().iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&sel.label).color(colors::MUTED));
            ui.add_space(4.0);
            for (j, val) in sel.values.iter().enumerate() {
                let selected = product.selector_state[i] == j;
                let color = if selected { colors::PRIMARY } else { colors::TEXT };
                let btn = egui::Button::new(egui::RichText::new(val).color(color).small())
                    .min_size(egui::vec2(44.0, 20.0))
                    .selected(selected);
                if ui.add(btn).clicked() {
                    product.selector_state[i] = j;
                }
            }
        });
    }

    ui.add_space(4.0);

    // Dynamic sliders
    ui.horizontal(|ui| {
        for (i, sl) in product.sliders.clone().iter().enumerate() {
            ui.label(egui::RichText::new(&sl.label).color(colors::MUTED));
            ui.add(egui::DragValue::new(&mut product.slider_state[i])
                .range(sl.min..=sl.max)
                .speed(if sl.max > 20 { 1.0 } else { 0.2 }));
            ui.add_space(8.0);
        }
    });

    ui.add_space(layout::GAP);

    // Action button
    let is_generating = *product.generating.lock().unwrap();
    let can_gen = !is_generating;

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
            product.start_action(ctx);
        }
        ui.add_space(layout::GAP);
        if !product.status.is_empty() {
            ui.label(egui::RichText::new(&product.status).color(colors::MUTED));
        }
        if product.total_generated > 0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("{} total", product.total_generated)).color(colors::MUTED).small());
            });
        }
    });

    // Collect results
    {
        let mut result_lock = product.gen_result.lock().unwrap();
        if let Some(result) = result_lock.take() {
            product.status = result.status;
            let n = result.sprites.len() as u32;
            product.total_generated += n;

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
                    format!("product_{}_{}", product.batches.len(), textures.len()),
                    color_image,
                    opts,
                ));
            }

            product.batches.push(Batch {
                label: result.label,
                elapsed_ms: result.elapsed_ms,
                textures,
            });
        }
    }

    // Gallery
    if !product.batches.is_empty() {
        ui.add_space(layout::GAP);
        ui.separator();
        ui.add_space(layout::PADDING_SM);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for batch in product.batches.iter().rev() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&batch.label).color(colors::PRIMARY).strong());
                    ui.label(egui::RichText::new(format!(
                        "{} in {:.1}s", batch.textures.len(), batch.elapsed_ms as f64 / 1000.0
                    )).color(colors::MUTED).small());
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let scale = 5.0;
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
                ui.add_space(layout::GAP);
            }
        });
    }
}
