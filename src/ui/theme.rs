//! Colours, type and spacing, in a dark and a light version.
//!
//! The colours are functions (`TEXT()`, `SURFACE()`…) rather than constants, so the whole
//! window switches when the appearance changes. Which palette is in use is a process-wide
//! flag set once per frame from egui's effective theme (`sync`), so Automatic follows the
//! computer's light/dark setting as it changes.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};

static DARK: AtomicBool = AtomicBool::new(true);

/// Is the dark palette in use?
pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

pub fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

macro_rules! palette {
    ($($(#[$doc:meta])* $name:ident: $dark:expr, $light:expr;)*) => {
        $(
            $(#[$doc])*
            #[allow(non_snake_case)]
            #[inline]
            pub fn $name() -> Color32 {
                if is_dark() { $dark } else { $light }
            }
        )*
    };
}

palette! {
    /// The window background.
    BG: rgb(13, 14, 18), rgb(244, 244, 247);
    /// The sidebar and the bottom bar.
    SIDEBAR: rgb(18, 19, 24), rgb(234, 235, 240);
    /// Cards.
    SURFACE: rgb(26, 28, 35), rgb(255, 255, 255);
    /// Buttons and raised parts of cards.
    SURFACE_HI: rgb(36, 39, 49), rgb(230, 231, 237);
    SURFACE_HOVER: rgb(44, 47, 59), rgb(218, 220, 228);
    LINE: rgb(44, 47, 58), rgb(212, 214, 222);
    TEXT: rgb(242, 243, 246), rgb(22, 23, 28);
    TEXT_DIM: rgb(154, 160, 176), rgb(84, 89, 102);
    TEXT_FAINT: rgb(98, 104, 120), rgb(134, 139, 152);
    /// The orange of the brand; a little deeper on light backgrounds for contrast.
    ACCENT: rgb(255, 140, 82), rgb(230, 104, 42);
    ACCENT_SOFT: rgb(64, 40, 30), rgb(255, 230, 216);
    GOOD: rgb(88, 214, 141), rgb(26, 146, 80);
    BAD: rgb(255, 99, 99), rgb(204, 50, 50);
    S1_TINT: rgb(120, 170, 255), rgb(48, 104, 214);
    S2_TINT: rgb(255, 140, 82), rgb(230, 104, 42);
    /// Text fields and other sunken areas.
    SUNKEN: rgb(20, 21, 27), rgb(236, 237, 242);
}

/// A faint wash over the background (slider tracks, hover tints): white on dark, black on light.
pub fn overlay(alpha: u8) -> Color32 {
    if is_dark() {
        Color32::from_white_alpha(alpha)
    } else {
        Color32::from_black_alpha(alpha / 2 + alpha / 4)
    }
}

/// An icon drawn on a button filled with `TEXT()`.
#[allow(non_snake_case)]
pub fn ON_TEXT() -> Color32 {
    if is_dark() {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

pub fn system_tint(s: crate::model::System) -> Color32 {
    match s {
        crate::model::System::S1 => S1_TINT(),
        crate::model::System::S2 => S2_TINT(),
    }
}

/// The person's choice: "auto", "light" or "dark".
pub fn preference(code: &str) -> egui::ThemePreference {
    match code {
        "light" => egui::ThemePreference::Light,
        "dark" => egui::ThemePreference::Dark,
        _ => egui::ThemePreference::System,
    }
}

/// Fonts and both styles. Call once at start-up, then `set_appearance` and `sync`.
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

    let was = is_dark();
    for (theme, dark) in [(egui::Theme::Dark, true), (egui::Theme::Light, false)] {
        set_dark(dark);
        ctx.set_style_of(theme, style(dark));
    }
    set_dark(was);
}

/// Light, dark or follow the computer.
pub fn set_appearance(ctx: &egui::Context, code: &str) {
    ctx.set_theme(preference(code));
}

/// Pick the palette egui is showing this frame (follows the computer in Automatic).
pub fn sync(ctx: &egui::Context) {
    set_dark(ctx.theme() == egui::Theme::Dark);
}

/// The full style for one palette (the palette flag must already be set to match).
fn style(dark: bool) -> egui::Style {
    let mut style = egui::Style {
        text_styles: [
            (TextStyle::Heading, FontId::new(26.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into(),
        ..Default::default()
    };
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.scroll = egui::style::ScrollStyle::floating();

    let shadow = |a: u8| if dark { Color32::from_black_alpha(a) } else { Color32::from_black_alpha(a / 3) };
    let v = &mut style.visuals;
    *v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = BG();
    v.window_fill = SURFACE();
    v.extreme_bg_color = SUNKEN();
    v.faint_bg_color = SURFACE();
    v.window_stroke = Stroke::new(1.0_f32, LINE());
    v.window_corner_radius = 16.0.into();
    v.menu_corner_radius = 12.0.into();
    v.window_shadow = egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: shadow(140) };
    v.popup_shadow = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: shadow(120) };
    v.selection.bg_fill = ACCENT();
    v.selection.stroke = Stroke::new(1.0_f32, TEXT());
    v.hyperlink_color = ACCENT();
    v.override_text_color = None;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = 10.0.into();
        w.bg_stroke = Stroke::NONE;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_DIM());
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE());
    v.widgets.inactive.bg_fill = SURFACE_HI();
    v.widgets.inactive.weak_bg_fill = SURFACE_HI();
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT());
    v.widgets.hovered.bg_fill = SURFACE_HOVER();
    v.widgets.hovered.weak_bg_fill = SURFACE_HOVER();
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT());
    v.widgets.active.bg_fill = ACCENT();
    v.widgets.active.weak_bg_fill = ACCENT();
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::BLACK);
    v.widgets.open.weak_bg_fill = SURFACE_HOVER();
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_differ_and_text_contrasts_with_background() {
        let lum = |c: Color32| 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
        let was = is_dark();
        for dark in [true, false] {
            set_dark(dark);
            assert!((lum(TEXT()) - lum(BG())).abs() > 150.0, "text must stand out (dark={dark})");
            assert!((lum(TEXT_DIM()) - lum(SURFACE())).abs() > 70.0, "dim text must be readable (dark={dark})");
            assert_eq!(is_dark(), dark);
        }
        set_dark(true);
        let d = BG();
        set_dark(false);
        assert_ne!(d, BG());
        set_dark(was);
    }

    #[test]
    fn preference_codes() {
        assert_eq!(preference("light"), egui::ThemePreference::Light);
        assert_eq!(preference("dark"), egui::ThemePreference::Dark);
        assert_eq!(preference("auto"), egui::ThemePreference::System);
        assert_eq!(preference("anything else"), egui::ThemePreference::System);
    }
}
