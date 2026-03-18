// Unlicense — cochranblock.org
//! Kova web theme. THEME.md palette applied to egui WASM canvas.
//! Neon electric blue, teal, purple/magenta. Dark cosmic background.

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle, Visuals};

pub mod colors {
    use eframe::egui::Color32;

    pub const BG: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0f);
    pub const SURFACE: Color32 = Color32::from_rgb(0x14, 0x14, 0x1f);
    pub const SURFACE_ELEVATED: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x28);
    pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0x1a, 0x2a, 0x35);
    pub const PRIMARY: Color32 = Color32::from_rgb(0x00, 0xd4, 0xff);
    pub const SECONDARY: Color32 = Color32::from_rgb(0xa8, 0x55, 0xf7);
    pub const TERTIARY: Color32 = Color32::from_rgb(0x14, 0xb8, 0xa6);
    pub const TEXT: Color32 = Color32::from_rgb(0xe2, 0xe8, 0xf0);
    pub const MUTED: Color32 = Color32::from_rgb(0x64, 0x74, 0x8b);
    #[allow(dead_code)] // Available for text input backgrounds
    pub const INPUT_BG: Color32 = Color32::from_rgb(0x10, 0x10, 0x1a);
    pub const BORDER: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x3a);
}

pub const RADIUS: u8 = 8;
pub const RADIUS_SM: u8 = 4;
pub const GAP: f32 = 8.0;
pub const PADDING_SM: f32 = 6.0;
pub const PADDING_MD: f32 = 12.0;

/// f221=apply
pub fn f221(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals();
    style.spacing = spacing();
    style.text_styles = text_styles();
    ctx.set_style(style);
}

fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.window_fill = colors::BG;
    v.panel_fill = colors::BG;
    v.override_text_color = Some(colors::TEXT);
    v.faint_bg_color = colors::SURFACE;
    v.extreme_bg_color = colors::BG;
    v.code_bg_color = colors::SURFACE_ELEVATED;
    v.hyperlink_color = colors::PRIMARY;
    v.warn_fg_color = colors::TERTIARY;
    v.error_fg_color = colors::SECONDARY;
    v.window_corner_radius = CornerRadius::same(RADIUS);
    v.window_stroke = Stroke::new(1.0, colors::BORDER);
    v.widgets.noninteractive.bg_fill = colors::SURFACE;
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v.widgets.noninteractive.corner_radius = CornerRadius::same(RADIUS_SM);
    v.widgets.inactive.bg_fill = colors::SURFACE_ELEVATED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, colors::BORDER);
    v.widgets.inactive.corner_radius = CornerRadius::same(RADIUS_SM);
    v.widgets.hovered.bg_fill = colors::SURFACE_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, colors::PRIMARY);
    v.widgets.hovered.corner_radius = CornerRadius::same(RADIUS_SM);
    v.widgets.active.bg_fill = colors::PRIMARY;
    v.widgets.active.bg_stroke = Stroke::new(1.0, colors::PRIMARY);
    v.widgets.active.corner_radius = CornerRadius::same(RADIUS_SM);
    v.selection.bg_fill = Color32::from_rgba_premultiplied(0x00, 0xd4, 0xff, 40);
    v.selection.stroke = Stroke::new(1.0, colors::PRIMARY);
    v.collapsing_header_frame = true;
    v
}

fn spacing() -> egui::style::Spacing {
    egui::style::Spacing {
        item_spacing: egui::vec2(GAP, GAP),
        button_padding: egui::vec2(PADDING_MD, PADDING_SM),
        window_margin: egui::Margin::same(16),
        ..Default::default()
    }
}

fn text_styles() -> std::collections::BTreeMap<TextStyle, FontId> {
    let mut map = std::collections::BTreeMap::new();
    map.insert(TextStyle::Small, FontId::proportional(12.0));
    map.insert(TextStyle::Body, FontId::proportional(14.0));
    map.insert(TextStyle::Monospace, FontId::monospace(13.0));
    map.insert(TextStyle::Button, FontId::proportional(14.0));
    map.insert(TextStyle::Heading, FontId::proportional(22.0));
    map
}

/// f222=header_frame
pub fn f222() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::SURFACE)
        .inner_margin(egui::Margin::symmetric(16, 10))
        .stroke(Stroke::new(1.0, colors::BORDER))
}

/// f223=message_frame
pub fn f223() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::SURFACE)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(egui::Margin::same(12))
        .stroke(Stroke::new(1.0, colors::BORDER))
}

/// f224=user_message_frame
pub fn f224() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::SURFACE_ELEVATED)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(egui::Margin::same(12))
        .stroke(Stroke::new(1.0, colors::PRIMARY))
}

/// f225=code_frame
pub fn f225() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::BG)
        .corner_radius(CornerRadius::same(RADIUS_SM))
        .inner_margin(egui::Margin::same(10))
        .stroke(Stroke::new(1.0, colors::BORDER))
}

/// f226=input_frame
pub fn f226() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::SURFACE)
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(egui::Margin::same(12))
        .stroke(Stroke::new(1.0, colors::BORDER))
}

/// f227=sidebar_frame
pub fn f227() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::SURFACE)
        .inner_margin(egui::Margin::same(12))
        .stroke(Stroke::new(1.0, colors::BORDER))
}
