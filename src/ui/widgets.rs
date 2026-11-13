//! Hand-drawn icons and controls: sliders, switches, pills, artwork tiles.

use eframe::egui::{self, pos2, vec2, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};

use super::art::ArtCache;
use super::theme::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Next,
    Prev,
    Shuffle,
    Repeat,
    RepeatOne,
    Speaker,
    Muted,
    Home,
    Search,
    Star,
    Playlist,
    Library,
    Radio,
    Input,
    Tv,
    Alarm,
    Gear,
    Plus,
    Close,
    Dots,
    Rooms,
    Queue,
    Back,
    Chevron,
    Up,
    Down,
    Moon,
    #[allow(dead_code)]
    Crossfade,
    Sliders,
    Check,
    Refresh,
    Save,
    Trash,
    Services,
    Folder,
    Filter,
}

struct Pen<'a> {
    p: &'a egui::Painter,
    r: Rect,
    c: Color32,
    w: f32,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.r.min.x + x / 24.0 * self.r.width(), self.r.min.y + y / 24.0 * self.r.height())
    }
    fn s(&self, v: f32) -> f32 {
        v / 24.0 * self.r.width()
    }
    fn line(&self, pts: &[(f32, f32)]) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.p.add(Shape::line(pts, Stroke::new(self.w, self.c)));
    }
    fn closed(&self, pts: &[(f32, f32)]) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.p.add(Shape::closed_line(pts, Stroke::new(self.w, self.c)));
    }
    fn fill(&self, pts: &[(f32, f32)]) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.p.add(Shape::convex_polygon(pts, self.c, Stroke::NONE));
    }
    fn circle(&self, x: f32, y: f32, r: f32) {
        self.p.circle_stroke(self.at(x, y), self.s(r), Stroke::new(self.w, self.c));
    }
    fn dot(&self, x: f32, y: f32, r: f32) {
        self.p.circle_filled(self.at(x, y), self.s(r), self.c);
    }
    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let n = 16;
        let pts: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        self.line(&pts);
    }
    fn bar(&self, x0: f32, y0: f32, x1: f32, y1: f32, round: f32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        self.p.rect_filled(rect, self.s(round), self.c);
    }
}

