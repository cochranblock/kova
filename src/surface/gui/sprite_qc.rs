// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! Sprite QC — tinder-style swipe UI for pixel art quality control.
//! Approve/reject generated sprites. Keyboard: A/Left=reject, D/Right=approve, S/Down=skip.

use eframe::egui;
use std::path::{Path, PathBuf};

use crate::theme::{colors, layout};

/// QC verdict per sprite.
#[derive(Clone, Copy, PartialEq)]
pub enum Verdict {
    Approve,
    Reject,
    Skip,
}

/// One sprite in the queue.
struct SpriteEntry {
    path: PathBuf,
    /// Display label: "zone 03 / bg" or "player / run"
    label: String,
    verdict: Option<Verdict>,
}

/// State for the sprite QC panel.
pub struct SpriteQc {
    sprites: Vec<SpriteEntry>,
    current: usize,
    /// Loaded texture for current sprite.
    texture: Option<egui::TextureHandle>,
    /// Path of the texture currently loaded (to detect changes).
    loaded_path: Option<PathBuf>,
    /// Root dir we scanned.
    root: PathBuf,
    /// Animation: swipe direction for visual feedback.
    swipe_anim: Option<(f32, Verdict)>,
}

impl SpriteQc {
    /// Scan a directory tree for PNGs and build the queue.
    pub fn scan(root: &Path) -> Self {
        let mut sprites = Vec::new();
        collect_pngs(root, root, &mut sprites);
        sprites.sort_by(|a, b| a.path.cmp(&b.path));
        Self {
            sprites,
            current: 0,
            texture: None,
            loaded_path: None,
            root: root.to_path_buf(),
            swipe_anim: None,
        }
    }

    pub fn total(&self) -> usize {
        self.sprites.len()
    }

    pub fn approved(&self) -> usize {
        self.sprites
            .iter()
            .filter(|s| s.verdict == Some(Verdict::Approve))
            .count()
    }

    pub fn rejected(&self) -> usize {
        self.sprites
            .iter()
            .filter(|s| s.verdict == Some(Verdict::Reject))
            .count()
    }

    pub fn remaining(&self) -> usize {
        self.sprites.iter().filter(|s| s.verdict.is_none()).count()
    }

    pub fn is_done(&self) -> bool {
        self.current >= self.sprites.len()
    }

    /// Apply verdict and advance.
    fn decide(&mut self, verdict: Verdict) {
        if self.current < self.sprites.len() {
            self.sprites[self.current].verdict = Some(verdict);
            self.swipe_anim = Some((1.0, verdict));
            self.current += 1;
            self.texture = None;
            self.loaded_path = None;
        }
    }

    /// Move rejected files to rejected/ subdir, approved to approved/.
    pub fn apply_verdicts(&self) -> (usize, usize) {
        let approved_dir = self.root.join("approved");
        let rejected_dir = self.root.join("rejected");
        let _ = std::fs::create_dir_all(&approved_dir);
        let _ = std::fs::create_dir_all(&rejected_dir);

        let mut approved = 0;
        let mut rejected = 0;

        for sprite in &self.sprites {
            let rel = sprite
                .path
                .strip_prefix(&self.root)
                .unwrap_or(&sprite.path);
            match sprite.verdict {
                Some(Verdict::Approve) => {
                    let dest = approved_dir.join(rel);
                    if let Some(p) = dest.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let _ = std::fs::copy(&sprite.path, &dest);
                    approved += 1;
                }
                Some(Verdict::Reject) => {
                    let dest = rejected_dir.join(rel);
                    if let Some(p) = dest.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let _ = std::fs::copy(&sprite.path, &dest);
                    rejected += 1;
                }
                _ => {}
            }
        }
        (approved, rejected)
    }

    /// Render the QC UI. Returns true if user wants to close.
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
        let mut close = false;

