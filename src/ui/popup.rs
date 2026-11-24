//! The small player a click on the notification-area icon opens (Windows).
//!
//! A borderless window next to the icon with the chosen room's cover, song, seek bar, buttons and
//! volume, and the rooms to choose from. What it shows comes from `traymenu::player` and what its
//! buttons do from `traymenu::run`, the same as the Mac's pop-up player. It is drawn with the
//! app's own widgets as a second window of the same program, so it follows the theme and language,
//! and it closes when it loses the keyboard focus or Escape is pressed.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::time::{Duration, Instant};

use eframe::egui::{self, vec2, Align, Layout, Pos2, RichText, Sense, Stroke};

use super::art::ArtCache;
use super::theme::*;
use super::widgets::*;
use crate::backend::Backend;
use crate::traymenu::{self, Action};

/// The window's size in points.
pub const W: f32 = 300.0;
pub const H: f32 = 520.0;

/// What the player remembers between frames.
#[derive(Default)]
pub struct State {
    seek: Option<f32>,
    volume: Option<f32>,
    focused: bool,
    opened: Option<Instant>,
}

impl State {
    /// Forget the last time it was open (call when it is opened afresh).
    pub fn reset(&mut self) {
        *self = State { opened: Some(Instant::now()), ..State::default() };
    }
}

/// Draw the player with its top-left corner at `at` (in points). Returns whether it should stay
/// open. `open_app` brings the main window forward.
pub fn show(ctx: &egui::Context, backend: &Backend, art: &mut ArtCache, st: &mut State, at: Pos2, open_app: &dyn Fn()) -> bool {
    let id = egui::ViewportId::from_hash_of("roomtone-small-player");
    let builder = egui::ViewportBuilder::default()
        .with_title("Roomtone")
        .with_inner_size([W, H])
        .with_position(at)
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top()
        .with_taskbar(false)
        .with_active(true);
    let mut keep = true;
    let opened = *st.opened.get_or_insert_with(Instant::now);
    ctx.show_viewport_immediate(id, builder, |ctx, _class| {
        let p = traymenu::player(&backend.lock());
        // Closes: Escape, the system asking, or the focus moving elsewhere once it has had it.
        let (focused, escape, close) = ctx.input(|i| (i.viewport().focused, i.key_pressed(egui::Key::Escape), i.viewport().close_requested()));
        if focused == Some(true) {
            st.focused = true;
        }
        if escape || close || (st.focused && focused == Some(false)) || (!st.focused && opened.elapsed() > Duration::from_secs(3)) {
            keep = false;
        }
        let frame = egui::Frame::new().fill(SURFACE()).stroke(Stroke::new(1.0, overlay(36))).inner_margin(16.0);
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            let width = ui.available_width();
            if p.rooms.is_empty() {
                ui.add_space(24.0);
                ui.label(RichText::new(t!("No rooms found yet")).color(TEXT_DIM()));
            } else {
                let chosen = p.rooms.iter().find(|r| r.2).map(|r| r.1.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt("player-room").selected_text(chosen).width(width).show_ui(ui, |ui| {
                    for (gid, name, selected) in &p.rooms {
                        if ui.selectable_label(*selected, name).clicked() {
                            traymenu::run(backend, Action::Select(gid.clone()), &|| {}, &|| {});
                        }
                    }
                });
                let side = 220.0_f32.min(width);
                let (row, _) = ui.allocate_exact_size(vec2(width, side), Sense::hover());
                let r = egui::Rect::from_center_size(row.center(), vec2(side, side));
                if p.title.is_empty() {
                    ui.painter().rect_filled(r, 12.0, SURFACE_HOVER());
                    paint_icon(ui.painter(), egui::Rect::from_center_size(r.center(), vec2(40.0, 40.0)), Icon::Speaker, TEXT_FAINT());
                } else {
                    paint_art(ui, art, r, p.art.as_deref(), &p.title, 12.0);
                }
                let title: &str = if p.title.is_empty() { t!("Nothing playing") } else { &p.title };
                clipped_label(ui, title, 16.0, TEXT(), width, true);
                clipped_label(ui, &p.artist, 13.0, TEXT_DIM(), width, false);
                seek_row(ui, backend, &p, st);
                ui.horizontal(|ui| {
                    ui.add_space((width - 112.0) / 2.0);
                    if icon_button(ui, Icon::Prev, 20.0, TEXT_DIM()).clicked() {
                        traymenu::run(backend, Action::Previous(p.group.clone()), &|| {}, &|| {});
                    }
                    let icon = if p.playing { Icon::Pause } else { Icon::Play };
                    if round_button(ui, icon, 52.0, TEXT(), ON_TEXT()).clicked() {
                        traymenu::run(backend, Action::PlayPause(p.group.clone()), &|| {}, &|| {});
                    }
                    if icon_button(ui, Icon::Next, 20.0, TEXT_DIM()).clicked() {
                        traymenu::run(backend, Action::Next(p.group.clone()), &|| {}, &|| {});
                    }
                });
                volume_row(ui, backend, &p, st);
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                if pill(ui, None, t!("Open Roomtone"), false).clicked() {
                    open_app();
                    keep = false;
                }
            });
        });
        ctx.request_repaint_after(Duration::from_millis(500));
    });
    keep
}

