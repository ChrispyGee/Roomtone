//! Colours, type and spacing, in a dark and a light version.
//!
//! The colours are functions (`TEXT()`, `SURFACE()`…) rather than constants, so the whole
//! window switches when the appearance changes. Which palette is in use is a process-wide
//! flag set once per frame from egui's effective theme (`sync`), so Automatic follows the
//! computer's light/dark setting as it changes.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};

static DARK: AtomicBool = AtomicBool::new(true);
static CONTRAST: AtomicBool = AtomicBool::new(false);

/// Is the *High contrast* palette in use (stronger text, lines and tints)?
pub fn high_contrast() -> bool {
    CONTRAST.load(Ordering::Relaxed)
}

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
    ($($(#[$doc:meta])* $name:ident: $dark:expr, $light:expr, $hc_dark:expr, $hc_light:expr;)*) => {
        $(
            $(#[$doc])*
            #[allow(non_snake_case)]
            #[inline]
            pub fn $name() -> Color32 {
                match (is_dark(), high_contrast()) {
                    (true, false) => $dark,
                    (false, false) => $light,
                    (true, true) => $hc_dark,
                    (false, true) => $hc_light,
                }
            }
        )*
    };
}

// Each colour has four values: dark, light, and a stronger dark and light for *High contrast*.
palette! {
    /// The window background.
    BG: rgb(13, 14, 18), rgb(244, 244, 247), rgb(0, 0, 0), rgb(255, 255, 255);
    /// The sidebar and the bottom bar.
    SIDEBAR: rgb(18, 19, 24), rgb(234, 235, 240), rgb(8, 8, 10), rgb(240, 240, 244);
    /// Cards.
    SURFACE: rgb(26, 28, 35), rgb(255, 255, 255), rgb(16, 16, 20), rgb(255, 255, 255);
    /// Buttons and raised parts of cards.
    SURFACE_HI: rgb(36, 39, 49), rgb(230, 231, 237), rgb(40, 42, 52), rgb(222, 224, 232);
    SURFACE_HOVER: rgb(44, 47, 59), rgb(218, 220, 228), rgb(62, 65, 80), rgb(200, 203, 214);
    LINE: rgb(44, 47, 58), rgb(212, 214, 222), rgb(160, 166, 184), rgb(90, 94, 108);
    TEXT: rgb(242, 243, 246), rgb(22, 23, 28), rgb(255, 255, 255), rgb(0, 0, 0);
    TEXT_DIM: rgb(154, 160, 176), rgb(84, 89, 102), rgb(214, 218, 230), rgb(40, 44, 56);
    TEXT_FAINT: rgb(98, 104, 120), rgb(134, 139, 152), rgb(176, 182, 198), rgb(84, 89, 102);
    /// The orange of the brand; a little deeper on light backgrounds for contrast.
    ACCENT: rgb(255, 140, 82), rgb(230, 104, 42), rgb(255, 170, 120), rgb(170, 60, 8);
    ACCENT_SOFT: rgb(64, 40, 30), rgb(255, 230, 216), rgb(90, 52, 36), rgb(255, 222, 204);
    GOOD: rgb(88, 214, 141), rgb(26, 146, 80), rgb(110, 235, 160), rgb(10, 100, 46);
    BAD: rgb(255, 99, 99), rgb(204, 50, 50), rgb(255, 130, 130), rgb(170, 20, 20);
    S1_TINT: rgb(120, 170, 255), rgb(48, 104, 214), rgb(150, 195, 255), rgb(20, 70, 180);
    S2_TINT: rgb(255, 140, 82), rgb(230, 104, 42), rgb(255, 170, 120), rgb(170, 60, 8);
    /// Text fields and other sunken areas.
    SUNKEN: rgb(20, 21, 27), rgb(236, 237, 242), rgb(0, 0, 0), rgb(240, 240, 244);
}

/// A faint wash over the background (slider tracks, hover tints): white on dark, black on light.
pub fn overlay(alpha: u8) -> Color32 {
    // High contrast: twice as strong, so slider tracks and hover tints can be seen.
    let alpha = if high_contrast() { alpha.saturating_mul(2) } else { alpha };
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
    restyle(ctx);
}

/// Build both styles from the palettes now in use.
fn restyle(ctx: &egui::Context) {
    let was = is_dark();
    for (theme, dark) in [(egui::Theme::Dark, true), (egui::Theme::Light, false)] {
        set_dark(dark);
        ctx.set_style_of(theme, style(dark));
    }
    set_dark(was);
}

/// Switch *High contrast* on or off (the styles are rebuilt only when it changes).
pub fn set_contrast(ctx: &egui::Context, on: bool) {
    if high_contrast() != on {
        CONTRAST.store(on, Ordering::Relaxed);
        restyle(ctx);
    }
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
    if high_contrast() {
        // Outline every control, so it doesn't rely on a faint fill to be seen.
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.bg_stroke = Stroke::new(1.0_f32, LINE());
        }
    }
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

    /// WCAG contrast ratio between two colours (1 to 21).
    fn ratio(a: Color32, b: Color32) -> f32 {
        let lin = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let l = |c: Color32| 0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b());
        let (hi, lo) = (l(a).max(l(b)), l(a).min(l(b)));
        (hi + 0.05) / (lo + 0.05)
    }

    /// Run `check` once for each of the four palettes, with the flags set to match.
    fn each_palette(mut check: impl FnMut(&str, bool)) {
        let (was_dark, was_hc) = (is_dark(), high_contrast());
        for (name, dark, hc) in [("dark", true, false), ("light", false, false), ("high-contrast dark", true, true), ("high-contrast light", false, true)] {
            set_dark(dark);
            CONTRAST.store(hc, Ordering::Relaxed);
            check(name, hc);
        }
        set_dark(was_dark);
        CONTRAST.store(was_hc, Ordering::Relaxed);
    }

    #[test]
    fn text_is_readable_in_every_palette_and_strongly_so_in_high_contrast() {
        each_palette(|name, hc| {
            for (bg_name, bg) in [("BG", BG()), ("SURFACE", SURFACE()), ("SIDEBAR", SIDEBAR())] {
                // Ordinary palettes: WCAG AA (4.5) for the main and dim text. High contrast: AAA (7).
                let need = if hc { 7.0 } else { 4.5 };
                for (fg_name, fg) in [("TEXT", TEXT()), ("TEXT_DIM", TEXT_DIM())] {
                    let r = ratio(fg, bg);
                    assert!(r >= need, "{fg_name} on {bg_name} in {name}: {r:.2}, needs {need}");
                }
            }
        });
    }

    #[test]
    fn accents_and_the_faint_text_are_readable_in_high_contrast() {
        each_palette(|name, hc| {
            if !hc {
                return;
            }
            for (bg_name, bg) in [("BG", BG()), ("SURFACE", SURFACE()), ("SURFACE_HI", SURFACE_HI())] {
                for (fg_name, fg) in [("TEXT_FAINT", TEXT_FAINT()), ("ACCENT", ACCENT()), ("GOOD", GOOD()), ("BAD", BAD()), ("S1_TINT", S1_TINT())] {
                    let r = ratio(fg, bg);
                    assert!(r >= 4.5, "{fg_name} on {bg_name} in {name}: {r:.2}, needs 4.5");
                }
            }
            assert!(ratio(LINE(), BG()) >= 3.0, "outlines must be visible in {name}");
        });
    }

    #[test]
    fn preference_codes() {
        assert_eq!(preference("light"), egui::ThemePreference::Light);
        assert_eq!(preference("dark"), egui::ThemePreference::Dark);
        assert_eq!(preference("auto"), egui::ThemePreference::System);
        assert_eq!(preference("anything else"), egui::ThemePreference::System);
    }
}