pub fn paint_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let pen = Pen { p: painter, r: rect, c: color, w: (rect.width() / 24.0 * 1.8).max(1.2) };
    let arrow = |x: f32, y: f32, dx: f32| pen.line(&[(x - 2.6 * dx, y - 2.6), (x, y), (x - 2.6 * dx, y + 2.6)]);
    match icon {
        Icon::Play => pen.fill(&[(7.5, 4.5), (19.5, 12.0), (7.5, 19.5)]),
        Icon::Pause => {
            pen.bar(6.0, 5.0, 10.0, 19.0, 1.2);
            pen.bar(14.0, 5.0, 18.0, 19.0, 1.2);
        }
        Icon::Next => {
            pen.fill(&[(4.5, 5.5), (15.0, 12.0), (4.5, 18.5)]);
            pen.bar(16.0, 5.5, 19.0, 18.5, 1.0);
        }
        Icon::Prev => {
            pen.fill(&[(19.5, 5.5), (9.0, 12.0), (19.5, 18.5)]);
            pen.bar(5.0, 5.5, 8.0, 18.5, 1.0);
        }
        Icon::Shuffle => {
            pen.line(&[(3.0, 7.0), (7.5, 7.0), (14.5, 17.0), (20.0, 17.0)]);
            pen.line(&[(3.0, 17.0), (7.5, 17.0), (14.5, 7.0), (20.0, 7.0)]);
            arrow(20.5, 7.0, 1.0);
            arrow(20.5, 17.0, 1.0);
        }
        Icon::Repeat | Icon::RepeatOne => {
            pen.line(&[(4.5, 12.5), (4.5, 8.0), (19.0, 8.0)]);
            arrow(19.5, 8.0, 1.0);
            pen.line(&[(19.5, 11.5), (19.5, 16.0), (5.0, 16.0)]);
            arrow(4.5, 16.0, -1.0);
            if icon == Icon::RepeatOne {
                pen.line(&[(11.0, 10.8), (12.4, 10.0), (12.4, 14.0)]);
            }
        }
        Icon::Speaker | Icon::Muted => {
            pen.fill(&[(3.5, 9.0), (7.5, 9.0), (12.5, 4.5), (12.5, 19.5), (7.5, 15.0), (3.5, 15.0)]);
            if icon == Icon::Speaker {
                pen.arc(12.5, 12.0, 4.0, -45.0, 45.0);
                pen.arc(12.5, 12.0, 7.5, -50.0, 50.0);
            } else {
                pen.line(&[(15.5, 9.0), (21.0, 15.0)]);
                pen.line(&[(21.0, 9.0), (15.5, 15.0)]);
            }
        }
        Icon::Home => {
            pen.line(&[(3.5, 11.5), (12.0, 4.0), (20.5, 11.5)]);
            pen.line(&[(6.0, 9.5), (6.0, 20.0), (18.0, 20.0), (18.0, 9.5)]);
            pen.line(&[(10.0, 20.0), (10.0, 14.5), (14.0, 14.5), (14.0, 20.0)]);
        }
        Icon::Search => {
            pen.circle(10.5, 10.5, 6.0);
            pen.line(&[(15.0, 15.0), (20.0, 20.0)]);
        }
        Icon::Star => {
            let pts: Vec<(f32, f32)> = (0..10)
                .map(|i| {
                    let a = (-90.0 + i as f32 * 36.0f32).to_radians();
                    let r = if i % 2 == 0 { 8.5 } else { 3.8 };
                    (12.0 + r * a.cos(), 12.8 + r * a.sin())
                })
                .collect();
            pen.closed(&pts);
        }
        Icon::Playlist => {
            pen.line(&[(4.0, 6.5), (16.0, 6.5)]);
            pen.line(&[(4.0, 11.5), (16.0, 11.5)]);
            pen.line(&[(4.0, 16.5), (10.0, 16.5)]);
            pen.dot(16.0, 18.0, 2.4);
            pen.line(&[(18.2, 18.0), (18.2, 10.0), (21.0, 11.0)]);
        }
        Icon::Library => {
            pen.line(&[(5.0, 4.5), (5.0, 19.5)]);
            pen.line(&[(9.5, 4.5), (9.5, 19.5)]);
            pen.line(&[(13.5, 5.5), (18.5, 19.0)]);
        }
        Icon::Radio => {
            pen.dot(12.0, 12.0, 2.0);
            pen.arc(12.0, 12.0, 5.0, 140.0, 220.0);
            pen.arc(12.0, 12.0, 5.0, -40.0, 40.0);
            pen.arc(12.0, 12.0, 8.5, 140.0, 220.0);
            pen.arc(12.0, 12.0, 8.5, -40.0, 40.0);
        }
        Icon::Input => {
            pen.closed(&[(8.0, 8.0), (16.0, 8.0), (16.0, 13.0), (14.0, 15.5), (10.0, 15.5), (8.0, 13.0)]);
            pen.line(&[(10.0, 8.0), (10.0, 3.5)]);
            pen.line(&[(14.0, 8.0), (14.0, 3.5)]);
            pen.line(&[(12.0, 15.5), (12.0, 21.0)]);
        }
        Icon::Tv => {
            pen.closed(&[(3.0, 5.0), (21.0, 5.0), (21.0, 17.0), (3.0, 17.0)]);
            pen.line(&[(8.0, 20.5), (16.0, 20.5)]);
        }
        Icon::Alarm => {
            pen.circle(12.0, 13.0, 7.5);
            pen.line(&[(12.0, 9.0), (12.0, 13.0), (15.0, 15.0)]);
            pen.line(&[(3.5, 6.5), (6.5, 3.5)]);
            pen.line(&[(20.5, 6.5), (17.5, 3.5)]);
        }
        Icon::Gear => {
            pen.circle(12.0, 12.0, 3.0);
            pen.circle(12.0, 12.0, 6.5);
            for i in 0..8 {
                let a = (i as f32 * 45.0f32).to_radians();
                pen.line(&[(12.0 + 6.5 * a.cos(), 12.0 + 6.5 * a.sin()), (12.0 + 9.0 * a.cos(), 12.0 + 9.0 * a.sin())]);
            }
        }
        Icon::Plus => {
            pen.line(&[(12.0, 5.0), (12.0, 19.0)]);
            pen.line(&[(5.0, 12.0), (19.0, 12.0)]);
        }
        Icon::Close => {
            pen.line(&[(6.0, 6.0), (18.0, 18.0)]);
            pen.line(&[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::Dots => {
            pen.dot(5.5, 12.0, 1.8);
            pen.dot(12.0, 12.0, 1.8);
            pen.dot(18.5, 12.0, 1.8);
        }
        Icon::Rooms => {
            pen.closed(&[(3.5, 5.0), (10.5, 5.0), (10.5, 19.0), (3.5, 19.0)]);
            pen.closed(&[(13.5, 5.0), (20.5, 5.0), (20.5, 19.0), (13.5, 19.0)]);
            pen.circle(7.0, 14.0, 2.0);
            pen.circle(17.0, 14.0, 2.0);
            pen.dot(7.0, 8.5, 0.9);
            pen.dot(17.0, 8.5, 0.9);
        }
        Icon::Queue => {
            pen.line(&[(4.0, 6.5), (18.0, 6.5)]);
            pen.line(&[(4.0, 11.5), (18.0, 11.5)]);
            pen.line(&[(4.0, 16.5), (11.0, 16.5)]);
            pen.fill(&[(14.0, 14.0), (20.0, 17.0), (14.0, 20.0)]);
        }
        Icon::Back => pen.line(&[(15.0, 5.0), (8.0, 12.0), (15.0, 19.0)]),
        Icon::Chevron => pen.line(&[(9.0, 5.0), (16.0, 12.0), (9.0, 19.0)]),
        Icon::Up => pen.line(&[(5.0, 15.0), (12.0, 8.0), (19.0, 15.0)]),
        Icon::Down => pen.line(&[(5.0, 9.0), (12.0, 16.0), (19.0, 9.0)]),
        Icon::Moon => {
            let mut pts = vec![];
            for i in 0..=20 {
                let a = (40.0 + 280.0 * i as f32 / 20.0f32).to_radians();
                pts.push((12.0 + 8.0 * a.cos(), 12.0 + 8.0 * a.sin()));
            }
            for i in 0..=20 {
                let a = (300.0 - 220.0 * i as f32 / 20.0f32).to_radians();
                pts.push((15.5 + 6.0 * a.cos(), 9.5 + 6.0 * a.sin()));
            }
            pen.closed(&pts);
        }
        Icon::Crossfade => {
            pen.line(&[(3.0, 7.0), (8.0, 7.0), (16.0, 17.0), (21.0, 17.0)]);
            pen.line(&[(3.0, 17.0), (8.0, 17.0), (16.0, 7.0), (21.0, 7.0)]);
        }
        Icon::Sliders => {
            for (x, k) in [(6.0, 15.0), (12.0, 8.0), (18.0, 13.0)] {
                pen.line(&[(x, 4.0), (x, 20.0)]);
                pen.dot(x, k, 2.4);
            }
        }
        Icon::Check => pen.line(&[(5.0, 12.5), (10.0, 17.5), (19.0, 7.0)]),
        Icon::Refresh => {
            pen.arc(12.0, 12.0, 7.0, -60.0, 230.0);
            pen.line(&[(15.5, 2.5), (15.5, 6.0), (19.0, 6.0)]);
        }
        Icon::Save => {
            pen.line(&[(12.0, 4.0), (12.0, 15.0)]);
            pen.line(&[(7.5, 10.5), (12.0, 15.0), (16.5, 10.5)]);
            pen.line(&[(4.5, 15.0), (4.5, 20.0), (19.5, 20.0), (19.5, 15.0)]);
        }
        Icon::Services => {
            for (x, y) in [(4.0, 4.0), (13.0, 4.0), (4.0, 13.0)] {
                pen.closed(&[(x, y), (x + 7.0, y), (x + 7.0, y + 7.0), (x, y + 7.0)]);
            }
            pen.line(&[(16.5, 13.0), (16.5, 20.0)]);
            pen.line(&[(13.0, 16.5), (20.0, 16.5)]);
        }
        Icon::Folder => {
            pen.closed(&[(3.0, 6.0), (9.5, 6.0), (11.5, 8.5), (21.0, 8.5), (21.0, 19.0), (3.0, 19.0)]);
        }
        Icon::Filter => {
            pen.line(&[(4.0, 7.0), (20.0, 7.0)]);
            pen.line(&[(7.0, 12.0), (17.0, 12.0)]);
            pen.line(&[(10.0, 17.0), (14.0, 17.0)]);
        }
        Icon::Trash => {
            pen.line(&[(4.0, 7.0), (20.0, 7.0)]);
            pen.line(&[(9.0, 7.0), (9.5, 4.0), (14.5, 4.0), (15.0, 7.0)]);
            pen.line(&[(6.0, 7.0), (7.0, 20.0), (17.0, 20.0), (18.0, 7.0)]);
        }
    }
}

/// A round, quiet icon button that lights up on hover.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size + 12.0, size + 12.0), Sense::click());
    if resp.hovered() {
        ui.painter().circle_filled(rect.center(), rect.width() / 2.0, SURFACE_HOVER());
    }
    let r = Rect::from_center_size(rect.center(), vec2(size, size));
    paint_icon(ui.painter(), r, icon, if resp.hovered() { TEXT() } else { color });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A filled circle with an icon (the big play button).
pub fn round_button(ui: &mut Ui, icon: Icon, diameter: f32, fill: Color32, fg: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(diameter, diameter), Sense::click());
    let grow = if resp.hovered() { 1.5 } else { 0.0 };
    ui.painter().circle_filled(rect.center(), diameter / 2.0 + grow, fill);
    let s = diameter * 0.46;
    let offset = if icon == Icon::Play { vec2(diameter * 0.03, 0.0) } else { vec2(0.0, 0.0) };
    paint_icon(ui.painter(), Rect::from_center_size(rect.center() + offset, vec2(s, s)), icon, fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A rounded "pill" button with optional icon.
pub fn pill(ui: &mut Ui, icon: Option<Icon>, text: &str, primary: bool) -> Response {
    let font = egui::FontId::proportional(14.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, if primary { Color32::BLACK } else { TEXT() });
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let size = vec2(galley.size().x + 32.0 + icon_w, 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = match (primary, resp.hovered()) {
        (true, false) => ACCENT(),
        (true, true) => Color32::from_rgb(255, 160, 110),
        (false, false) => SURFACE_HI(),
        (false, true) => SURFACE_HOVER(),
    };
    ui.painter().rect_filled(rect, 18.0, fill);
    let mut x = rect.min.x + 16.0;
    if let Some(i) = icon {
        let r = Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0));
        paint_icon(ui.painter(), r, i, if primary { Color32::BLACK } else { TEXT() });
        x += icon_w;
    }
    ui.painter().galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, TEXT());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A slim slider with a knob that appears on hover. `value` is 0..=1.
pub fn slider(ui: &mut Ui, id: impl std::hash::Hash, value: &mut f32, width: f32, color: Color32) -> (Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 20.0), Sense::click_and_drag());
    let _ = id;
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on() {
            let v = ((p.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            if (v - *value).abs() > f32::EPSILON {
                *value = v;
                changed = true;
            }
        }
    }
    let hot = resp.hovered() || resp.dragged();
    let h = if hot { 6.0 } else { 4.0 };
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), h));
    let p = ui.painter();
    p.rect_filled(track, h / 2.0, overlay(28));
    let filled = Rect::from_min_max(track.min, pos2(track.min.x + track.width() * value.clamp(0.0, 1.0), track.max.y));
    p.rect_filled(filled, h / 2.0, color);
    if hot {
        p.circle_filled(pos2(filled.max.x, rect.center().y), 7.0, TEXT());
    }
    (resp.on_hover_cursor(egui::CursorIcon::PointingHand), changed)
}

