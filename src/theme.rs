//! Kova theme. THEME.md palette + professional layout.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use eframe::egui::{self, CornerRadius, FontId, Stroke, TextStyle, Visuals};

/// Theme colors from THEME.md.
pub mod colors {
    use eframe::egui::Color32;

    pub const BG: Color32 = Color32::from_rgb(0x05, 0x05, 0x08);
    pub const SURFACE: Color32 = Color32::from_rgb(0x0d, 0x0d, 0x14);
    pub const SURFACE_ELEVATED: Color32 = Color32::from_rgb(0x14, 0x14, 0x1f);
    pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0x1a, 0x2a, 0x35);
    pub const PRIMARY: Color32 = Color32::from_rgb(0x00, 0xd9, 0xff);
    pub const SECONDARY: Color32 = Color32::from_rgb(0x9d, 0x4e, 0xdd);
    pub const TERTIARY: Color32 = Color32::from_rgb(0x00, 0xff, 0xcc);
    pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
    pub const MUTED: Color32 = Color32::from_rgb(0x9c, 0xa3, 0xaf);
}

/// Layout constants.
pub mod layout {
    pub const MARGIN: f32 = 16.0;
    pub const MARGIN_I8: i8 = 16;
    pub const PADDING_SM: f32 = 6.0;
    pub const PADDING_MD: f32 = 12.0;
    pub const PADDING_MD_I8: i8 = 12;
    pub const PADDING_LG: f32 = 16.0;
    pub const GAP: f32 = 8.0;
    pub const RADIUS: f32 = 8.0;
    pub const RADIUS_U8: u8 = 8;
    pub const RADIUS_SM: f32 = 4.0;
    pub const RADIUS_SM_U8: u8 = 4;
}

/// Apply full theme to context.
/// f320=apply
pub fn f320(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals();
    style.spacing = spacing();
    style.text_styles = text_styles();
    ctx.set_style(style);
}

fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.window_fill = colors::BG;
    v.panel_fill = colors::SURFACE;
    v.override_text_color = Some(colors::TEXT);
    v.faint_bg_color = colors::SURFACE;
    v.extreme_bg_color = colors::BG;
    v.code_bg_color = colors::SURFACE_ELEVATED;
    v.hyperlink_color = colors::PRIMARY;
    v.warn_fg_color = colors::TERTIARY;
    v.error_fg_color = colors::SECONDARY;
    v.window_corner_radius = CornerRadius::same(layout::RADIUS_U8);
    v.window_stroke = Stroke::new(1.0, colors::SURFACE_ELEVATED);
    v.widgets.noninteractive.bg_fill = colors::SURFACE;
    v.widgets.noninteractive.corner_radius = CornerRadius::same(layout::RADIUS_SM_U8);
    v.widgets.inactive.bg_fill = colors::SURFACE_ELEVATED;
    v.widgets.inactive.corner_radius = CornerRadius::same(layout::RADIUS_SM_U8);
    v.widgets.hovered.bg_fill = colors::SURFACE_HOVER;
    v.widgets.hovered.corner_radius = CornerRadius::same(layout::RADIUS_SM_U8);
    v.widgets.active.bg_fill = colors::PRIMARY;
    v.widgets.active.bg_stroke = Stroke::new(1.0, colors::PRIMARY);
    v.widgets.active.corner_radius = CornerRadius::same(layout::RADIUS_SM_U8);
    v.selection.bg_fill = colors::PRIMARY;
    v.selection.stroke = Stroke::new(1.0, colors::PRIMARY);
    v.collapsing_header_frame = true;
    v
}

fn spacing() -> egui::style::Spacing {
    egui::style::Spacing {
        item_spacing: egui::vec2(layout::GAP, 12.0),
        button_padding: egui::vec2(16.0, 12.0),
        interact_size: egui::vec2(48.0, 48.0),
        window_margin: egui::Margin::same(layout::MARGIN_I8),
        ..Default::default()
    }
}

fn text_styles() -> std::collections::BTreeMap<TextStyle, FontId> {
    let mut map = std::collections::BTreeMap::new();
    map.insert(TextStyle::Small, FontId::proportional(12.0));
    map.insert(TextStyle::Body, FontId::proportional(14.0));
    map.insert(TextStyle::Monospace, FontId::monospace(13.0));
    map.insert(TextStyle::Button, FontId::proportional(14.0));
    map.insert(TextStyle::Heading, FontId::proportional(20.0));
    map
}

/// Styled frame for message cards.
/// f321=message_frame
pub fn f321() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::SURFACE)
        .corner_radius(CornerRadius::same(layout::RADIUS_U8))
        .inner_margin(egui::Margin::same(layout::PADDING_MD_I8))
        .stroke(Stroke::new(1.0, colors::SURFACE_ELEVATED))
}

/// Styled frame for input area.
/// f322=input_frame
pub fn f322() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::SURFACE)
        .corner_radius(CornerRadius::same(layout::RADIUS_U8))
        .inner_margin(egui::Margin::same(layout::PADDING_MD_I8))
        .stroke(Stroke::new(1.0, colors::SURFACE_ELEVATED))
}

/// Styled frame for panels (backlog, prompts).
/// f323=panel_frame
pub fn f323() -> egui::Frame {
    egui::Frame::default()
        .fill(colors::SURFACE)
        .corner_radius(CornerRadius::same(layout::RADIUS_U8))
        .inner_margin(egui::Margin::same(layout::PADDING_MD_I8))
        .stroke(Stroke::new(1.0, colors::SURFACE_ELEVATED))
}