        // Header
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Sprite QC")
                    .color(colors::PRIMARY)
                    .size(20.0)
                    .strong(),
            );
            ui.add_space(layout::MARGIN);
            ui.label(
                egui::RichText::new(format!(
                    "{} approved  {}  rejected  {}  remaining",
                    self.approved(),
                    self.rejected(),
                    self.remaining()
                ))
                .color(colors::MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
        ui.add_space(layout::GAP);

        // Progress bar
        let progress = if self.sprites.is_empty() {
            1.0
        } else {
            self.current as f32 / self.sprites.len() as f32
        };
        let bar = egui::ProgressBar::new(progress)
            .text(format!("{}/{}", self.current, self.sprites.len()));
        ui.add(bar);
        ui.add_space(layout::GAP);

        if self.is_done() {
            // Summary
            crate::theme::panel_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("QC Complete")
                        .color(colors::TERTIARY)
                        .size(18.0)
                        .strong(),
                );
                ui.add_space(layout::GAP);
                ui.label(
                    egui::RichText::new(format!(
                        "Approved: {}  |  Rejected: {}  |  Skipped: {}",
                        self.approved(),
                        self.rejected(),
                        self.sprites
                            .iter()
                            .filter(|s| s.verdict == Some(Verdict::Skip))
                            .count()
                    ))
                    .color(colors::TEXT)
                    .size(16.0),
                );
                ui.add_space(layout::GAP);
                if ui.button("Save results (copy to approved/rejected dirs)").clicked() {
                    let (a, r) = self.apply_verdicts();
                    ui.label(
                        egui::RichText::new(format!("Saved: {} approved, {} rejected", a, r))
                            .color(colors::TERTIARY),
                    );
                }
                if ui.button("Start over").clicked() {
                    self.current = 0;
                    for s in &mut self.sprites {
                        s.verdict = None;
                    }
                    self.texture = None;
                    self.loaded_path = None;
                }
            });
            return close;
        }

        // Current sprite
        let entry = &self.sprites[self.current];

        // Load texture if needed
        if self.loaded_path.as_ref() != Some(&entry.path) {
            if let Ok(img_data) = std::fs::read(&entry.path) {
                if let Ok(img) = image::load_from_memory(&img_data) {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let pixels = rgba.into_raw();
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                    let tex = ctx.load_texture(
                        entry.path.to_string_lossy(),
                        color_image,
                        egui::TextureOptions::NEAREST, // pixel art — no filtering
                    );
                    self.texture = Some(tex);
                    self.loaded_path = Some(entry.path.clone());
                }
            }
        }

        // Label
        ui.label(
            egui::RichText::new(&entry.label)
                .color(colors::TEXT)
                .size(16.0)
                .strong(),
        );
        ui.label(
            egui::RichText::new(entry.path.to_string_lossy())
                .color(colors::MUTED)
                .small(),
        );
        ui.add_space(layout::GAP);

        // Image display — scaled up for pixel art visibility
        if let Some(tex) = &self.texture {
            let tex_size = tex.size_vec2();
            // Scale up: pixel art is small, show at 4x or fill available width
            let available = ui.available_width().min(512.0);
            let scale = (available / tex_size.x).max(1.0).min(8.0);
            let display_size = egui::vec2(tex_size.x * scale, tex_size.y * scale);

            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - display_size.x) / 2.0);
                let img = egui::Image::new(tex)
                    .fit_to_exact_size(display_size);
                ui.add(img);
            });
        } else {
            ui.label(
                egui::RichText::new("(failed to load)")
                    .color(colors::SECONDARY),
            );
        }

        ui.add_space(layout::GAP);

        // Swipe buttons
        ui.horizontal(|ui| {
            let btn_size = egui::vec2(120.0, 48.0);

            // Reject button (red-ish)
            let reject_btn = egui::Button::new(
                egui::RichText::new("Reject (A)")
                    .color(egui::Color32::WHITE)
                    .size(16.0),
            )
            .fill(egui::Color32::from_rgb(0xdc, 0x26, 0x26))
            .min_size(btn_size);
            if ui.add(reject_btn).clicked() {
                self.decide(Verdict::Reject);
            }

            ui.add_space(layout::MARGIN);

            // Skip button
            let skip_btn = egui::Button::new(
                egui::RichText::new("Skip (S)")
                    .color(colors::TEXT)
                    .size(16.0),
            )
            .fill(colors::SURFACE_ELEVATED)
            .min_size(btn_size);
            if ui.add(skip_btn).clicked() {
                self.decide(Verdict::Skip);
            }

            ui.add_space(layout::MARGIN);

            // Approve button (green)
            let approve_btn = egui::Button::new(
                egui::RichText::new("Approve (D)")
                    .color(egui::Color32::WHITE)
                    .size(16.0),
            )
            .fill(egui::Color32::from_rgb(0x16, 0xa3, 0x4a))
            .min_size(btn_size);
            if ui.add(approve_btn).clicked() {
                self.decide(Verdict::Approve);
            }
        });

        // Keyboard input
        ui.input(|i| {
            if i.key_pressed(egui::Key::A) || i.key_pressed(egui::Key::ArrowLeft) {
                // handled below
            }
        });
        let keys = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::A) || i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::D) || i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::S) || i.key_pressed(egui::Key::ArrowDown),
            )
        });
        if keys.0 {
            self.decide(Verdict::Reject);
        }
        if keys.1 {
            self.decide(Verdict::Approve);
        }
        if keys.2 {
            self.decide(Verdict::Skip);
        }

        // Hint
        ui.add_space(layout::GAP);
        ui.label(
            egui::RichText::new("A/Left = Reject  |  D/Right = Approve  |  S/Down = Skip")
                .color(colors::MUTED)
                .small(),
        );

        close
    }
}

/// Recursively collect PNGs, building labels from relative path.
fn collect_pngs(root: &Path, dir: &Path, out: &mut Vec<SpriteEntry>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip approved/rejected dirs
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "approved" || name == "rejected" {
                continue;
            }
            collect_pngs(root, &path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("png") {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let label = rel
                .to_string_lossy()
                .replace('/', " / ")
                .replace(".png", "");
            out.push(SpriteEntry {
                path,
                label,
                verdict: None,
            });
        }
    }
}
