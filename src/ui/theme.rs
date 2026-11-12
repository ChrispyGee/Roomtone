//! Colours, type and spacing for the modern dark look.

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(13, 14, 18);
pub const SIDEBAR: Color32 = Color32::from_rgb(18, 19, 24);
pub const SURFACE: Color32 = Color32::from_rgb(26, 28, 35);
pub const SURFACE_HI: Color32 = Color32::from_rgb(36, 39, 49);
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(44, 47, 59);
pub const LINE: Color32 = Color32::from_rgb(44, 47, 58);
pub const TEXT: Color32 = Color32::from_rgb(242, 243, 246);
pub const TEXT_DIM: Color32 = Color32::from_rgb(154, 160, 176);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(98, 104, 120);
pub const ACCENT: Color32 = Color32::from_rgb(255, 140, 82);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(64, 40, 30);
pub const GOOD: Color32 = Color32::from_rgb(88, 214, 141);
pub const BAD: Color32 = Color32::from_rgb(255, 99, 99);
pub const S1_TINT: Color32 = Color32::from_rgb(120, 170, 255);
pub const S2_TINT: Color32 = Color32::from_rgb(255, 140, 82);

pub fn system_tint(s: crate::model::System) -> Color32 {
    match s {
        crate::model::System::S1 => S1_TINT,
        crate::model::System::S2 => S2_TINT,
    }
}

pub fn apply(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Prefer the Mac's own interface font when it can be read; fall back to the built-in one.
    for (name, path) in [("system", "/System/Library/Fonts/SFNS.ttf"), ("system", "/System/Library/Fonts/Helvetica.ttc"), ("system", "C:\\Windows\\Fonts\\segoeui.ttf")] {
        if fonts.font_data.contains_key(name) {
            break;
        }
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            fonts.families.entry(FontFamily::Proportional).or_default().insert(0, name.into());
        }
    }
    ctx.set_fonts(fonts);

    // The design is dark whatever the computer is set to.
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.global_style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(26.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.scroll = egui::style::ScrollStyle::floating();

    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = Color32::from_rgb(20, 21, 27);
    v.faint_bg_color = SURFACE;
    v.window_stroke = Stroke::new(1.0_f32, LINE);
    v.window_corner_radius = 16.0.into();
    v.menu_corner_radius = 12.0.into();
    v.window_shadow = egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(140) };
    v.popup_shadow = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(120) };
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0_f32, TEXT);
    v.hyperlink_color = ACCENT;
    v.override_text_color = None;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = 10.0.into();
        w.bg_stroke = Stroke::NONE;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_DIM);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.inactive.bg_fill = SURFACE_HI;
    v.widgets.inactive.weak_bg_fill = SURFACE_HI;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.hovered.bg_fill = SURFACE_HOVER;
    v.widgets.hovered.weak_bg_fill = SURFACE_HOVER;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::BLACK);
    v.widgets.open.weak_bg_fill = SURFACE_HOVER;
    ctx.set_global_style(style);
}