fn seek_row(ui: &mut egui::Ui, backend: &Backend, p: &traymenu::Player, st: &mut State) {
    if p.duration <= 0.0 {
        ui.add_space(20.0);
        return;
    }
    ui.horizontal(|ui| {
        let label_w = 38.0;
        let pos = st.seek.map(|v| v * p.duration).unwrap_or(p.position);
        ui.add_sized([label_w, 16.0], egui::Label::new(RichText::new(fmt_time(pos)).size(11.5).color(TEXT_FAINT())));
        let mut v = st.seek.unwrap_or((p.position / p.duration).clamp(0.0, 1.0));
        let w = ui.available_width() - label_w - 8.0;
        let said = t!("{pos} of {total}", pos = fmt_time(v * p.duration), total = fmt_time(p.duration));
        let (r, changed) = slider(ui, "player-seek", t!("Playback position"), &mut v, w, TEXT(), &said);
        if changed {
            st.seek = Some(v);
        }
        if r.drag_stopped() || (r.clicked() && !r.dragged()) {
            if let Some(v) = st.seek.take() {
                traymenu::seek(backend, &p.group, v * p.duration);
            }
        }
        ui.label(RichText::new(format!("-{}", fmt_time(p.duration - p.position))).size(11.5).color(TEXT_FAINT()));
    });
}

fn volume_row(ui: &mut egui::Ui, backend: &Backend, p: &traymenu::Player, st: &mut State) {
    ui.horizontal(|ui| {
        let icon = if p.muted { Icon::Muted } else { Icon::Speaker };
        if icon_button(ui, icon, 16.0, TEXT_DIM()).clicked() {
            traymenu::run(backend, Action::Mute(p.group.clone()), &|| {}, &|| {});
        }
        let mut v = st.volume.unwrap_or(if p.muted { 0.0 } else { p.volume as f32 / 100.0 });
        let w = ui.available_width() - 4.0;
        let said = t!("{n} of 100", n = (v * 100.0).round() as u32);
        let (r, changed) = slider(ui, "player-volume", t!("Volume"), &mut v, w, ACCENT(), &said);
        if changed {
            st.volume = Some(v);
            traymenu::set_volume(backend, &p.group, (v * 100.0).round() as u8);
        }
        if r.drag_stopped() || (r.clicked() && !r.dragged()) {
            st.volume = None;
        }
    });
}

/// Where the player goes, as its top-left corner in physical pixels: centred over the icon and
/// above it (below it when the taskbar is at the top of the screen), kept inside the work area
/// `(left, top, right, bottom)`. `icon` is the icon's `(left, top, right, bottom)`.
pub fn place(icon: (f32, f32, f32, f32), work: (f32, f32, f32, f32), size: (f32, f32)) -> (f32, f32) {
    let gap = 8.0;
    let cx = (icon.0 + icon.2) / 2.0;
    let x = (cx - size.0 / 2.0).clamp(work.0 + gap, (work.2 - size.0 - gap).max(work.0 + gap));
    let above = icon.1 - size.1 - gap;
    let y = if above >= work.1 { above } else { (icon.3 + gap).min(work.3 - size.1 - gap) };
    (x, y.max(work.1))
}

#[cfg(test)]
mod tests {
    use super::place;

    #[test]
    fn the_player_opens_above_an_icon_in_a_bottom_taskbar() {
        // 1920x1080, taskbar 48 high at the bottom, icon near the right.
        let work = (0.0, 0.0, 1920.0, 1032.0);
        let (x, y) = place((1700.0, 1040.0, 1724.0, 1072.0), work, (300.0, 478.0));
        assert_eq!((x, y), (1700.0 + 12.0 - 150.0, 1040.0 - 478.0 - 8.0));
    }

    #[test]
    fn the_player_stays_on_screen_and_goes_below_a_top_taskbar() {
        let work = (0.0, 48.0, 1920.0, 1080.0);
        let (x, y) = place((1900.0, 8.0, 1920.0, 40.0), work, (300.0, 478.0));
        assert_eq!(x, 1920.0 - 300.0 - 8.0);
        assert_eq!(y, 48.0);
        let (x, _) = place((0.0, 8.0, 20.0, 40.0), work, (300.0, 478.0));
        assert_eq!(x, 8.0);
    }
}