/// An on/off switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let fill = lerp_color(SURFACE_HOVER(), ACCENT(), t);
    ui.painter().rect_filled(rect, 11.0, fill);
    let x = egui::lerp(rect.min.x + 11.0..=rect.max.x - 11.0, t);
    ui.painter().circle_filled(pos2(x, rect.center().y), 8.0, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Segmented control; returns the newly picked index.
pub fn segmented(ui: &mut Ui, options: &[&str], selected: usize, width: f32) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 32.0), Sense::hover());
    ui.painter().rect_filled(rect, 16.0, SURFACE());
    let w = rect.width() / options.len() as f32;
    let mut picked = None;
    for (i, label) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.min.x + w * i as f32, rect.min.y), vec2(w, rect.height())).shrink(3.0);
        let resp = ui.interact(r, ui.scope_id().with(("seg", label)), Sense::click());
        if i == selected {
            ui.painter().rect_filled(r, 13.0, SURFACE_HOVER());
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 13.0, overlay(8));
        }
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *label, egui::FontId::proportional(13.0), if i == selected { TEXT() } else { TEXT_DIM() });
        if resp.clicked() && i != selected {
            picked = Some(i);
        }
    }
    picked
}

pub fn paint_art(ui: &Ui, art: &mut ArtCache, rect: Rect, url: Option<&str>, seed: &str, radius: f32) {
    let tex = art.texture(ui.ctx(), url, seed);
    egui::Image::new(egui::load::SizedTexture::new(tex, rect.size())).corner_radius(radius).paint_at(ui, rect);
    if !art.is_ready(url) {
        // A small note mark so placeholders read as music.
        let s = rect.width().min(rect.height()) * 0.2;
        let c = rect.center();
        let col = Color32::from_white_alpha(46);
        let p = ui.painter();
        p.circle_filled(pos2(c.x - s * 0.25, c.y + s * 0.35), s * 0.22, col);
        p.line_segment([pos2(c.x - s * 0.04, c.y + s * 0.35), pos2(c.x - s * 0.04, c.y - s * 0.45)], Stroke::new((s * 0.09).max(1.0), col));
        p.line_segment([pos2(c.x - s * 0.04, c.y - s * 0.45), pos2(c.x + s * 0.38, c.y - s * 0.28)], Stroke::new((s * 0.09).max(1.0), col));
    }
}

