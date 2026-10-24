//! L-Connect-style look: dark background, cards, cyan-violet accents.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Visuals,
};

pub const BG: Color32 = Color32::from_rgb(11, 13, 18);
pub const SIDEBAR: Color32 = Color32::from_rgb(16, 19, 26);
pub const CARD: Color32 = Color32::from_rgb(22, 26, 35);
pub const CARD_HI: Color32 = Color32::from_rgb(30, 35, 47);
pub const BORDER: Color32 = Color32::from_rgb(38, 44, 58);
pub const TEXT: Color32 = Color32::from_rgb(230, 233, 240);
pub const MUTED: Color32 = Color32::from_rgb(132, 141, 161);
pub const ACCENT: Color32 = Color32::from_rgb(0, 200, 255);
pub const ACCENT2: Color32 = Color32::from_rgb(168, 85, 247);
pub const WARN: Color32 = Color32::from_rgb(245, 165, 36);
pub const OK: Color32 = Color32::from_rgb(34, 197, 94);
pub const ERR: Color32 = Color32::from_rgb(239, 68, 68);

pub const RADIUS: u8 = 12;

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn apply(ctx: &egui::Context) {
    setup_fonts(ctx);

    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(RADIUS);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(140) };
    v.popup_shadow = v.window_shadow;
    v.extreme_bg_color = Color32::from_rgb(14, 17, 23);
    v.faint_bg_color = CARD_HI;
    v.warn_fg_color = WARN;
    v.override_text_color = None;
    v.selection.bg_fill = ACCENT.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = CARD;
    w.noninteractive.weak_bg_fill = CARD;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = CornerRadius::same(8);
    for (state, fill, stroke) in [
        (&mut w.inactive, CARD_HI, BORDER),
        (&mut w.hovered, Color32::from_rgb(38, 45, 60), ACCENT.linear_multiply(0.6)),
        (&mut w.active, Color32::from_rgb(44, 52, 70), ACCENT),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, stroke);
        state.fg_stroke = Stroke::new(1.0, TEXT);
        state.corner_radius = CornerRadius::same(8);
        state.expansion = 0.0;
    }
    w.open = w.active;
    // Always dark, regardless of the Windows light/dark theme.
    ctx.options_mut(|o| o.theme_preference = egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);

    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(10.0, 10.0);
        s.spacing.button_padding = egui::vec2(14.0, 7.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.window_margin = Margin::same(16);
        s.text_styles = [
            (TextStyle::Heading, semibold(20.0)),
            (TextStyle::Body, FontId::proportional(14.5)),
            (TextStyle::Button, FontId::proportional(14.5)),
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
    });
}

/// Segoe UI (+ Semibold) from Windows; emoji and symbols still come from the default egui fonts.
/// The Lucide icon font sits right after Segoe: it only has icon glyphs (private use area), so normal
/// text still uses Segoe's metrics, while icons win over egui's own emoji icon font, which uses the
/// same code points.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "lucide".into(),
        Arc::new(egui::FontData::from_static(include_bytes!("../assets/lucide/lucide.ttf"))),
    );
    let read = |f: &str| std::fs::read(format!("C:\\Windows\\Fonts\\{f}")).ok();
    let mut regular = vec![];
    if let Some(b) = read("segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), Arc::new(egui::FontData::from_owned(b)));
        regular.push("segoe".to_string());
    }
    regular.push("lucide".to_string());
    if let Some(f) = fonts.families.get_mut(&FontFamily::Proportional) {
        f.splice(0..0, regular.iter().cloned());
    }
    let mut semi = vec![];
    if let Some(b) = read("seguisb.ttf") {
        fonts.font_data.insert("segoe-semibold".into(), Arc::new(egui::FontData::from_owned(b)));
        semi.push("segoe-semibold".to_string());
    }
    semi.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(FontFamily::Name("semibold".into()), semi);
    ctx.set_fonts(fonts);
}