pub fn chip(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let font = egui::FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(14.0, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 9.0, color.gamma_multiply(0.18));
    ui.painter().galley(rect.min + vec2(7.0, 3.0), galley, color);
    resp
}

/// Text cut to fit `width`, with an ellipsis.
pub fn clipped_label(ui: &mut Ui, text: &str, size: f32, color: Color32, width: f32, strong: bool) -> Response {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat { font_id: egui::FontId::proportional(size), color, ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: width, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let galley = ui.painter().layout_job(job);
    let (rect, resp) = ui.allocate_exact_size(vec2(width.min(galley.size().x.max(1.0)), galley.size().y), Sense::hover());
    ui.painter().galley(rect.min, galley.clone(), color);
    if strong {
        // Faux bold: draw twice with a hair's offset.
        ui.painter().galley(rect.min + vec2(0.35, 0.0), galley, color);
    }
    resp
}

pub fn section_title(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(text).size(19.0).color(TEXT()).strong());
    ui.add_space(4.0);
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new().fill(SURFACE()).corner_radius(14.0).inner_margin(16.0)
}

pub fn fmt_time(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn spinner_dots(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(44.0, 14.0), Sense::hover());
    let t = ui.input(|i| i.time) as f32;
    for i in 0..3 {
        let phase = ((t * 3.0 - i as f32 * 0.6).sin() + 1.0) / 2.0;
        ui.painter().circle_filled(pos2(rect.min.x + 8.0 + i as f32 * 14.0, rect.center().y), 3.0 + phase * 2.0, color.gamma_multiply(0.4 + phase * 0.6));
    }
    ui.ctx().request_repaint();
}

/// A rounded filter chip that can be switched on and off.
pub fn filter_chip(ui: &mut Ui, text: &str, on: bool) -> Response {
    let font = egui::FontId::proportional(12.5);
    let color = if on { Color32::BLACK } else { TEXT_DIM() };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(22.0, 12.0), Sense::click());
    let fill = if on {
        ACCENT()
    } else if resp.hovered() {
        SURFACE_HOVER()
    } else {
        SURFACE_HI()
    };
    ui.painter().rect_filled(rect, rect.height() / 2.0, fill);
    ui.painter().galley(rect.min + vec2(11.0, 6.0), galley, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small coloured badge naming where a result came from.
pub fn source_badge(ui: &Ui, pos: Pos2, text: &str) -> f32 {
    let color = super::art::seed_color(text);
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(10.5), color);
    let r = Rect::from_min_size(pos, galley.size() + vec2(10.0, 4.0));
    ui.painter().rect_filled(r, 7.0, color.gamma_multiply(0.16));
    ui.painter().galley(r.min + vec2(5.0, 2.0), galley, color);
    r.width()
}
