//! The page area: home, search, browsing, now playing, alarms and settings.

use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Align, Color32, Layout, Rect, RichText, Sense, Ui};

use super::art::seed_color;
use super::lists;
use super::theme::*;
use super::widgets::*;
use super::{paint_text, App, View};
use crate::backend::Cmd;
use crate::model::*;

/// An item's second line; podcast episodes start it with whether they're played or how much is left
/// (first, so it isn't the part cut off on a narrow tile).
fn item_subtitle(item: &Item) -> String {
    use crate::sources::progress::{self, Status};
    let base = item.subtitle();
    if !progress::is_episode(item) {
        return base;
    }
    match progress::status(&item.uri) {
        Status::Played => format!("{} · {base}", t!("Played")),
        Status::InProgress { left } if left > 0.0 => format!("{} · {base}", t!("{n} min left", n = ((left / 60.0).ceil() as u32).max(1))),
        _ => base,
    }
}
use crate::net::speaker::PlayHow;

const LIBRARY_ROOT: &[(&str, &str, &str)] = &[
    ("A:ARTIST", n_!("Artists"), n_!("Browse by performer")),
    ("A:ALBUMARTIST", n_!("Album Artists"), n_!("Browse by album artist")),
    ("A:ALBUM", n_!("Albums"), n_!("Every album in your library")),
    ("A:TRACKS", n_!("Songs"), n_!("All songs")),
    ("A:GENRE", n_!("Genres"), n_!("Browse by style")),
    ("A:COMPOSER", n_!("Composers"), n_!("Browse by composer")),
    ("A:PLAYLISTS", n_!("Imported Playlists"), n_!("Playlist files found in your library")),
    ("S:", n_!("Folders"), n_!("Shared folders on your network")),
];

/// Combined library lists: (id, title, the system's list, this computer's list, icon).
const MERGED: &[(&str, &str, &str, &str, Icon)] = &[
    ("ALL:ARTISTS", n_!("Artists"), "A:ARTIST", "LOCAL:ARTISTS", Icon::Rooms),
    ("ALL:ALBUMS", n_!("Albums"), "A:ALBUM", "LOCAL:ALBUMS", Icon::Library),
    ("ALL:TRACKS", n_!("Songs"), "A:TRACKS", "LOCAL:TRACKS", Icon::Queue),
    ("ALL:GENRES", n_!("Genres"), "A:GENRE", "LOCAL:GENRES", Icon::Sliders),
    ("ALL:FOLDERS", n_!("Folders"), "S:", "LOCAL:FOLDERS", Icon::Folder),
];

fn seed_of(item: &Item) -> &str {
    if item.album.is_empty() {
        &item.title
    } else {
        &item.album
    }
}

impl App {
    pub(super) fn page(&mut self, ui: &mut Ui, s: &AppState) {
        if s.phase == Phase::Demo {
            self.demo_banner(ui);
        }
        if let Some(top) = self.stack.last().cloned() {
            self.container_page(ui, s, &top);
            return;
        }
        match self.view.clone() {
            View::Home => self.home(ui, s),
            View::Search => self.search_page(ui, s),
            View::Favorites => self.grid_page(ui, s, t!("Favorites"), t!("Stations, playlists and albums you've saved"), "FV:2"),
            View::Playlists => self.grid_page(ui, s, t!("Playlists"), t!("Playlists saved on your system"), "SQ:"),
            View::Library => self.library_root(ui, s),
            View::Radio => self.radio_page(ui, s),
            View::Inputs => self.inputs_page(ui, s),
            View::Alarms => self.alarms_page(ui, s),
            View::Settings => self.settings_page(ui, s),
            View::NowPlaying => self.now_playing_page(ui, s),
            View::Room(uuid) => self.room_page(ui, s, &uuid),
            View::Services => self.services_page(ui, s),
            View::History => self.history_page(ui, s),
        }
    }

    fn demo_banner(&mut self, ui: &mut Ui) {
        egui::Frame::new().fill(ACCENT_SOFT()).corner_radius(12.0).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("Demo system")).strong().color(ACCENT()));
                ui.label(RichText::new(t!("No speakers answered, so you're exploring made-up rooms.")).size(13.0).color(TEXT_DIM()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new(t!("Search again")).size(13.0)).corner_radius(14.0)).clicked() {
                        self.send(Cmd::Discover);
                    }
                });
            });
        });
        ui.add_space(14.0);
    }

    fn header(&mut self, ui: &mut Ui, title: &str, subtitle: &str) {
        ui.label(RichText::new(title).size(30.0).strong().color(TEXT()));
        if !subtitle.is_empty() {
            ui.label(RichText::new(subtitle).size(14.0).color(TEXT_DIM()));
        }
        ui.add_space(14.0);
    }

    // ---------- Home ----------

    fn home(&mut self, ui: &mut Ui, s: &AppState) {
        let Some(h) = s.household() else { return };
        let playing = h.groups.iter().filter(|g| g.now.transport == Transport::Playing).count();
        self.header(
            ui,
            t!("Home"),
            &format!(
                "{} · {} · {}",
                t!("{system} system", system = h.system.label()),
                tn!(h.rooms.len(), "1 room", "{n} rooms"),
                if playing == 0 { t!("nothing playing").to_string() } else { tn!(playing, "1 playing", "{n} playing") }
            ),
        );
        if let Some((h, g)) = s.selected_group.as_ref().and_then(|g| s.find_group(g)) {
            self.hero(ui, h, g);
            ui.add_space(22.0);
        }
        let gid = s.selected_group.clone();
        let mut favs: Vec<Item> = match self.browse(s, "FV:2") {
            Some(Browse::Ready(v)) => v.clone(),
            _ => vec![],
        };
        favs.extend(s.config.favorites.iter().cloned());
        if !favs.is_empty() {
            if self.section_with_link(ui, t!("Your favorites"), t!("See all")) {
                self.go(View::Favorites);
            }
            self.tile_row(ui, &favs, gid.as_deref(), "home_fav");
            ui.add_space(18.0);
        }
        let resume: Vec<Item> = crate::sources::progress::continue_list(8).into_iter().map(|e| e.item).collect();
        if !resume.is_empty() {
            section_title(ui, t!("Continue listening"));
            self.tile_row(ui, &resume, gid.as_deref(), "home_resume");
            ui.add_space(18.0);
        }
        if !s.recent.is_empty() {
            if s.config.remember_history && crate::history::count() > 0 {
                if self.section_with_link(ui, t!("Recently played"), t!("History")) {
                    self.go(View::History);
                }
            } else {
                section_title(ui, t!("Recently played"));
            }
            self.tile_row(ui, &s.recent, gid.as_deref(), "home_recent");
            ui.add_space(18.0);
        }
        if let Some(Browse::Ready(pls)) = self.browse(s, "SQ:") {
            if !pls.is_empty() {
                if self.section_with_link(ui, t!("Your playlists"), t!("See all")) {
                    self.go(View::Playlists);
                }
                self.tile_row(ui, pls, gid.as_deref(), "home_pl");
            }
        }
    }

    fn section_with_link(&mut self, ui: &mut Ui, title: &str, link: &str) -> bool {
        let mut clicked = false;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(19.0).color(TEXT()).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let r = ui.add(egui::Label::new(RichText::new(link).size(13.0).color(TEXT_DIM())).sense(Sense::click()));
                if r.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                clicked = r.clicked();
            });
        });
        ui.add_space(4.0);
        clicked
    }

    fn hero(&mut self, ui: &mut Ui, h: &Household, g: &Group) {
        let w = ui.available_width();
        let height = 190.0;
        let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
        let seed = if g.now.album.is_empty() { g.now.title.clone() } else { g.now.album.clone() };
        let tint = if g.now.is_empty() { SURFACE_HI() } else { seed_color(&seed) };
        // A dark card washed from the left with the artwork's colour.
        let p = ui.painter();
        p.rect_filled(rect, 20.0, SURFACE());
        let a = tint.gamma_multiply(0.42);
        let clear = Color32::TRANSPARENT;
        let inner = Rect::from_min_max(rect.min + vec2(20.0, 0.0), rect.max - vec2(20.0, 0.0));
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(inner.left_top(), a);
        mesh.colored_vertex(inner.right_top(), clear);
        mesh.colored_vertex(inner.right_bottom(), clear);
        mesh.colored_vertex(inner.left_bottom(), a);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(mesh);
        p.rect_filled(Rect::from_min_max(rect.min, pos2(inner.min.x, rect.max.y)), egui::CornerRadius { nw: 20, sw: 20, ne: 0, se: 0 }, a);

        let art_r = Rect::from_min_size(rect.min + vec2(20.0, 20.0), vec2(150.0, 150.0));
        if g.now.is_empty() {
            ui.painter().rect_filled(art_r, 14.0, SURFACE_HOVER());
            paint_icon(ui.painter(), Rect::from_center_size(art_r.center(), vec2(48.0, 48.0)), Icon::Speaker, TEXT_FAINT());
        } else {
            paint_art(ui, &mut self.art, art_r, g.now.art.as_deref(), &seed, 14.0);
        }
        let x = art_r.max.x + 24.0;
        let tw = rect.max.x - x - 24.0;
        let state = match g.now.transport {
            Transport::Playing => t!("NOW PLAYING"),
            Transport::Paused => t!("PAUSED"),
            Transport::Transitioning => t!("LOADING"),
            Transport::Stopped => t!("READY"),
        };
        paint_text(ui, pos2(x, rect.min.y + 26.0), &format!("{state}  ·  {}", h.group_name(g).to_uppercase()), 11.5, ACCENT(), tw);
        let title = if g.now.is_empty() { t!("Nothing playing yet") } else { &g.now.title };
        paint_text(ui, pos2(x, rect.min.y + 48.0), title, 26.0, TEXT(), tw);
        let sub = if g.now.is_empty() {
            t!("Pick something from your favorites below").to_string()
        } else {
            [g.now.artist.as_str(), g.now.album.as_str()].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" · ")
        };
        paint_text(ui, pos2(x, rect.min.y + 84.0), &sub, 14.5, TEXT_DIM(), tw);

        let bar_rect = Rect::from_min_max(pos2(x, rect.min.y + 116.0), pos2(rect.max.x - 20.0, rect.max.y - 22.0));
        let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(bar_rect).layout(Layout::left_to_right(Align::Center)));
        let playing = g.now.transport == Transport::Playing;
        if round_button(&mut bar, if playing { Icon::Pause } else { Icon::Play }, 46.0, ACCENT(), Color32::BLACK).clicked() {
            self.send(if playing { Cmd::Pause(g.id.clone()) } else { Cmd::Play(g.id.clone()) });
        }
        bar.add_space(6.0);
        if icon_button(&mut bar, Icon::Prev, 18.0, TEXT()).clicked() {
            self.send(Cmd::Previous(g.id.clone()));
        }
        if icon_button(&mut bar, Icon::Next, 18.0, TEXT()).clicked() {
            self.send(Cmd::Next(g.id.clone()));
        }
        if g.episode.is_some() {
            self.skip_buttons(&mut bar, g);
        }
        self.now_star(&mut bar, &g.now, 18.0);
        bar.add_space(8.0);
        if pill(&mut bar, Some(Icon::Up), t!("Open player"), false).clicked() {
            self.go(View::NowPlaying);
        }
        if g.now.duration > 0.0 {
            let frac = (g.now.live_position() / g.now.duration).clamp(0.0, 1.0);
            let track = Rect::from_min_size(pos2(x, rect.max.y - 16.0), vec2(tw, 3.0));
            ui.painter().rect_filled(track, 1.5, overlay(25));
            ui.painter().rect_filled(Rect::from_min_size(track.min, vec2(tw * frac, 3.0)), 1.5, ACCENT());
        }
    }

    /// The sound of what's playing (format, sample rate, bit depth, bitrate) where the speaker says,
    /// and a plain list of what it sent, to copy into a report.
    fn quality_row(&mut self, ui: &mut Ui, now: &NowPlaying) {
        let q = crate::quality::of(&now.uri, &now.media_uri, &now.track_meta);
        if !q.is_empty() {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for l in q.labels() {
                    chip(ui, &l, TEXT_DIM());
                }
                if q.lossless {
                    chip(ui, t!("Lossless"), GOOD());
                }
            });
        }
        if q.details.is_empty() {
            return;
        }
        egui::CollapsingHeader::new(RichText::new(t!("Technical details")).size(12.5).color(TEXT_FAINT())).id_salt("np_quality").show(ui, |ui| {
            ui.label(RichText::new(t!("This is what the speaker told Roomtone about this song. Anything it doesn't send can't be shown.")).size(12.0).color(TEXT_FAINT()));
            ui.add_space(4.0);
            for (k, v) in &q.details {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(k).size(12.0).color(TEXT_DIM()));
                    ui.label(RichText::new(v).size(12.0).monospace().color(TEXT()));
                });
            }
            ui.add_space(6.0);
            if pill(ui, None, t!("Copy details"), false).clicked() {
                ui.ctx().copy_text(q.report());
                self.toast_here(t!("Details copied"));
            }
        });
    }

    /// Podcast episodes: jump back 15 seconds or ahead 30.
    fn skip_buttons(&mut self, ui: &mut Ui, g: &Group) {
        let now = g.now.live_position();
        let limit = if g.now.duration > 0.0 { g.now.duration } else { f32::MAX };
        if pill(ui, None, t!("−15 s"), false).on_hover_text(t!("Skip back 15 seconds")).clicked() {
            self.send(Cmd::Seek(g.id.clone(), (now - 15.0).max(0.0)));
        }
        if pill(ui, None, t!("+30 s"), false).on_hover_text(t!("Skip forward 30 seconds")).clicked() {
            self.send(Cmd::Seek(g.id.clone(), (now + 30.0).min(limit)));
        }
    }

    // ---------- Tiles and rows ----------

    fn tile_size(width: f32) -> (usize, f32) {
        let gap = 18.0;
        let cols = ((width + gap) / (160.0 + gap)).floor().max(2.0) as usize;
        let size = ((width - gap * (cols as f32 - 1.0)) / cols as f32).floor() - 1.0;
        (cols, size)
    }

    fn tile_row(&mut self, ui: &mut Ui, items: &[Item], gid: Option<&str>, salt: &str) {
        let (cols, size) = Self::tile_size(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            for (i, it) in items.iter().take(cols).enumerate() {
                self.tile(ui, it, size, gid, (salt, i));
            }
        });
    }

    fn tile_grid(&mut self, ui: &mut Ui, items: &[Item], gid: Option<&str>, salt: &str) {
        let (cols, size) = Self::tile_size(ui.available_width());
        for (r, chunk) in items.chunks(cols).enumerate() {
            if self.jump_to.is_some_and(|j| j >= r * cols && j < (r + 1) * cols) {
                // A letter was clicked in the A–Z strip: bring this row of tiles to the top.
                self.jump_to = None;
                ui.scroll_to_rect(Rect::from_min_size(ui.cursor().min, vec2(1.0, size + 46.0)), Some(Align::TOP));
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                for (i, it) in chunk.iter().enumerate() {
                    self.tile(ui, it, size, gid, (salt, r * cols + i));
                }
            });
            ui.add_space(12.0);
        }
    }

    fn tile(&mut self, ui: &mut Ui, item: &Item, size: f32, gid: Option<&str>, salt: (&str, usize)) {
        let (rect, resp) = ui.allocate_exact_size(vec2(size, size + 46.0), Sense::click());
        describe(&resp, egui::Role::Button, &if item.artist.is_empty() { item.title.clone() } else { format!("{} — {}", item.title, item.artist) });
        let base = Rect::from_min_size(rect.min, vec2(size, size));
        focus_ring(ui, &resp, base, 14.0);
        let hovered = resp.hovered();
        let art_r = base.translate(vec2(0.0, if hovered { -3.0 } else { 0.0 }));
        if hovered {
            ui.painter().rect_filled(art_r.translate(vec2(0.0, 6.0)).shrink(6.0), 14.0, Color32::from_black_alpha(90));
        }
        paint_art(ui, &mut self.art, art_r, item.art.as_deref(), seed_of(item), 14.0);
        let play_c = art_r.right_bottom() - vec2(30.0, 30.0);
        let play_r = Rect::from_center_size(play_c, vec2(42.0, 42.0));
        let play = ui.interact(play_r, ui.scope_id().with(("tile_play", salt.0, salt.1)), Sense::click());
        describe(&play, egui::Role::Button, &t!("Play {title}", title = item.title));
        focus_ring(ui, &play, play_r, 21.0);
        if hovered || play.hovered() {
            ui.painter().circle_filled(play_c, if play.hovered() { 22.0 } else { 20.0 }, ACCENT());
            paint_icon(ui.painter(), Rect::from_center_size(play_c + vec2(1.0, 0.0), vec2(18.0, 18.0)), Icon::Play, Color32::BLACK);
        }
        paint_text(ui, pos2(rect.min.x + 2.0, base.max.y + 8.0), &item.title, 14.0, TEXT(), size - 4.0);
        paint_text(ui, pos2(rect.min.x + 2.0, base.max.y + 27.0), &item_subtitle(item), 12.0, TEXT_DIM(), size - 4.0);
        if play.clicked() {
            self.play(gid, item, PlayHow::Now);
        } else if resp.clicked() {
            if item.browsable() {
                self.open(item);
            } else {
                self.play(gid, item, PlayHow::Now);
            }
        }
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        resp.context_menu(|ui| self.item_menu(ui, item, gid));
    }

    fn play(&mut self, gid: Option<&str>, item: &Item, how: PlayHow) {
        if let Some(g) = gid {
            self.send(Cmd::PlayItem(g.to_string(), item.clone(), how));
        }
    }

    fn row(&mut self, ui: &mut Ui, item: &Item, gid: Option<&str>, salt: (&str, usize), number: Option<usize>) {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click());
        let dots_r = Rect::from_center_size(pos2(rect.max.x - 24.0, rect.center().y), vec2(30.0, 30.0));
        let dots = ui.interact(dots_r, ui.scope_id().with(("row_dots", salt.0, salt.1)), Sense::click());
        describe(&dots, egui::Role::Button, t!("More"));
        describe(&resp, egui::Role::Button, &if item.artist.is_empty() { item.title.clone() } else { format!("{} — {}", item.title, item.artist) });
        focus_ring(ui, &resp, rect, 12.0);
        focus_ring(ui, &dots, dots_r, 15.0);
        let hovered = resp.hovered() || dots.hovered();
        if hovered {
            ui.painter().rect_filled(rect, 12.0, SURFACE());
        }
        let mut x = rect.min.x + 10.0;
        if let Some(n) = number {
            ui.painter().text(pos2(x + 10.0, rect.center().y), egui::Align2::CENTER_CENTER, n.to_string(), egui::FontId::proportional(13.0), TEXT_FAINT());
            x += 30.0;
        } else {
            let art_r = Rect::from_min_size(pos2(x, rect.min.y + 8.0), vec2(42.0, 42.0));
            let radius = if item.class.contains("musicArtist") || item.class.contains("person") { 21.0 } else { 8.0 };
            paint_art(ui, &mut self.art, art_r, item.art.as_deref(), seed_of(item), radius);
            x = art_r.max.x + 14.0;
        }
        let tw = rect.max.x - x - 110.0;
        paint_text(ui, pos2(x, rect.min.y + 11.0), &item.title, 14.5, TEXT(), tw);
        let mut sx = x;
        let in_merged = self.stack.last().is_some_and(|t| t.id.starts_with("ALL:") || t.id.starts_with("ARTIST:"));
        if (self.view == View::Search && self.stack.is_empty() || in_merged) && !item.source.is_empty() {
            sx += source_badge(ui, pos2(x, rect.min.y + 32.0), crate::sources::source_label(&item.source)) + 8.0;
        }
        paint_text(ui, pos2(sx, rect.min.y + 32.0), &item_subtitle(item), 12.5, TEXT_DIM(), tw - (sx - x));
        if hovered {
            if dots.hovered() {
                ui.painter().circle_filled(dots_r.center(), 15.0, SURFACE_HOVER());
            }
            paint_icon(ui.painter(), dots_r.shrink(7.0), Icon::Dots, TEXT_DIM());
        }
        if item.browsable() {
            paint_icon(ui.painter(), Rect::from_center_size(pos2(rect.max.x - 58.0, rect.center().y), vec2(14.0, 14.0)), Icon::Chevron, TEXT_FAINT());
        } else if item.duration > 0.0 {
            ui.painter().text(pos2(rect.max.x - 48.0, rect.center().y), egui::Align2::RIGHT_CENTER, fmt_time(item.duration), egui::FontId::proportional(12.5), TEXT_FAINT());
        }
        egui::Popup::menu(&dots).close_behavior(egui::PopupCloseBehavior::CloseOnClick).show(|ui| {
            ui.set_min_width(180.0);
            self.item_menu(ui, item, gid);
        });
        if resp.clicked() {
            if item.browsable() {
                self.open(item);
            } else {
                self.play(gid, item, PlayHow::Now);
            }
        }
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        resp.context_menu(|ui| self.item_menu(ui, item, gid));
    }

    fn loading_or<F: FnOnce(&mut Self, &mut Ui, &Vec<Item>)>(&mut self, ui: &mut Ui, b: Option<&Browse>, empty: &str, f: F) {
        match b {
            None | Some(Browse::Loading) => {
                ui.add_space(20.0);
                spinner_dots(ui, TEXT_DIM());
            }
            Some(Browse::Failed(e)) => {
                ui.label(RichText::new(t!("Couldn't load this right now. Check the internet connection and try again.")).color(BAD())).on_hover_text(e);
            }
            Some(Browse::Ready(items)) if items.is_empty() => {
                ui.add_space(10.0);
                ui.label(RichText::new(empty).color(TEXT_DIM()));
            }
            Some(Browse::Ready(items)) => f(self, ui, items),
        }
    }

    // ---------- Browsing ----------

    fn grid_page(&mut self, ui: &mut Ui, s: &AppState, title: &str, sub: &str, id: &str) {
        self.header(ui, title, sub);
        if id == "SQ:" {
            ui.horizontal(|ui| {
                if pill(ui, Some(Icon::Plus), t!("Import playlist"), false)
                    .on_hover_text(t!("Add a playlist from an M3U, PLS, XSPF or CSV file, or a file saved with Export playlist or Save backup"))
                    .clicked()
                {
                    if let Some(path) =
                        rfd::FileDialog::new().set_title(t!("Import playlist")).add_filter("Playlists and backups", &["m3u8", "m3u", "pls", "xspf", "csv", "json"]).pick_file()
                    {
                        self.backup_sections = Default::default();
                        self.send(Cmd::ReadBackup(path));
                        // The summary, with its Restore button, is in Settings › Backup and restore.
                        self.go(View::Settings);
                    }
                }
            });
            ui.add_space(8.0);
        }
        let gid = s.selected_group.clone();
        if id == "FV:2" {
            egui::CollapsingHeader::new(RichText::new(t!("What can be a favorite?")).size(13.5).color(ACCENT())).id_salt("fav_help").show(ui, |ui| {
                ui.label(RichText::new(t!("Favorites live on your speakers, so every controller and room shares them. Use ⋯ or right-click on any of these and choose Add to Favorites:")).size(12.5).color(TEXT_DIM()));
                ui.add_space(4.0);
                for (what, from) in [
                    (t!("Radio stations"), t!("Radio directory, My stations, and stations saved on your system")),
                    (t!("Albums, artists and songs"), t!("Your system's music library, and connected streaming services")),
                    (t!("Playlists"), t!("Playlists saved on your system and from connected services")),
                    (t!("Podcast episodes"), t!("Any episode")),
                    (t!("From this computer"), t!("Songs, albums, artists and folders (songs play while this app is open)")),
                    (t!("Podcast shows and Internet Archive recordings"), t!("Whole shows and recordings, as well as single episodes and tracks")),
                    (t!("What's playing"), t!("The star in the player bar, the Now Playing page and the Home card")),
                ] {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(what).size(12.5).color(TEXT()).strong());
                        ui.label(RichText::new(from).size(12.5).color(TEXT_DIM()));
                    });
                }
                ui.add_space(2.0);
                ui.label(RichText::new(t!("Things the speakers can't store themselves (albums, artists and folders from this computer, podcast shows, archive recordings) are kept as favorites in this app. Not possible: Line-In/TV and the queue itself (save it as a playlist instead).")).size(12.0).color(TEXT_FAINT()));
            });
            ui.add_space(8.0);
        }
        let b = self.browse(s, id);
        let empty = if id == "FV:2" {
            t!("No favorites yet. Add stations, albums, playlists or songs from anywhere in the app with ⋯ › Add to Favorites.")
        } else {
            t!("No playlists yet. Save the queue as a playlist to create one.")
        };
        let kept = s.config.favorites.clone();
        if id == "FV:2" && !kept.is_empty() {
            match b {
                Some(Browse::Ready(v)) if v.is_empty() => {}
                _ => {
                    section_title(ui, t!("On your speakers"));
                    ui.label(RichText::new(t!("Shared with every room and controller.")).size(12.0).color(TEXT_FAINT()));
                    ui.add_space(4.0);
                }
            }
        }
        if id == "FV:2" && !kept.is_empty() && matches!(b, Some(Browse::Ready(v)) if v.is_empty()) {
            // Nothing on the speakers yet; just show the app's own.
        } else {
            self.loading_or(ui, b, empty, |me, ui, items| me.tile_grid(ui, items, gid.as_deref(), id));
        }
        if id == "FV:2" && !kept.is_empty() {
            ui.add_space(12.0);
            section_title(ui, t!("Kept in this app"));
            ui.label(
                RichText::new(t!("Albums, artists and folders from this computer, podcast shows and archive recordings can't be stored on the speakers, so they're kept here."))
                    .size(12.0)
                    .color(TEXT_FAINT()),
            );
            ui.add_space(6.0);
            self.tile_grid(ui, &kept, gid.as_deref(), "app_favs");
        }
    }

    fn library_root(&mut self, ui: &mut Ui, s: &AppState) {
        self.header(ui, t!("Music Library"), t!("Music shared on your speakers' system and folders on this computer"));
        ui.horizontal(|ui| {
            for (i, label) in [t!("Everything"), t!("On your system"), t!("On this computer"), t!("Streaming services")].iter().enumerate() {
                if filter_chip(ui, label, self.library_scope == i).clicked() {
                    self.library_scope = i;
                }
            }
        });
        ui.add_space(14.0);
        if self.library_scope == 0 {
            section_title(ui, t!("All your music"));
            ui.label(RichText::new(t!("Your system's library and the folders on this computer, together.")).size(12.5).color(TEXT_DIM()));
            ui.add_space(6.0);
            let cards: Vec<(String, String, String, Icon)> =
                MERGED.iter().map(|(id, t, _, _, icon)| (id.to_string(), crate::i18n::tr(t).to_string(), t!("System library and this computer").to_string(), *icon)).collect();
            self.category_cards(ui, &cards);
            ui.add_space(10.0);
        }
        if self.library_scope == 1 {
            section_title(ui, t!("On your system"));
            let cards: Vec<(String, String, String, Icon)> = LIBRARY_ROOT
                .iter()
                .map(|(id, t, sub)| {
                    let icon = match *id {
                        "A:ARTIST" | "A:ALBUMARTIST" | "A:COMPOSER" => Icon::Rooms,
                        "A:ALBUM" => Icon::Library,
                        "A:TRACKS" => Icon::Queue,
                        "A:PLAYLISTS" => Icon::Playlist,
                        "S:" => Icon::Folder,
                        _ => Icon::Sliders,
                    };
                    (id.to_string(), crate::i18n::tr(t).to_string(), crate::i18n::tr(sub).to_string(), icon)
                })
                .collect();
            self.category_cards(ui, &cards);
            ui.add_space(10.0);
        }
        if self.library_scope == 0 {
            section_title(ui, t!("More from your system"));
            let cards: Vec<(String, String, String, Icon)> = LIBRARY_ROOT
                .iter()
                .filter(|(id, ..)| matches!(*id, "A:ALBUMARTIST" | "A:COMPOSER" | "A:PLAYLISTS"))
                .map(|(id, t, sub)| {
                    (id.to_string(), crate::i18n::tr(t).to_string(), crate::i18n::tr(sub).to_string(), if *id == "A:PLAYLISTS" { Icon::Playlist } else { Icon::Rooms })
                })
                .collect();
            self.category_cards(ui, &cards);
            ui.add_space(10.0);
        }
        if self.library_scope == 0 || self.library_scope == 3 {
            self.service_cards(ui, s);
        }
        if self.library_scope == 0 || self.library_scope == 2 {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("On this computer")).size(19.0).color(TEXT()).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, Some(Icon::Plus), t!("Add folder"), true).clicked() {
                        self.pick_folder();
                    }
                    if !s.config.folders.is_empty() && pill(ui, Some(Icon::Refresh), t!("Rescan"), false).clicked() {
                        self.send(Cmd::Rescan);
                    }
                });
            });
            ui.add_space(4.0);
            if !s.local_status.is_empty() {
                ui.label(RichText::new(&s.local_status).size(12.5).color(TEXT_DIM()));
                ui.add_space(6.0);
            }
            if s.config.folders.is_empty() {
                card_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(t!("Add folders of music from this Mac, an external drive or a mounted network share.")).color(TEXT()));
                    ui.label(RichText::new(t!("Your speakers play them straight from this computer while the app is open.")).size(12.5).color(TEXT_DIM()));
                });
            } else {
                let cards: Vec<(String, String, String, Icon)> = vec![
                    ("LOCAL:ARTISTS".into(), t!("Artists").into(), t!("From your folders").into(), Icon::Rooms),
                    ("LOCAL:ALBUMS".into(), t!("Albums").into(), t!("From your folders").into(), Icon::Library),
                    ("LOCAL:TRACKS".into(), t!("Songs").into(), tn!(crate::sources::local::track_count(), "1 song", "{n} songs"), Icon::Queue),
                    ("LOCAL:GENRES".into(), t!("Genres").into(), t!("From your folders").into(), Icon::Sliders),
                    ("LOCAL:FOLDERS".into(), t!("Folders").into(), t!("{n} added", n = s.config.folders.len()), Icon::Folder),
                ];
                // In Everything these are already part of the combined lists above.
                if self.library_scope == 2 {
                    self.category_cards(ui, &cards);
                    ui.add_space(6.0);
                }
                self.folder_list(ui, s);
            }
        }
    }

    /// Connected streaming services, each opening the service's own home in the library.
    pub(super) fn open_service_favorites(&mut self, sid: u32, name: &str) {
        self.go(View::Library);
        self.library_scope = 3;
        self.open(&Item {
            id: crate::sources::favorites_id(sid),
            title: t!("{service} favorites", service = name),
            class: "object.container".into(),
            is_container: true,
            ..Default::default()
        });
    }

    fn service_cards(&mut self, ui: &mut Ui, s: &AppState) {
        section_title(ui, t!("Streaming services"));
        // Services that only play what's saved in Favorites get a card for those favorites.
        let favs: Vec<Item> = match self.browse(s, "FV:2") {
            Some(Browse::Ready(v)) => v.clone(),
            _ => vec![],
        };
        let fav_cards: Vec<(String, String, String, Icon)> = crate::sources::SERVICES
            .iter()
            .filter(|x| !x.note.is_empty() && s.config.account(x.sid).is_none())
            .map(|x| {
                let n = favs.iter().filter(|i| crate::sources::from_service(i, x.sid)).count();
                (crate::sources::favorites_id(x.sid), t!("{service} favorites", service = x.name), tn!(n, "1 saved favorite", "{n} saved favorites"), Icon::Star)
            })
            .collect();
        if s.config.accounts.is_empty() {
            card_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(t!("No streaming services connected yet.")).color(TEXT()));
                        ui.label(RichText::new(t!("Connect Spotify, Amazon Music, Deezer, TIDAL or Qobuz, then browse them here.")).size(12.5).color(TEXT_DIM()));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if pill(ui, Some(Icon::Services), t!("Music Services"), false).clicked() {
                            self.go(View::Services);
                        }
                    });
                });
            });
            ui.add_space(10.0);
            self.category_cards(ui, &fav_cards);
            ui.add_space(10.0);
            return;
        }
        let mut cards: Vec<(String, String, String, Icon)> =
            s.config.accounts.iter().map(|a| (format!("SVC:{}/", a.sid), a.name.clone(), t!("Browse playlists, albums, artists and more").to_string(), Icon::Services)).collect();
        cards.extend(fav_cards);
        self.category_cards(ui, &cards);
        ui.add_space(10.0);
    }

    /// Open the page that gathers everything by one artist from every source.
    pub(super) fn open_artist(&mut self, name: &str) {
        let first = name.split([';', '/']).next().unwrap_or(name).trim();
        if first.is_empty() {
            return;
        }
        self.open(&Item {
            id: format!("ARTIST:{first}"),
            title: first.to_string(),
            class: "object.container.person.musicArtist".into(),
            is_container: true,
            description: t!("Artist · everywhere").into(),
            ..Default::default()
        });
    }

    fn category_cards(&mut self, ui: &mut Ui, cards: &[(String, String, String, Icon)]) {
        let w = ui.available_width();
        let cols = if w > 600.0 { 2 } else { 1 };
        let cw = ((w - 14.0 * (cols as f32 - 1.0)) / cols as f32).floor() - 1.0;
        for chunk in cards.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                for (id, title, sub, icon) in chunk {
                    let (rect, resp) = ui.allocate_exact_size(vec2(cw, 68.0), Sense::click());
                    describe(&resp, egui::Role::Button, &format!("{title} — {sub}"));
                    focus_ring(ui, &resp, rect, 14.0);
                    ui.painter().rect_filled(rect, 14.0, if resp.hovered() { SURFACE_HI() } else { SURFACE() });
                    let badge = Rect::from_min_size(rect.min + vec2(14.0, 12.0), vec2(44.0, 44.0));
                    ui.painter().rect_filled(badge, 12.0, seed_color(title).gamma_multiply(0.35));
                    paint_icon(ui.painter(), badge.shrink(11.0), *icon, TEXT());
                    paint_text(ui, pos2(badge.max.x + 14.0, rect.min.y + 15.0), title, 15.0, TEXT(), cw - 120.0);
                    paint_text(ui, pos2(badge.max.x + 14.0, rect.min.y + 37.0), sub, 12.5, TEXT_DIM(), cw - 120.0);
                    paint_icon(ui.painter(), Rect::from_center_size(pos2(rect.max.x - 24.0, rect.center().y), vec2(14.0, 14.0)), Icon::Chevron, TEXT_FAINT());
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.open(&Item { id: id.clone(), title: title.clone(), class: "object.container".into(), is_container: true, ..Default::default() });
                    }
                }
            });
            ui.add_space(10.0);
        }
    }

    /// The folders the person has added, each with a remove button.
    fn folder_list(&mut self, ui: &mut Ui, s: &AppState) {
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(t!("Your folders")).size(13.0).color(TEXT_DIM()));
            for f in &s.config.folders {
                ui.horizontal(|ui| {
                    ui.set_height(34.0);
                    let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                    paint_icon(ui.painter(), r, Icon::Folder, ACCENT());
                    let exists = std::path::Path::new(f).exists();
                    ui.label(RichText::new(f).color(if exists { TEXT() } else { TEXT_FAINT() }));
                    if !exists {
                        chip(ui, t!("Not reachable"), BAD());
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, Icon::Close, 14.0, TEXT_DIM()).on_hover_text(t!("Remove folder")).clicked() {
                            self.send(Cmd::RemoveFolder(f.clone()));
                        }
                    });
                });
            }
        });
    }

    fn pick_folder(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title(t!("Add a music folder")).pick_folder() {
            self.send(Cmd::AddFolder(dir.to_string_lossy().to_string()));
        }
    }

    /// Text filter, sort and genre controls for a list; returns the filtered, sorted items.
    fn filter_bar(&mut self, ui: &mut Ui, key: &str, items: &[Item]) -> Vec<Item> {
        if self.list_key != key {
            self.list_key = key.to_string();
            self.list_filter.clear();
            self.list_sort = 0;
            self.list_genre = None;
        }
        let genres: std::collections::BTreeSet<String> = items.iter().map(|i| i.genre.clone()).filter(|g| !g.is_empty()).collect();
        let sorts: [&str; 7] = [t!("Default order"), t!("Title A–Z"), t!("Artist A–Z"), t!("Longest first"), t!("Title Z–A"), t!("Album A–Z"), t!("Shortest first")];
        if items.len() > 6 || !self.list_filter.is_empty() {
            ui.horizontal(|ui| {
                egui::Frame::new().fill(SURFACE_HI()).corner_radius(18.0).inner_margin(egui::Margin { left: 12, right: 12, top: 6, bottom: 6 }).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
                        paint_icon(ui.painter(), r, Icon::Filter, TEXT_DIM());
                        ui.add(egui::TextEdit::singleline(&mut self.list_filter).hint_text(t!("Filter this list")).frame(egui::Frame::NONE).desired_width(200.0));
                    });
                });
                egui::ComboBox::from_id_salt(("sort", key)).width(150.0).selected_text(sorts[self.list_sort]).show_ui(ui, |ui| {
                    for (i, l) in sorts.iter().enumerate() {
                        ui.selectable_value(&mut self.list_sort, i, *l);
                    }
                });
                if !genres.is_empty() {
                    let label = self.list_genre.clone().unwrap_or_else(|| t!("All genres").into());
                    egui::ComboBox::from_id_salt(("genre", key)).width(150.0).selected_text(label).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.list_genre, None, t!("All genres"));
                        for g in &genres {
                            ui.selectable_value(&mut self.list_genre, Some(g.clone()), g);
                        }
                    });
                }
            });
            ui.add_space(8.0);
        }
        let f = self.list_filter.to_lowercase();
        let mut out: Vec<Item> = items
            .iter()
            .filter(|i| f.is_empty() || i.title.to_lowercase().contains(&f) || i.artist.to_lowercase().contains(&f) || i.album.to_lowercase().contains(&f))
            .filter(|i| self.list_genre.as_ref().is_none_or(|g| &i.genre == g))
            .cloned()
            .collect();
        lists::sort_items(&mut out, self.list_sort);
        if out.len() != items.len() {
            ui.label(RichText::new(t!("Showing {shown} of {total}", shown = out.len(), total = items.len())).size(12.0).color(TEXT_FAINT()));
        }
        self.jump_strip(ui, &out);
        out
    }

    /// A–Z along the top of a long, alphabetical list: click a letter to scroll to it.
    fn jump_strip(&mut self, ui: &mut Ui, shown: &[Item]) {
        if shown.len() < lists::JUMP_MIN {
            return;
        }
        let sort = lists::effective_sort(shown, self.list_sort);
        let keys: Option<Vec<String>> = shown.iter().map(|i| lists::jump_key(i, sort)).collect();
        let Some(keys) = keys else { return };
        let targets = lists::jump_targets(&keys);
        if targets.len() < 3 {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (letter, at) in targets {
                let r = ui.add(egui::Button::new(RichText::new(letter.to_string()).size(12.5).color(TEXT_DIM())).frame(false).min_size(vec2(20.0, 20.0)));
                if r.on_hover_text(t!("Jump to {letter}", letter = letter)).clicked() {
                    self.jump_to = Some(at);
                }
            }
        });
        ui.add_space(6.0);
    }

    /// If a letter was clicked, scroll the page so that row `i` is at the top.
    fn apply_jump(&mut self, ui: &mut Ui, i: usize) {
        if self.jump_to == Some(i) {
            ui.scroll_to_rect(Rect::from_min_size(ui.cursor().min, vec2(1.0, 56.0)), Some(Align::TOP));
            self.jump_to = None;
        }
    }

    /// One list made from the system's library and this computer's folders, with a source filter.
    fn merged_list(&mut self, ui: &mut Ui, s: &AppState, id: &str, system_id: &str, local_id: &str) {
        let gid = s.selected_group.clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("Show")).size(12.0).color(TEXT_FAINT()));
            for (i, label) in [t!("Everything"), t!("System library"), t!("This computer")].iter().enumerate() {
                if filter_chip(ui, label, self.merged_source == i).clicked() {
                    self.merged_source = i;
                }
            }
        });
        ui.add_space(8.0);
        let mut items: Vec<Item> = vec![];
        let mut waiting = false;
        let mut notes: Vec<String> = vec![];
        if self.merged_source != 2 {
            match self.browse(s, system_id) {
                Some(Browse::Ready(v)) => items.extend(v.iter().cloned().map(|mut i| {
                    if i.source.is_empty() {
                        i.source = "library".into();
                    }
                    i
                })),
                Some(Browse::Failed(_)) => notes.push(t!("Your system's library couldn't be read right now.").into()),
                _ => waiting = true,
            }
        }
        if self.merged_source != 1 {
            if s.config.folders.is_empty() {
                notes.push(t!("No folders from this computer yet. Add one in Music Library.").into());
            } else {
                match self.browse(s, local_id) {
                    Some(Browse::Ready(v)) => items.extend(v.iter().cloned()),
                    Some(Browse::Failed(_)) => notes.push(t!("This computer's folders couldn't be read.").into()),
                    _ => waiting = true,
                }
            }
        }
        for n in &notes {
            ui.label(RichText::new(n).size(12.0).color(TEXT_FAINT()));
        }
        if waiting {
            spinner_dots(ui, TEXT_DIM());
        }
        if items.is_empty() {
            if !waiting {
                ui.label(RichText::new(t!("Nothing here yet.")).color(TEXT_DIM()));
            }
            return;
        }
        // One alphabetical list across both places (songs keep artist order).
        if id != "ALL:TRACKS" {
            items.sort_by_key(|i| i.title.to_lowercase());
        } else {
            items.sort_by(|a, b| {
                (a.artist.to_lowercase(), a.album.to_lowercase(), a.title.to_lowercase()).cmp(&(b.artist.to_lowercase(), b.album.to_lowercase(), b.title.to_lowercase()))
            });
        }
        let shown = self.filter_bar(ui, id, &items);
        let all_albums = shown.len() > 3 && shown.iter().all(|i| i.class.contains("musicAlbum"));
        if all_albums {
            self.tile_grid(ui, &shown, gid.as_deref(), id);
            return;
        }
        for (i, it) in shown.iter().take(3000).enumerate() {
            self.apply_jump(ui, i);
            self.row(ui, it, gid.as_deref(), (id, i), None);
        }
    }

    fn container_page(&mut self, ui: &mut Ui, s: &AppState, top: &Item) {
        let gid = s.selected_group.clone();
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Back, 18.0, TEXT()).clicked() {
                self.stack.pop();
            }
            let crumbs: Vec<String> = self.stack.iter().map(|i| i.title.clone()).collect();
            ui.label(RichText::new(crumbs.join("  ›  ")).size(13.0).color(TEXT_DIM()));
        });
        ui.add_space(10.0);
        let hero = !top.uri.is_empty()
            || ["musicAlbum", "musicArtist", "person", "playlist"].iter().any(|c| top.class.contains(c)) && top.id.contains('/')
            || ["POD:FEED/", "LOCAL:FOLDER/", "IA:ITEM/", "ARTIST:"].iter().any(|p| top.id.starts_with(p));
        if hero {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(180.0, 180.0), Sense::hover());
                paint_art(ui, &mut self.art, r, top.art.as_deref(), seed_of(top), 16.0);
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.add_space(22.0);
                    let kind = if top.id.starts_with("POD:") {
                        t!("Podcast")
                    } else if top.id.starts_with("LOCAL:FOLDER/") {
                        t!("Folder")
                    } else {
                        kind_label(&top.class)
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(kind.to_uppercase()).size(11.5).color(ACCENT()));
                        if !top.source.is_empty() {
                            chip(ui, crate::sources::source_label(&top.source), TEXT_DIM());
                        }
                    });
                    ui.label(RichText::new(&top.title).size(30.0).strong().color(TEXT()));
                    if !top.artist.is_empty() {
                        let link = ui
                            .add(egui::Label::new(RichText::new(&top.artist).size(15.0).color(TEXT_DIM())).sense(Sense::click()))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(t!("See everything by {artist}", artist = top.artist));
                        if link.hovered() {
                            let r = link.rect;
                            ui.painter().line_segment([pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)], egui::Stroke::new(1.0_f32, TEXT_DIM()));
                        }
                        if link.clicked() {
                            self.open_artist(&top.artist);
                        }
                    }
                    ui.add_space(12.0);
                    ui.horizontal_wrapped(|ui| {
                        if pill(ui, Some(Icon::Play), t!("Play"), true).clicked() {
                            self.play(gid.as_deref(), top, PlayHow::Replace);
                        }
                        if pill(ui, Some(Icon::Shuffle), t!("Shuffle"), false).clicked() {
                            if let Some(g) = gid.as_deref() {
                                self.send(Cmd::SetPlayMode(g.to_string(), true, Repeat::Off));
                                self.play(Some(g), top, PlayHow::Replace);
                            }
                        }
                        if pill(ui, Some(Icon::Plus), t!("Add to queue"), false).clicked() {
                            self.play(gid.as_deref(), top, PlayHow::End);
                        }
                        let saved = self.favorite_of(top).is_some();
                        if (saved || top.can_be_favorite())
                            && pill(ui, Some(Icon::Star), if saved { t!("In Favorites") } else { t!("Favorite") }, false)
                                .on_hover_text(if saved { t!("Remove from Favorites") } else { t!("Add to Favorites") })
                                .clicked()
                        {
                            self.toggle_favorite(top);
                        }
                        if super::is_saved_playlist(top)
                            && pill(ui, None, t!("Export playlist…"), false)
                                .on_hover_text(t!("Choose the file type when you pick the name: M3U, PLS or XSPF playlist, CSV spreadsheet or Roomtone backup (.json)"))
                                .clicked()
                        {
                            self.export_playlist(top);
                        }
                        if let Some(p) = crate::sources::podcasts::podcast_of(top) {
                            let subscribed = s.config.podcasts.iter().any(|x| x.feed == p.feed);
                            let label = if subscribed { t!("Subscribed") } else { t!("Subscribe") };
                            if pill(ui, Some(if subscribed { Icon::Check } else { Icon::Plus }), label, false).clicked() {
                                self.send(if subscribed { Cmd::Unsubscribe(p.feed) } else { Cmd::Subscribe(p) });
                            }
                        }
                    });
                });
            });
            ui.add_space(16.0);
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&top.title).size(30.0).strong().color(TEXT()));
                if top.id == "S:" || top.id == "LOCAL:FOLDERS" || top.id == "ALL:FOLDERS" {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if pill(ui, Some(Icon::Plus), t!("Add folder"), true).clicked() {
                            self.pick_folder();
                        }
                    });
                }
            });
            ui.add_space(12.0);
        }
        let id = top.id.clone();
        let is_folders = id == "S:" || id == "LOCAL:FOLDERS";
        // Folders shared on the system and folders on this computer sit side by side.
        let extra: Vec<Item> = if id == "S:" { crate::sources::local::browse("LOCAL:FOLDERS", &s.config.folders) } else { vec![] };
        let numbered = top.class.contains("musicAlbum");
        if let Some((_, _, system_id, local_id, _)) = MERGED.iter().find(|m| m.0 == id) {
            self.merged_list(ui, s, &id, system_id, local_id);
            return;
        }
        let items: Option<Vec<Item>> = match self.browse(s, &id) {
            Some(Browse::Ready(v)) => Some(v.iter().cloned().chain(extra.iter().cloned()).collect()),
            Some(Browse::Failed(e)) if !extra.is_empty() => {
                ui.label(RichText::new(t!("System folders unavailable: {error}", error = e)).size(12.5).color(TEXT_FAINT()));
                Some(extra.clone())
            }
            other => {
                let other = other.cloned();
                self.loading_or(ui, other.as_ref(), t!("Nothing here yet."), |_, _, _| {});
                None
            }
        };
        let Some(items) = items else { return };
        if items.is_empty() {
            if let Some(sid) = top.id.strip_prefix("FVSVC:").and_then(|x| x.parse::<u32>().ok()) {
                let name = crate::sources::SERVICES.iter().find(|x| x.sid == sid).map(|x| x.name).unwrap_or("");
                ui.label(RichText::new(t!("No {service} favorites yet.", service = name)).color(TEXT()));
                ui.label(
                    RichText::new(t!("In the speaker maker's app, save {service} albums, playlists, songs or stations to Favorites (or press ☆ in the player here while one is playing). They show up here, in Favorites and in Search, and play from this app.", service = name))
                        .size(12.5)
                        .color(TEXT_DIM()),
                );
                return;
            }
            let msg = if is_folders { t!("No folders yet. Use Add folder to choose music on this computer.") } else { t!("Nothing here yet.") };
            ui.label(RichText::new(msg).color(TEXT_DIM()));
            return;
        }
        let shown = self.filter_bar(ui, &id, &items);
        let all_albums = shown.len() > 3 && shown.iter().all(|i| i.class.contains("musicAlbum"));
        if all_albums {
            self.tile_grid(ui, &shown, gid.as_deref(), &id);
            return;
        }
        for (i, it) in shown.iter().enumerate() {
            self.apply_jump(ui, i);
            self.row(ui, it, gid.as_deref(), (&id, i), (numbered && self.list_sort == 0).then_some(i + 1));
            if is_folders && it.id.starts_with("LOCAL:FOLDER/") {
                let path = it.id.trim_start_matches("LOCAL:FOLDER/").to_string();
                ui.horizontal(|ui| {
                    ui.add_space(66.0);
                    if ui.add(egui::Button::new(RichText::new(t!("Remove folder")).size(11.5).color(TEXT_DIM())).frame(false)).clicked() {
                        self.send(Cmd::RemoveFolder(path));
                    }
                });
            }
        }
    }

    fn search_page(&mut self, ui: &mut Ui, s: &AppState) {
        self.header(ui, t!("Search"), t!("Every music source you've set up, in one place"));
        let gid = s.selected_group.clone();
        let frame = egui::Frame::new().fill(SURFACE_HI()).corner_radius(22.0).inner_margin(egui::Margin { left: 16, right: 16, top: 9, bottom: 9 });
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                paint_icon(ui.painter(), r, Icon::Search, TEXT_DIM());
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(t!("Artists, albums, songs, stations, podcasts…"))
                        .frame(egui::Frame::NONE)
                        .font(egui::FontId::proportional(16.0))
                        .desired_width(f32::INFINITY),
                );
                if edit.changed() {
                    self.search_edit_at = Some(Instant::now());
                }
            });
        });
        let term = self.search.trim().to_string();
        if self.search_edit_at.is_some_and(|t| t.elapsed() > Duration::from_millis(450)) {
            self.search_edit_at = None;
            if term != self.searched {
                self.searched = term.clone();
                self.search_genre = None;
                if !term.is_empty() {
                    self.send(Cmd::Search(term.clone()));
                }
            }
        }
        if self.search_edit_at.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(120));
        }
        ui.add_space(12.0);

        // Filters: where to look and what kind of thing.
        let all_sources = crate::sources::all_search_sources(&s.config);
        let sources: Vec<&str> = all_sources.iter().map(String::as_str).filter(|src| crate::sources::enabled(&s.config, src)).collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t!("Sources")).size(12.0).color(TEXT_FAINT()));
            if filter_chip(ui, t!("All"), self.search_sources.is_empty()).clicked() {
                self.search_sources.clear();
            }
            for src in &sources {
                let on = self.search_sources.contains(*src);
                if filter_chip(ui, crate::sources::source_label(src), on).clicked() {
                    if on {
                        self.search_sources.remove(*src);
                    } else {
                        self.search_sources.insert(src.to_string());
                    }
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t!("Types")).size(12.0).color(TEXT_FAINT()));
            if filter_chip(ui, t!("All"), self.search_types.is_empty()).clicked() {
                self.search_types.clear();
            }
            for t in ITEM_TYPES.iter().filter(|t| **t != "Other" && **t != "Genres") {
                let on = self.search_types.contains(*t);
                if filter_chip(ui, crate::i18n::tr(t), on).clicked() {
                    if on {
                        self.search_types.remove(*t);
                    } else {
                        self.search_types.insert(t.to_string());
                    }
                }
            }
        });
        ui.add_space(10.0);

        if self.searched.is_empty() {
            section_title(ui, t!("Browse"));
            ui.horizontal_wrapped(|ui| {
                for (label, view) in [
                    (t!("Favorites"), View::Favorites),
                    (t!("Playlists"), View::Playlists),
                    (t!("Music Library"), View::Library),
                    (t!("Radio"), View::Radio),
                    (t!("Music Services"), View::Services),
                    (t!("Line-In & TV"), View::Inputs),
                ] {
                    if pill(ui, None, label, false).clicked() {
                        self.go(view);
                    }
                }
            });
            return;
        }

        let mut all: Vec<Item> = vec![];
        let mut waiting: Vec<&str> = vec![];
        let mut failed: Vec<(&str, String)> = vec![];
        for src in &sources {
            if !self.search_sources.is_empty() && !self.search_sources.contains(*src) {
                continue;
            }
            match s.browse.get(&format!("search:{src}:{}", self.searched)) {
                Some(Browse::Ready(v)) => all.extend(v.iter().cloned()),
                Some(Browse::Failed(e)) => failed.push((src, e.clone())),
                _ => waiting.push(src),
            }
        }
        let genres: std::collections::BTreeSet<String> = all.iter().map(|i| i.genre.clone()).filter(|g| !g.is_empty()).collect();
        let sorts: [&str; 4] = [t!("Best match"), t!("Title A–Z"), t!("Artist A–Z"), t!("Longest first")];
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("search_sort").width(160.0).selected_text(sorts[self.search_sort]).show_ui(ui, |ui| {
                for (i, l) in sorts.iter().enumerate() {
                    ui.selectable_value(&mut self.search_sort, i, *l);
                }
            });
            if !genres.is_empty() {
                let label = self.search_genre.clone().unwrap_or_else(|| t!("All genres").into());
                egui::ComboBox::from_id_salt("search_genre").width(170.0).selected_text(label).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.search_genre, None, t!("All genres"));
                    for g in genres.iter().take(80) {
                        ui.selectable_value(&mut self.search_genre, Some(g.clone()), g);
                    }
                });
            }
            if !waiting.is_empty() {
                spinner_dots(ui, TEXT_DIM());
                let names: Vec<&str> = waiting.iter().map(|w| crate::sources::source_label(w)).collect();
                ui.label(RichText::new(t!("Searching {sources}…", sources = names.join(", "))).size(12.0).color(TEXT_FAINT()));
            }
        });
        for (src, e) in &failed {
            ui.label(RichText::new(t!("{source} couldn't be reached right now.", source = crate::i18n::tr(crate::sources::source_label(src)))).size(12.0).color(TEXT_FAINT()))
                .on_hover_text(e);
        }
        ui.add_space(8.0);
        let term_l = self.searched.to_lowercase();
        let mut shown: Vec<Item> = all
            .into_iter()
            .filter(|i| self.search_types.is_empty() || self.search_types.contains(item_type(i)))
            .filter(|i| self.search_genre.as_ref().is_none_or(|g| &i.genre == g))
            .collect();
        if self.search_sort == 0 {
            // Best match: exact titles, then titles that start with the words, otherwise each source's own order.
            shown.sort_by_key(|i| {
                let t = i.title.to_lowercase();
                if t == term_l {
                    0
                } else if t.starts_with(&term_l) {
                    1
                } else {
                    2
                }
            });
        } else {
            lists::sort_items(&mut shown, self.search_sort);
        }
        if shown.is_empty() && waiting.is_empty() {
            ui.label(RichText::new(t!("Nothing matched “{query}” with these filters.", query = self.searched)).color(TEXT_DIM()));
            return;
        }
        for t in ITEM_TYPES {
            let list: Vec<&Item> = shown.iter().filter(|i| item_type(i) == *t).collect();
            if list.is_empty() {
                continue;
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(crate::i18n::tr(t)).size(19.0).color(TEXT()).strong());
                ui.label(RichText::new(list.len().to_string()).size(12.0).color(TEXT_FAINT()));
            });
            let limit = if self.search_types.len() == 1 { 200 } else { 8 };
            for (i, it) in list.iter().take(limit).enumerate() {
                self.row(ui, it, gid.as_deref(), (t, i), None);
            }
            if list.len() > limit {
                let label = t!("Show all {n} {kind}", n = list.len(), kind = crate::i18n::tr(t).to_lowercase());
                if ui.add(egui::Button::new(RichText::new(label).size(13.0).color(ACCENT())).frame(false)).clicked() {
                    self.search_types = [t.to_string()].into_iter().collect();
                }
            }
            ui.add_space(10.0);
        }
    }

    fn radio_page(&mut self, ui: &mut Ui, s: &AppState) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| self.header(ui, t!("Radio"), t!("Your stations, saved favorites and a free worldwide directory")));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                if pill(ui, Some(Icon::Plus), t!("Add station"), true).clicked() {
                    self.station_editor = Some(Default::default());
                }
            });
        });
        let gid = s.selected_group.clone();

        section_title(ui, t!("My stations"));
        if s.config.stations.is_empty() {
            ui.label(RichText::new(t!("Add any internet radio stream by its address, or save stations from the directory below.")).color(TEXT_DIM()));
        } else {
            let mine: Vec<Item> = s.config.stations.iter().map(crate::sources::radio::my_station_item).collect();
            self.tile_grid(ui, &mine, gid.as_deref(), "mine");
        }
        ui.add_space(12.0);

        let mut favs: Vec<Item> = vec![];
        for id in ["FV:2", "R:0/0"] {
            if let Some(Browse::Ready(items)) = self.browse(s, id) {
                for it in items.iter().filter(|i| i.is_stream()) {
                    if !favs.iter().any(|x| x.title == it.title) {
                        favs.push(it.clone());
                    }
                }
            }
        }
        if !favs.is_empty() {
            section_title(ui, t!("Saved on your system"));
            self.tile_row(ui, &favs, gid.as_deref(), "radio_favs");
            ui.add_space(12.0);
        }

        if !s.config.service_on("radio") {
            ui.label(RichText::new(t!("The radio directory is switched off in Music Services.")).color(TEXT_DIM()));
            return;
        }
        section_title(ui, t!("Radio directory"));
        ui.horizontal_wrapped(|ui| {
            if filter_chip(ui, t!("Most popular"), self.radio_tag == "TOP").clicked() {
                self.radio_tag = "TOP".into();
                self.radio_searched.clear();
            }
            for t in crate::sources::radio::TAGS {
                let mut label = t.to_string();
                label[..1].make_ascii_uppercase();
                if filter_chip(ui, &label, self.radio_tag == *t && self.radio_searched.is_empty()).clicked() {
                    self.radio_tag = t.to_string();
                    self.radio_searched.clear();
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            egui::Frame::new().fill(SURFACE_HI()).corner_radius(18.0).inner_margin(egui::Margin { left: 12, right: 12, top: 6, bottom: 6 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
                    paint_icon(ui.painter(), r, Icon::Search, TEXT_DIM());
                    let e = ui.add(egui::TextEdit::singleline(&mut self.radio_search).hint_text(t!("Find a station by name")).frame(egui::Frame::NONE).desired_width(260.0));
                    if e.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.radio_search.trim().is_empty() {
                        self.radio_searched = self.radio_search.trim().to_string();
                        self.out.push(Cmd::Search(self.radio_searched.clone()));
                    }
                });
            });
            ui.label(RichText::new(t!("Press Return to search")).size(12.0).color(TEXT_FAINT()));
        });
        ui.add_space(8.0);
        let key = if self.radio_searched.is_empty() {
            if self.radio_tag == "TOP" {
                "RADIO:TOP".to_string()
            } else {
                format!("RADIO:TAG/{}", self.radio_tag)
            }
        } else {
            format!("search:radio:{}", self.radio_searched)
        };
        let b = if self.radio_searched.is_empty() { self.browse(s, &key) } else { s.browse.get(&key) };
        self.loading_or(ui, b, t!("No stations found."), |me, ui, items| {
            let shown = me.filter_bar(ui, &key, items);
            for (i, it) in shown.iter().take(120).enumerate() {
                me.apply_jump(ui, i);
                me.row(ui, it, gid.as_deref(), ("radio_dir", i), None);
            }
        });
    }

    fn services_page(&mut self, ui: &mut Ui, s: &AppState) {
        self.header(ui, t!("Music Services"), t!("Free sources work straight away. Streaming services: sign in once, then browse, search and play them here."));
        section_title(ui, t!("Available now"));
        for svc in crate::sources::SERVICES.iter().filter(|x| x.ready) {
            card_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    service_badge(ui, svc.name, true);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(crate::i18n::tr(svc.name)).size(15.0).color(TEXT()));
                        ui.label(RichText::new(crate::i18n::tr(svc.blurb)).size(12.5).color(TEXT_DIM()));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut on = s.config.service_on(svc.id);
                        if toggle(ui, &mut on, crate::i18n::tr(svc.name)).changed() {
                            self.send(Cmd::SetService(svc.id.to_string(), on));
                        }
                        if on && pill(ui, None, t!("Open"), false).clicked() {
                            match svc.id {
                                "radio" => self.go(View::Radio),
                                "local" => {
                                    self.go(View::Library);
                                    self.library_scope = 2;
                                }
                                "podcasts" => {
                                    self.go(View::Services);
                                    self.open(&Item {
                                        id: "POD:SUBS".into(),
                                        title: t!("Your podcasts").into(),
                                        class: "object.container".into(),
                                        is_container: true,
                                        ..Default::default()
                                    });
                                }
                                _ => {
                                    self.go(View::Services);
                                    self.open(&Item {
                                        id: "IA:FEATURED".into(),
                                        title: t!("Internet Archive").into(),
                                        class: "object.container".into(),
                                        is_container: true,
                                        ..Default::default()
                                    });
                                }
                            }
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
        ui.label(RichText::new(t!("Tip: find new podcasts and Internet Archive recordings with Search.")).size(12.0).color(TEXT_FAINT()));
        ui.add_space(12.0);
        section_title(ui, t!("Streaming services"));
        let live = s.phase == Phase::Live;
        ui.label(
            RichText::new(if live {
                t!("Sign in on each service's own web page. Your speakers play the music with the account already added to your system in the speaker maker's app, so add it there first if you haven't.")
            } else {
                t!("Connecting a streaming service needs your speakers on the network. Anything you've saved to Favorites from these services plays here today.")
            })
            .size(12.5)
            .color(TEXT_DIM()),
        );
        ui.add_space(8.0);
        let w = ui.available_width();
        let cw = ((w - 14.0) / 2.0).floor() - 1.0;
        let streaming: Vec<&crate::sources::Service> = crate::sources::SERVICES.iter().filter(|x| !x.ready).collect();
        for chunk in streaming.chunks(2) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                for svc in chunk {
                    let account = s.config.account(svc.sid).cloned();
                    let offered = s.music_services.iter().any(|m| m.sid == svc.sid);
                    ui.allocate_ui_with_layout(vec2(cw, 140.0), Layout::top_down(Align::Min), |ui| {
                        card_frame().show(ui, |ui| {
                            ui.set_width(cw - 32.0);
                            ui.set_max_width(cw - 32.0);
                            ui.set_min_height(104.0);
                            ui.horizontal(|ui| {
                                service_badge(ui, svc.name, account.is_some());
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(crate::i18n::tr(svc.name)).size(15.0).color(TEXT()));
                                        if account.is_some() {
                                            chip(ui, t!("Connected"), GOOD());
                                        } else if !svc.note.is_empty() {
                                            chip(ui, t!("Favorites only"), TEXT_DIM());
                                        } else if live && !offered {
                                            chip(ui, t!("Not offered on this system"), TEXT_FAINT());
                                        } else {
                                            chip(ui, t!("Ready to connect"), ACCENT());
                                        }
                                    });
                                    ui.label(RichText::new(crate::i18n::tr(svc.blurb)).size(12.0).color(TEXT_DIM()));
                                });
                            });
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| match &account {
                                Some(a) => {
                                    if pill(ui, None, t!("Open"), false).clicked() {
                                        self.go(View::Services);
                                        self.open(&Item {
                                            id: format!("SVC:{}/", a.sid),
                                            title: a.name.clone(),
                                            class: "object.container".into(),
                                            is_container: true,
                                            ..Default::default()
                                        });
                                    }
                                    let checking = s.service_checks.get(&a.sid).is_some_and(|r| r.is_empty());
                                    if pill(ui, Some(Icon::Refresh), if checking { t!("Checking…") } else { t!("Check") }, false)
                                        .on_hover_text(t!("Tries browsing, searching and finding the right account, without playing anything"))
                                        .clicked()
                                        && !checking
                                        && live
                                    {
                                        self.send(Cmd::CheckService(a.sid));
                                    }
                                    if pill(ui, None, t!("Disconnect"), false).clicked() {
                                        self.send(Cmd::DisconnectService(a.sid));
                                    }
                                    let mut sn = a.sn.max(1);
                                    ui.label(RichText::new(t!("Account")).size(11.5).color(TEXT_FAINT()))
                                        .on_hover_text(t!("Which of your system's accounts for this service plays the music. Change it if songs won't start."));
                                    if ui.add(egui::DragValue::new(&mut sn).range(1..=20)).changed() {
                                        self.send(Cmd::SetAccountNumber(a.sid, sn));
                                    }
                                }
                                None if !svc.note.is_empty() => {
                                    let count = match self.browse(s, "FV:2") {
                                        Some(Browse::Ready(v)) => v.iter().filter(|i| crate::sources::from_service(i, svc.sid)).count(),
                                        _ => 0,
                                    };
                                    let label = tn!(count, "Your favorites (1)", "Your favorites ({n})");
                                    if pill(ui, Some(Icon::Star), &label, false).clicked() {
                                        self.open_service_favorites(svc.sid, svc.name);
                                    }
                                }
                                None => {
                                    let enabled = live && offered;
                                    let r = ui.add_enabled_ui(enabled, |ui| pill(ui, Some(Icon::Plus), t!("Connect"), true)).inner;
                                    if r.clicked() && enabled {
                                        self.link_dismissed = false;
                                        self.send(Cmd::ConnectService(svc.sid));
                                    }
                                }
                            });
                            if let Some(result) = account.as_ref().and_then(|a| s.service_checks.get(&a.sid)).filter(|r| !r.is_empty()) {
                                let failed = result.starts_with('!');
                                ui.add(egui::Label::new(RichText::new(result.trim_start_matches('!')).size(11.5).color(if failed { BAD() } else { GOOD() })).wrap());
                            }
                            if account.is_none() && !svc.note.is_empty() {
                                ui.add(egui::Label::new(RichText::new(crate::i18n::tr(svc.note)).size(11.5).color(TEXT_FAINT())).wrap());
                            }
                        });
                    });
                }
            });
            ui.add_space(10.0);
        }
    }

    fn inputs_page(&mut self, ui: &mut Ui, s: &AppState) {
        let Some(h) = s.household() else { return };
        let Some(gid) = s.selected_group.clone() else { return };
        let target = s.find_group(&gid).map(|(h, g)| h.group_name(g)).unwrap_or_default();
        self.header(ui, t!("Line-In & TV"), &t!("Play a connected source in {room}", room = target));
        let mut any = false;
        for r in &h.rooms {
            let mut sources = vec![];
            if r.has_line_in() {
                sources.push((Icon::Input, t!("Line-In"), false));
            }
            if r.has_tv() {
                sources.push((Icon::Tv, t!("TV"), true));
            }
            for (icon, label, tv) in sources {
                any = true;
                card_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (b, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                        ui.painter().rect_filled(b, 12.0, SURFACE_HI());
                        paint_icon(ui.painter(), b.shrink(11.0), icon, ACCENT());
                        ui.vertical(|ui| {
                            ui.label(RichText::new(format!("{} · {}", label, r.name)).size(15.0).color(TEXT()));
                            ui.label(RichText::new(if r.model.is_empty() { t!("Speaker").to_string() } else { r.model.clone() }).size(12.0).color(TEXT_DIM()));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if pill(ui, Some(Icon::Play), t!("Play"), false).clicked() {
                                self.send(if tv { Cmd::Tv(gid.clone(), r.uuid.clone()) } else { Cmd::LineIn(gid.clone(), r.uuid.clone()) });
                            }
                        });
                    });
                });
                ui.add_space(8.0);
            }
        }
        if !any {
            ui.label(RichText::new(t!("None of your speakers look like they have a line-in or TV connection.")).color(TEXT_DIM()));
        }
        ui.add_space(6.0);
        ui.label(RichText::new(t!("Sources are guessed from each speaker's model; a speaker without that socket will refuse to play it.")).size(12.0).color(TEXT_FAINT()));
    }

    // ---------- Now playing ----------

    fn now_playing_page(&mut self, ui: &mut Ui, s: &AppState) {
        let Some((h, g)) = s.selected_group.as_ref().and_then(|g| s.find_group(g)) else {
            self.header(ui, t!("Now Playing"), t!("Pick a room on the right"));
            return;
        };
        let gid = g.id.clone();
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Down, 18.0, TEXT()).clicked() {
                self.go(View::Home);
            }
            ui.label(RichText::new(t!("Playing in {room}", room = h.group_name(g))).size(13.0).color(TEXT_DIM()));
            ui.add_space(6.0);
            chip(ui, &t!("{system} system", system = h.system.label()), system_tint(h.system));
            if !g.now.source.is_empty() {
                chip(ui, &g.now.source, TEXT_DIM());
            }
        });
        ui.add_space(12.0);
        let w = ui.available_width();
        let art = (w * 0.42).clamp(200.0, 340.0);
        ui.horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(art, art), Sense::hover());
            ui.painter().rect_filled(r.translate(vec2(0.0, 10.0)).shrink(10.0), 20.0, Color32::from_black_alpha(120));
            let seed = if g.now.album.is_empty() { g.now.title.clone() } else { g.now.album.clone() };
            if g.now.is_empty() {
                ui.painter().rect_filled(r, 20.0, SURFACE_HI());
            } else {
                paint_art(ui, &mut self.art, r, g.now.art.as_deref(), &seed, 20.0);
            }
            ui.add_space(22.0);
            ui.vertical(|ui| {
                let cw = ui.available_width();
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    clipped_label(ui, if g.now.is_empty() { t!("Nothing playing") } else { &g.now.title }, 28.0, TEXT(), cw - 44.0, true);
                    self.now_star(ui, &g.now, 20.0);
                });
                ui.add_space(2.0);
                let artist = clipped_label(ui, &g.now.artist, 17.0, TEXT_DIM(), cw, false);
                if !g.now.artist.is_empty() && !g.now.is_stream {
                    let artist = ui
                        .interact(artist.rect, ui.scope_id().with("np_artist"), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(t!("See everything by {artist}", artist = g.now.artist));
                    if artist.clicked() {
                        let name = g.now.artist.clone();
                        self.go(View::Library);
                        self.open_artist(&name);
                    }
                }
                clipped_label(ui, &g.now.album, 14.0, TEXT_FAINT(), cw, false);
                if !g.now.is_empty() {
                    self.quality_row(ui, &g.now);
                }
                ui.add_space(18.0);
                let dur = g.now.duration;
                let pos = g.now.live_position();
                if dur > 0.0 {
                    let mut v = self.seek_drag.unwrap_or(pos / dur);
                    let (r, changed) = slider(ui, "np_seek", t!("Playback position"), &mut v, cw, ACCENT());
                    if changed {
                        self.seek_drag = Some(v);
                    }
                    if r.drag_stopped() || r.clicked() {
                        if let Some(v) = self.seek_drag.take() {
                            self.send(Cmd::Seek(gid.clone(), v * dur));
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(fmt_time(self.seek_drag.map(|v| v * dur).unwrap_or(pos))).size(12.0).color(TEXT_FAINT()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(format!("-{}", fmt_time(dur - pos))).size(12.0).color(TEXT_FAINT()));
                        });
                    });
                } else if g.now.is_stream {
                    chip(ui, t!("LIVE"), ACCENT());
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    if icon_button(ui, Icon::Shuffle, 20.0, if g.shuffle { ACCENT() } else { TEXT_DIM() }).clicked() {
                        self.send(Cmd::SetPlayMode(gid.clone(), !g.shuffle, g.repeat));
                    }
                    if icon_button(ui, Icon::Prev, 24.0, TEXT()).clicked() {
                        self.send(Cmd::Previous(gid.clone()));
                    }
                    let playing = g.now.transport == Transport::Playing;
                    if round_button(ui, if playing { Icon::Pause } else { Icon::Play }, 62.0, ACCENT(), Color32::BLACK).clicked() {
                        self.send(if playing { Cmd::Pause(gid.clone()) } else { Cmd::Play(gid.clone()) });
                    }
                    if icon_button(ui, Icon::Next, 24.0, TEXT()).clicked() {
                        self.send(Cmd::Next(gid.clone()));
                    }
                    if g.episode.is_some() {
                        self.skip_buttons(ui, g);
                    }
                    let rep_icon = if g.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat };
                    if icon_button(ui, rep_icon, 20.0, if g.repeat != Repeat::Off { ACCENT() } else { TEXT_DIM() }).clicked() {
                        self.send(Cmd::SetPlayMode(gid.clone(), g.shuffle, g.repeat.next()));
                    }
                });
                ui.add_space(14.0);
                ui.horizontal_wrapped(|ui| {
                    let mut cf = g.crossfade;
                    ui.label(RichText::new(t!("Crossfade")).size(13.0).color(TEXT_DIM()));
                    if toggle(ui, &mut cf, t!("Crossfade")).changed() {
                        self.send(Cmd::SetCrossfade(gid.clone(), cf));
                    }
                    ui.add_space(12.0);
                    let sleep_on = g.sleep_left.is_some_and(|x| x > 0);
                    let sleep_label = match g.sleep_left {
                        Some(sec) if sec > 0 => t!("Sleep in {time}", time = fmt_time(sec as f32)),
                        _ => t!("Sleep timer").to_string(),
                    };
                    let resp = pill(ui, Some(Icon::Moon), &sleep_label, sleep_on);
                    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClick).show(|ui| {
                        ui.set_min_width(140.0);
                        for (label, mins) in [
                            (t!("Off"), None),
                            (t!("15 minutes"), Some(15)),
                            (t!("30 minutes"), Some(30)),
                            (t!("45 minutes"), Some(45)),
                            (t!("1 hour"), Some(60)),
                            (t!("2 hours"), Some(120)),
                        ] {
                            if ui.button(label).clicked() {
                                self.out.push(Cmd::SleepTimer(gid.clone(), mins));
                            }
                        }
                    });
                    if pill(ui, Some(Icon::Rooms), t!("Group"), false).clicked() {
                        self.group_editor = Some((gid.clone(), g.members.clone()));
                    }
                });
            });
        });
        ui.add_space(24.0);
        section_title(ui, t!("Volume"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            if g.members.len() > 1 {
                let (a, b) = (gid.clone(), gid.clone());
                self.volume_line(ui, t!("All rooms"), &format!("g:{gid}"), g.volume, g.muted, true, move |v| Cmd::GroupVolume(a.clone(), v), move |m| Cmd::GroupMute(b.clone(), m));
                ui.separator();
            }
            for m in &g.members {
                if let Some(r) = h.room(m) {
                    let (a, b) = (r.uuid.clone(), r.uuid.clone());
                    self.volume_line(
                        ui,
                        &r.name,
                        &format!("r:{}", r.uuid),
                        r.volume,
                        r.muted,
                        false,
                        move |v| Cmd::RoomVolume(a.clone(), v),
                        move |mu| Cmd::RoomMute(b.clone(), mu),
                    );
                }
            }
        });
        if let Some(q) = &g.queue {
            let next: Vec<(usize, &Item)> = q.iter().enumerate().skip(g.now.track_no as usize).take(5).collect();
            if !next.is_empty() && g.now.source == "Queue" {
                ui.add_space(18.0);
                section_title(ui, t!("Up next"));
                for (i, it) in next {
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 50.0), Sense::click());
                    describe(&resp, egui::Role::Button, &format!("{} — {}", it.title, it.artist));
                    focus_ring(ui, &resp, rect, 10.0);
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, 10.0, SURFACE());
                    }
                    let ar = Rect::from_min_size(rect.min + vec2(8.0, 7.0), vec2(36.0, 36.0));
                    paint_art(ui, &mut self.art, ar, it.art.as_deref(), seed_of(it), 7.0);
                    paint_text(ui, pos2(ar.max.x + 12.0, rect.min.y + 8.0), &it.title, 14.0, TEXT(), w - 140.0);
                    paint_text(ui, pos2(ar.max.x + 12.0, rect.min.y + 27.0), &it.artist, 12.0, TEXT_DIM(), w - 140.0);
                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() {
                        self.send(Cmd::PlayQueueIndex(gid.clone(), i));
                    }
                    resp.context_menu(|ui| self.favorite_menu_entry(ui, it));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn volume_line(&mut self, ui: &mut Ui, name: &str, key: &str, volume: u8, muted: bool, strong: bool, set: impl Fn(u8) -> Cmd, mute: impl Fn(bool) -> Cmd) {
        ui.horizontal(|ui| {
            ui.set_height(36.0);
            let label = RichText::new(name).size(14.0).color(if strong { TEXT() } else { TEXT_DIM() });
            ui.allocate_ui_with_layout(vec2(130.0, 20.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(130.0);
                ui.add(egui::Label::new(if strong { label.strong() } else { label }).truncate());
            });
            if icon_button(ui, if muted { Icon::Muted } else { Icon::Speaker }, 16.0, if muted { BAD() } else { TEXT_DIM() }).clicked() {
                self.send(mute(!muted));
            }
            let mut v = self.shown_volume(key, volume);
            let w = ui.available_width() - 40.0;
            let (_, changed) = slider(ui, key, name, &mut v, w, if strong { ACCENT() } else { TEXT() });
            if changed {
                self.local_vol.insert(key.to_string(), (v, Instant::now()));
                self.send(set((v * 100.0).round() as u8));
            }
            ui.label(RichText::new(format!("{}", (v * 100.0).round() as u8)).size(12.5).color(TEXT_DIM()));
        });
    }

    // ---------- Alarms ----------

    fn alarms_page(&mut self, ui: &mut Ui, s: &AppState) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| self.header(ui, t!("Alarms"), t!("Wake up to music in any room")));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                if pill(ui, Some(Icon::Plus), t!("New alarm"), true).clicked() {
                    let room = s.selected_group.clone().unwrap_or_default();
                    self.alarm_editor = Some(Alarm {
                        hour: 7,
                        minute: 0,
                        duration_min: 60,
                        recurrence: "DAILY".into(),
                        enabled: true,
                        room_uuid: room,
                        play_mode: "NORMAL".into(),
                        volume: 20,
                        program_name: n_!("Chime").into(),
                        ..Default::default()
                    });
                }
            });
        });
        let Some(list) = &s.alarms else {
            self.ask_once("alarms", Duration::from_secs(10), Cmd::LoadAlarms);
            spinner_dots(ui, TEXT_DIM());
            return;
        };
        if list.is_empty() {
            ui.label(RichText::new(t!("No alarms yet.")).color(TEXT_DIM()));
            return;
        }
        for a in list {
            card_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let col = if a.enabled { TEXT() } else { TEXT_FAINT() };
                    ui.label(RichText::new(format!("{:02}:{:02}", a.hour, a.minute)).size(34.0).color(col));
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        let room = s.find_room(&a.room_uuid).map(|(_, r)| r.name.clone()).unwrap_or_else(|| t!("Unknown room").into());
                        ui.label(RichText::new(format!("{} · {}", recurrence_label(&a.recurrence), room)).size(14.0).color(if a.enabled { TEXT() } else { TEXT_DIM() }));
                        ui.label(
                            RichText::new(t!(
                                "{program} · volume {volume} · {minutes} min",
                                program = crate::i18n::tr(&a.program_name),
                                volume = a.volume,
                                minutes = a.duration_min
                            ))
                            .size(12.5)
                            .color(TEXT_DIM()),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut on = a.enabled;
                        if toggle(ui, &mut on, t!("Alarm on")).changed() {
                            let mut a2 = a.clone();
                            a2.enabled = on;
                            self.send(Cmd::SaveAlarm(a2));
                        }
                        if icon_button(ui, Icon::Sliders, 16.0, TEXT_DIM()).on_hover_text(t!("Edit")).clicked() {
                            self.alarm_editor = Some(a.clone());
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
    }

    /// Returns false when the editor should close.
    pub(super) fn alarm_form(&mut self, ui: &mut Ui, s: &AppState, a: &mut Alarm) -> bool {
        let mut keep = true;
        ui.label(RichText::new(if a.id.is_empty() { t!("New alarm") } else { t!("Edit alarm") }).size(20.0).strong());
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add(egui::DragValue::new(&mut a.hour).range(0..=23).custom_formatter(|v, _| format!("{:02}", v as u32)).speed(0.1));
            ui.label(RichText::new(":").size(22.0));
            ui.add(egui::DragValue::new(&mut a.minute).range(0..=59).custom_formatter(|v, _| format!("{:02}", v as u32)).speed(0.2));
            ui.add_space(10.0);
            ui.label(RichText::new(t!("Drag or type to set the time")).size(12.0).color(TEXT_FAINT()));
        });
        ui.add_space(8.0);
        egui::Grid::new("alarm_grid").num_columns(2).spacing([16.0, 12.0]).show(ui, |ui| {
            ui.label(RichText::new(t!("Repeat")).color(TEXT_DIM()));
            egui::ComboBox::from_id_salt("alarm_rec").width(220.0).selected_text(recurrence_label(&a.recurrence)).show_ui(ui, |ui| {
                for r in ["DAILY", "WEEKDAYS", "WEEKENDS", "ONCE"] {
                    ui.selectable_value(&mut a.recurrence, r.to_string(), recurrence_label(r));
                }
            });
            ui.end_row();

            ui.label(RichText::new(t!("Room")).color(TEXT_DIM()));
            let room_name = s.find_room(&a.room_uuid).map(|(_, r)| r.name.clone()).unwrap_or_else(|| t!("Choose a room").into());
            egui::ComboBox::from_id_salt("alarm_room").width(220.0).selected_text(room_name).show_ui(ui, |ui| {
                if let Some(h) = s.household() {
                    for r in &h.rooms {
                        ui.selectable_value(&mut a.room_uuid, r.uuid.clone(), &r.name);
                    }
                }
            });
            ui.end_row();

            ui.label(RichText::new(t!("Music")).color(TEXT_DIM()));
            let favs: Vec<Item> = match s.browse.get("FV:2") {
                Some(Browse::Ready(f)) => f.clone(),
                _ => {
                    self.ask_once("browse:FV:2", Duration::from_secs(5), Cmd::Browse("FV:2".into()));
                    vec![]
                }
            };
            egui::ComboBox::from_id_salt("alarm_music").width(220.0).selected_text(a.program_name.clone()).show_ui(ui, |ui| {
                if ui.selectable_label(a.program_uri.is_empty() || a.program_uri.starts_with("x-rincon-buzzer"), t!("Chime")).clicked() {
                    a.program_uri = "x-rincon-buzzer:0".into();
                    a.program_meta.clear();
                    a.program_name = n_!("Chime").into();
                }
                for f in favs.iter().filter(|f| !f.uri.is_empty()) {
                    if ui.selectable_label(a.program_uri == f.uri, &f.title).clicked() {
                        a.program_uri = f.uri.clone();
                        a.program_meta = crate::net::didl::metadata_for(f);
                        a.program_name = f.title.clone();
                    }
                }
            });
            ui.end_row();

            ui.label(RichText::new(t!("Volume")).color(TEXT_DIM()));
            ui.horizontal(|ui| {
                let mut v = a.volume as f32 / 100.0;
                if slider(ui, "alarm_vol", t!("Volume"), &mut v, 180.0, ACCENT()).1 {
                    a.volume = (v * 100.0).round() as u8;
                }
                ui.label(RichText::new(a.volume.to_string()).color(TEXT_DIM()));
            });
            ui.end_row();

            ui.label(RichText::new(t!("Play for")).color(TEXT_DIM()));
            egui::ComboBox::from_id_salt("alarm_dur").width(220.0).selected_text(t!("{n} minutes", n = a.duration_min)).show_ui(ui, |ui| {
                for m in [15, 30, 45, 60, 90, 120, 180] {
                    ui.selectable_value(&mut a.duration_min, m, t!("{n} minutes", n = m));
                }
            });
            ui.end_row();

            ui.label(RichText::new(t!("Grouped rooms")).color(TEXT_DIM()));
            toggle(ui, &mut a.include_linked, t!("Grouped rooms"));
            ui.end_row();
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if !a.id.is_empty() && pill(ui, Some(Icon::Trash), t!("Delete"), false).clicked() {
                self.out.push(Cmd::DeleteAlarm(a.id.clone()));
                keep = false;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if pill(ui, None, t!("Save"), true).clicked() {
                    if a.room_uuid.is_empty() {
                        if let Some(r) = s.household().and_then(|h| h.rooms.first()) {
                            a.room_uuid = r.uuid.clone();
                        }
                    }
                    self.out.push(Cmd::SaveAlarm(a.clone()));
                    keep = false;
                }
                if pill(ui, None, t!("Cancel"), false).clicked() {
                    keep = false;
                }
            });
        });
        keep
    }

    // ---------- Settings ----------

    fn settings_page(&mut self, ui: &mut Ui, s: &AppState) {
        self.header(ui, t!("Settings"), t!("Systems, rooms and connection"));
        if let Some(problem) = &s.config_problem {
            let (title, text) = match problem {
                crate::config::Problem::SetAside { kept_as, .. } => (
                    t!("Your settings file couldn't be read"),
                    t!("It was kept as {file} in the settings folder, and Roomtone started with the defaults. Fix it or copy back what you need, then rename it to config.json to use it again.", file = kept_as),
                ),
                crate::config::Problem::NotSaved { error, .. } => (
                    t!("Your settings file couldn't be read"),
                    t!("It couldn't be set aside either ({error}), so changes won't be saved this time. Fix or move the file, then start Roomtone again.", error = error),
                ),
            };
            card_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(title).color(BAD()).strong());
                ui.label(RichText::new(text).size(12.5).color(TEXT_DIM()));
            });
            ui.add_space(14.0);
        }
        for (i, h) in s.households.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("{system} system", system = h.system.label())).size(19.0).color(TEXT()).strong());
                chip(ui, if h.system == System::S1 { t!("Legacy firmware 11.x") } else { t!("Current firmware") }, system_tint(h.system));
                if i == s.active {
                    chip(ui, t!("Active"), GOOD());
                }
            });
            card_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                for r in &h.rooms {
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 50.0), Sense::click());
                    describe(&resp, egui::Role::Button, &r.name);
                    focus_ring(ui, &resp, rect, 10.0);
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, 10.0, SURFACE_HI());
                    }
                    paint_icon(ui.painter(), Rect::from_min_size(rect.min + vec2(10.0, 15.0), vec2(20.0, 20.0)), Icon::Speaker, system_tint(h.system));
                    paint_text(ui, pos2(rect.min.x + 44.0, rect.min.y + 8.0), &r.name, 14.5, TEXT(), w - 200.0);
                    let sub = if r.version.is_empty() { r.model.clone() } else { format!("{} · {}", r.model, t!("version {version}", version = r.version)) };
                    paint_text(ui, pos2(rect.min.x + 44.0, rect.min.y + 28.0), &sub, 12.0, TEXT_DIM(), w - 200.0);
                    paint_icon(ui.painter(), Rect::from_center_size(pos2(rect.max.x - 18.0, rect.center().y), vec2(14.0, 14.0)), Icon::Chevron, TEXT_FAINT());
                    if !r.ip.is_empty() {
                        ui.painter().text(pos2(rect.max.x - 36.0, rect.center().y), egui::Align2::RIGHT_CENTER, &r.ip, egui::FontId::proportional(12.0), TEXT_FAINT());
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        self.view = View::Room(r.uuid.clone());
                    }
                }
            });
            ui.add_space(14.0);
        }

        self.updates_card(ui, s);
        ui.add_space(14.0);
        section_title(ui, t!("Connection"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(t!("Find speakers")).color(TEXT()));
                    ui.label(RichText::new(t!("Searches this network for S1 and S2 speakers.")).size(12.0).color(TEXT_DIM()));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, Some(Icon::Refresh), t!("Search again"), false).clicked() {
                        self.send(Cmd::Discover);
                    }
                });
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(RichText::new(t!("Add a speaker by address")).color(TEXT()));
            ui.label(RichText::new(t!("If your network blocks discovery, type a speaker's IP address (find it in your router).")).size(12.0).color(TEXT_DIM()));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.manual_ip).hint_text("192.168.1.20").desired_width(200.0).margin(vec2(10.0, 7.0)));
                if pill(ui, Some(Icon::Plus), t!("Add"), false).clicked() {
                    let ip = self.manual_ip.trim().to_string();
                    if ip.parse::<std::net::Ipv4Addr>().is_ok() {
                        self.send(Cmd::AddManual(ip));
                        self.manual_ip.clear();
                    }
                }
                if pill(ui, None, t!("Can't find your speakers?"), false).clicked() {
                    self.finder_open = true;
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(t!("Demo system")).color(TEXT()));
                    ui.label(RichText::new(t!("Explore every screen with made-up rooms and music.")).size(12.0).color(TEXT_DIM()));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Open demo"), false).clicked() {
                        self.send(Cmd::UseDemo);
                    }
                });
            });
        });
        ui.add_space(14.0);
        self.appearance_card(ui, s);
        ui.add_space(14.0);
        self.language_card(ui, s);
        ui.add_space(14.0);
        self.events_card(ui, s);
        ui.add_space(14.0);
        self.history_card(ui, s);
        ui.add_space(14.0);
        self.media_keys_card(ui, s);
        ui.add_space(14.0);
        self.new_version_card(ui, s);
        ui.add_space(14.0);
        self.backup_card(ui, s);
        ui.add_space(14.0);
        self.assistant_card(ui, s);
        ui.add_space(14.0);
        self.about_card(ui, s);
    }

    fn about_card(&mut self, ui: &mut Ui, s: &AppState) {
        section_title(ui, t!("About"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Name, version and platform.
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 20.0, ACCENT());
                ui.painter().circle_filled(r.center() + vec2(5.0, -5.0), 10.0, Color32::from_rgb(255, 196, 140));
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Roomtone").size(22.0).strong().color(TEXT()));
                        chip(ui, &t!("Version {version}", version = env!("CARGO_PKG_VERSION")), ACCENT());
                        chip(ui, &format!("{} · {}", crate::help::platform(), std::env::consts::ARCH), TEXT_DIM());
                    });
                    ui.label(RichText::new(t!("A modern controller for S1 and S2 speaker systems")).size(13.0).color(TEXT_DIM()));
                });
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(t!("Roomtone talks to your speakers directly on your home network, with no account of its own. S1 (firmware 11.x) and S2 systems each appear as their own system on the left: they can't share a group, but rooms can play along across them. Streaming services play through your system's own accounts."))
                    .size(12.5)
                    .color(TEXT_DIM()),
            );
            ui.add_space(12.0);

            // This computer and the systems it controls.
            let lang = crate::i18n::LANGUAGES.iter().find(|(c, _)| *c == crate::i18n::current()).map(|(_, n)| *n).unwrap_or("English");
            let lang = if s.config.language == "auto" { format!("{lang} ({})", t!("Automatic")) } else { lang.to_string() };
            let folder = crate::config::Config::data_dir();
            let music = crate::sources::local::base_url().unwrap_or_else(|| t!("Not started").to_string());
            let link = match s.mcp_status.strip_prefix("ok:") {
                Some(addr) if s.config.mcp.enabled => t!("On at {address}", address = addr),
                _ if s.config.mcp.enabled => t!("On").to_string(),
                _ => t!("Off").to_string(),
            };
            let link = if s.config.mcp.enabled && !s.config.mcp.allow_control { format!("{link} · {}", t!("look only")) } else { link };
            egui::Grid::new("about_grid").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                ui.label(RichText::new(t!("Systems")).color(TEXT_DIM()));
                ui.vertical(|ui| {
                    if s.households.is_empty() {
                        ui.label(RichText::new(t!("No speakers yet.")).color(TEXT()));
                    }
                    for h in &s.households {
                        let mut versions: Vec<&str> = h.rooms.iter().map(|r| r.version.as_str()).filter(|v| !v.is_empty()).collect();
                        versions.sort();
                        versions.dedup();
                        let software = match versions.as_slice() {
                            [] => String::new(),
                            [one] => format!(" · {}", t!("software {version}", version = one)),
                            [first, .., last] => format!(" · {}", t!("software {version}", version = format!("{first}–{last}"))),
                        };
                        let demo = if s.phase == Phase::Demo { format!(" · {}", t!("Demo")) } else { String::new() };
                        ui.label(RichText::new(format!("{} · {}{software}{demo}", t!("{system} system", system = h.system.label()), tn!(h.rooms.len(), "1 room", "{n} rooms"))).color(TEXT()));
                    }
                });
                ui.end_row();
                ui.label(RichText::new(t!("Language")).color(TEXT_DIM()));
                ui.label(RichText::new(lang).color(TEXT()));
                ui.end_row();
                ui.label(RichText::new(t!("Music from this computer")).color(TEXT_DIM()));
                ui.label(RichText::new(music).color(TEXT()));
                ui.end_row();
                ui.label(RichText::new(t!("Assistant link (MCP)")).color(TEXT_DIM()));
                ui.label(RichText::new(link).color(TEXT()));
                ui.end_row();
                ui.label(RichText::new(t!("Settings folder")).color(TEXT_DIM()));
                ui.horizontal(|ui| {
                    let path = folder.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                    let w = (ui.available_width() - 80.0).max(120.0);
                    ui.add_sized([w, 18.0], egui::Label::new(RichText::new(&path).size(12.5).color(TEXT())).truncate()).on_hover_text(&path);
                    if folder.is_some() && ui.add(egui::Button::new(RichText::new(t!("Show")).size(12.0)).corner_radius(10.0)).clicked() {
                        show_folder(&path);
                    }
                });
                ui.end_row();
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(RichText::new(t!("What's new in this version")).color(TEXT()).strong());
            for line in [
                t!("Your speakers shows whether each one is wired or wireless"),
                t!("Export a playlist from its own page"),
                t!("Install on a Mac with Homebrew"),
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("•").color(ACCENT()));
                    ui.label(RichText::new(line).size(12.5).color(TEXT_DIM()));
                });
            }

            ui.add_space(10.0);
            ui.label(RichText::new(t!("Help")).color(TEXT()).strong());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(t!("Open source: the code, issues and new versions are on GitHub.")).size(12.5).color(TEXT_DIM()));
            });
            ui.horizontal_wrapped(|ui| {
                if pill(ui, None, t!("Open project page"), false).on_hover_text(crate::help::REPO_URL).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(crate::help::REPO_URL));
                }
                if pill(ui, None, t!("Settings guide"), false).on_hover_text(t!("Every setting explained, and how to back them up")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(crate::help::settings_guide_url()));
                }
                if pill(ui, None, t!("Report a problem"), false).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("{}/issues/new/choose", crate::help::REPO_URL)));
                }
                if pill(ui, None, t!("Latest version"), false).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("{}/releases", crate::help::REPO_URL)));
                }
            });
            ui.add_space(4.0);
            let exe = crate::help::exe();
            let name = std::env::current_exe().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).unwrap_or_else(|| "roomtone".into());
            for (text, args) in [(t!("Every option, in a terminal:"), "--help"), (t!("Running without a window and connecting assistants:"), "--help mcp")] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(text).size(12.5).color(TEXT_DIM()));
                    ui.label(RichText::new(format!("{name} {args}")).monospace().size(12.0).color(TEXT()));
                    if ui.add(egui::Button::new(RichText::new(t!("Copy")).size(12.0)).corner_radius(10.0)).on_hover_text(format!("{exe} {args}")).clicked() {
                        ui.ctx().copy_text(format!("{exe} {args}"));
                        self.toast_here(t!("Copied"));
                    }
                });
            }

            ui.add_space(10.0);
            ui.label(RichText::new(t!("Thanks to")).color(TEXT()).strong());
            ui.label(
                RichText::new(t!("Free music directories: Radio Browser (radio-browser.info), the Apple Podcasts directory search and the Internet Archive (archive.org)."))
                    .size(12.5)
                    .color(TEXT_DIM()),
            );
            ui.label(
                RichText::new(t!("Built with Rust and the open-source libraries egui/eframe, winit, ureq, roxmltree, serde, image, lofty and rfd."))
                    .size(12.5)
                    .color(TEXT_DIM()),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t!("Roomtone is an independent app. It isn't made or endorsed by the speaker maker or by any music service; their names belong to their owners."))
                    .size(12.0)
                    .color(TEXT_FAINT()),
            );
            if cfg!(target_os = "macos") {
                ui.add_space(4.0);
                ui.label(RichText::new(t!("On macOS, allow Local Network access for this app in System Settings › Privacy & Security if no speakers appear.")).size(12.0).color(TEXT_FAINT()));
            }
        });
    }

    /// Software updates for each system: check, see what's offered, install after confirming.
    /// For an S1 system: which of its speakers could move to S2 (asked of the speakers, read-only).
    fn s2_rows(&mut self, ui: &mut Ui, s: &AppState, h: &Household) {
        use crate::s2::{summarize, Verdict};
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("Can this system move to S2?")).size(13.0).color(TEXT_DIM()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| match s.s2.get(&h.id) {
                Some(S2State::Checking) => spinner_dots(ui, TEXT_DIM()),
                _ => {
                    let label = if s.s2.contains_key(&h.id) { t!("Check again") } else { t!("Check my speakers") };
                    if pill(ui, None, label, false).on_hover_text(t!("Asks each speaker which model it is. Nothing is changed.")).clicked() {
                        self.send(Cmd::CheckS2(h.id.clone()));
                    }
                }
            });
        });
        match s.s2.get(&h.id) {
            Some(S2State::Done(rooms)) => {
                let sum = summarize(rooms);
                let (line, color) = if sum.cannot > 0 {
                    (t!("{n} of {total} speakers can't run S2. They would have to stay on S1 as a separate system.", n = sum.cannot, total = rooms.len()).to_string(), BAD())
                } else if sum.check > 0 {
                    (
                        t!(
                            "{n} of {total} speakers need checking: their model comes in a first generation that can't run S2 and a second that can.",
                            n = sum.check,
                            total = rooms.len()
                        )
                        .to_string(),
                        ACCENT(),
                    )
                } else {
                    (t!("Every speaker looks able to run S2.").to_string(), GOOD())
                };
                ui.label(RichText::new(line).size(12.5).color(color));
                ui.add_space(4.0);
                for r in rooms {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&r.room).size(12.5).color(TEXT()));
                        let model = if r.details.model_name.is_empty() { r.details.model_number.clone() } else { r.details.model_name.clone() };
                        ui.label(RichText::new(format!("{model} · {} · {}", r.details.model_number, r.details.hardware)).size(11.5).color(TEXT_FAINT()));
                        match r.verdict {
                            Verdict::AlreadyS2 => chip(ui, t!("Already on S2"), GOOD()),
                            Verdict::Can => chip(ui, t!("Can run S2"), GOOD()),
                            Verdict::Cannot => chip(ui, t!("Can't run S2"), BAD()),
                            Verdict::Check => chip(ui, t!("Check"), ACCENT()),
                        };
                    });
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new(t!("Based on Sonos's list of products that can't run S2. For a speaker marked Check, look at Sonos's S2 compatibility list for your product and its generation (the label under the speaker shows the model and a date)."))
                        .size(11.5)
                        .color(TEXT_FAINT()),
                );
            }
            Some(S2State::Failed(e)) => {
                ui.label(RichText::new(t!("Couldn't check: {error}", error = e)).size(12.5).color(BAD()));
            }
            _ => {}
        }
    }

    fn updates_card(&mut self, ui: &mut Ui, s: &AppState) {
        if s.households.is_empty() {
            return;
        }
        section_title(ui, t!("Software updates"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, h) in s.households.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                let mut versions: Vec<&str> = h.rooms.iter().map(|r| r.version.as_str()).filter(|v| !v.is_empty()).collect();
                versions.sort();
                versions.dedup();
                let state = s.updates.get(&h.id);
                ui.horizontal(|ui| {
                    ui.set_min_height(44.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t!("{system} system", system = h.system.label())).color(TEXT()));
                            if !versions.is_empty() {
                                chip(ui, &t!("software {version}", version = versions.join(", ")), system_tint(h.system));
                            }
                        });
                        let (text, color) = match state {
                            None => (t!("Not checked yet").to_string(), TEXT_DIM()),
                            Some(UpdateState::Checking) => (t!("Checking…").to_string(), TEXT_DIM()),
                            Some(UpdateState::UpToDate) => (t!("Up to date").to_string(), GOOD()),
                            Some(UpdateState::Available(o)) => (t!("Version {version} is available", version = o.version), ACCENT()),
                            Some(UpdateState::Installing(done, total)) => (t!("Sending the update to speaker {n} of {total}…", n = done + 1, total = total), ACCENT()),
                            Some(UpdateState::Started(v)) => (t!("Installing {version}: speakers restart one by one", version = v), ACCENT()),
                            Some(UpdateState::Failed(e)) => (t!("Couldn't check: {error}", error = e), BAD()),
                        };
                        ui.label(RichText::new(text).size(12.5).color(color));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| match state {
                        Some(UpdateState::Available(o)) => {
                            if pill(ui, None, t!("Update all speakers"), true).clicked() {
                                self.update_confirm = Some((h.id.clone(), h.system, o.version.clone()));
                            }
                        }
                        Some(UpdateState::Checking) | Some(UpdateState::Installing(..)) => spinner_dots(ui, TEXT_DIM()),
                        _ => {
                            if pill(ui, Some(Icon::Refresh), t!("Check for updates"), false).clicked() {
                                self.send(Cmd::CheckUpdates(h.id.clone()));
                            }
                        }
                    });
                });
                if h.system == System::S1 {
                    self.s2_rows(ui, s, h);
                }
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(t!("Updates come from the speakers' own update service. S1 systems only get S1 updates here; moving to S2 isn't possible from this app."))
                    .size(12.0)
                    .color(TEXT_FAINT()),
            );
        });
    }

    /// Save and restore favorites, playlists, alarms and Roomtone's own lists and settings.
    fn backup_card(&mut self, ui: &mut Ui, s: &AppState) {
        use crate::backup::Counts;
        let counts_line = |c: &Counts| -> String {
            t!(
                "{favorites} favorites · {playlists} playlists · {alarms} alarms · {app} of Roomtone's own",
                favorites = c.favorites,
                playlists = c.playlists,
                alarms = c.alarms,
                app = c.app
            )
            .to_string()
        };
        section_title(ui, t!("Backup and restore"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let row = |ui: &mut Ui, title: &str, sub: &str| {
                ui.vertical(|ui| {
                    ui.set_max_width((ui.available_width() - 200.0).max(160.0));
                    ui.label(RichText::new(title).color(TEXT()));
                    ui.label(RichText::new(sub).size(12.0).color(TEXT_DIM()));
                });
            };
            ui.horizontal(|ui| {
                ui.set_min_height(40.0);
                row(
                    ui,
                    t!("Save a backup"),
                    t!("Saves this system's favorites, playlists and alarms, and Roomtone's own lists and settings, to one file. Sign-ins and the access key are never included."),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, Some(Icon::Save), t!("Save backup"), true).clicked() {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                        let name = format!("roomtone-backup-{}.json", crate::backup::date_stamp(now));
                        if let Some(path) = rfd::FileDialog::new().set_title(t!("Save a backup")).set_file_name(name).add_filter("Roomtone backup", &["json"]).save_file() {
                            self.send(Cmd::SaveBackup(path, crate::backup::Sections::default()));
                        }
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.set_min_height(40.0);
                row(ui, t!("Restore from a backup"), t!("Adds what's in a backup file to this system. Nothing that's already there is changed or removed."));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Choose a backup"), false).clicked() {
                        if let Some(path) = rfd::FileDialog::new().set_title(t!("Choose a backup")).add_filter("Roomtone backup", &["json"]).pick_file() {
                            self.backup_sections = Default::default();
                            self.send(Cmd::ReadBackup(path));
                        }
                    }
                });
            });
            if let Some((path, sum)) = &s.backup_preview {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                let made = if sum.created > 0 { crate::backup::date_stamp(sum.created) } else { "?".into() };
                let from = if sum.generation.is_empty() { String::new() } else { format!(" · {}", t!("{system} system", system = sum.generation)) };
                ui.label(RichText::new(format!("{}{}", t!("Backup made on {date}", date = made), from)).color(TEXT()));
                ui.label(RichText::new(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).size(12.0).color(TEXT_DIM()));
                ui.add_space(6.0);
                let c = &sum.counts;
                let sec = &mut self.backup_sections;
                ui.checkbox(&mut sec.favorites, t!("Favorites ({n})", n = c.favorites));
                ui.checkbox(&mut sec.playlists, t!("Playlists ({n})", n = c.playlists));
                ui.checkbox(&mut sec.alarms, t!("Alarms ({n})", n = c.alarms));
                ui.checkbox(&mut sec.app, t!("Roomtone's own lists and settings ({n})", n = c.app));
                ui.add_space(4.0);
                ui.label(RichText::new(t!("Alarms are matched to rooms by name. Things already on this system are skipped.")).size(12.0).color(TEXT_FAINT()));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let sections = self.backup_sections;
                    if pill(ui, Some(Icon::Refresh), t!("Restore"), true).clicked() {
                        self.send(Cmd::RestoreBackup(path.clone(), sections));
                    }
                    if pill(ui, None, t!("Cancel"), false).clicked() {
                        self.send(Cmd::DismissBackup);
                    }
                });
            }
            match &s.backup_result {
                Some(BackupResult::Saved { file, counts, notes }) => {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.label(RichText::new(t!("Backup saved as {file}", file = file)).color(GOOD()));
                    ui.label(RichText::new(counts_line(counts)).size(12.5).color(TEXT_DIM()));
                    for n in notes {
                        ui.label(RichText::new(n).size(12.0).color(TEXT_FAINT()));
                    }
                }
                Some(BackupResult::Restored(r)) => {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    if r.added.total() == 0 && r.failed.total() == 0 {
                        ui.label(RichText::new(t!("Nothing new to add: everything was already there.")).color(TEXT()));
                    } else {
                        ui.label(RichText::new(t!("Added: {counts}", counts = counts_line(&r.added))).color(GOOD()));
                    }
                    if r.skipped.total() > 0 {
                        ui.label(RichText::new(t!("Already there, left alone: {counts}", counts = counts_line(&r.skipped))).size(12.5).color(TEXT_DIM()));
                    }
                    if r.failed.total() > 0 {
                        ui.label(RichText::new(t!("Couldn't restore: {counts}", counts = counts_line(&r.failed))).size(12.5).color(BAD()));
                    }
                    if !r.notes.is_empty() {
                        egui::CollapsingHeader::new(RichText::new(t!("Details")).size(12.5).color(TEXT_DIM())).id_salt("backup_notes").show(ui, |ui| {
                            for n in r.notes.iter().take(20) {
                                ui.label(RichText::new(n).size(12.0).color(TEXT_FAINT()));
                            }
                        });
                    }
                    if pill(ui, None, t!("Close"), false).clicked() {
                        self.send(Cmd::DismissBackup);
                    }
                }
                None => {}
            }
        });
    }

    /// The keyboard's media keys and the system's Now Playing.
    fn media_keys_card(&mut self, ui: &mut Ui, s: &AppState) {
        section_title(ui, t!("Media keys"));
        let mut on = s.config.media_keys;
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(40.0);
                ui.vertical(|ui| {
                    ui.set_max_width((ui.available_width() - 64.0).max(120.0));
                    ui.label(RichText::new(t!("Use the computer's media controls")).color(TEXT()));
                    ui.label(
                        RichText::new(t!("The keyboard's play, next and previous keys, headphone buttons, and the system's Now Playing (Control Center, the lock screen or the media overlay) control the room selected here."))
                            .size(12.0)
                            .color(TEXT_DIM()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle(ui, &mut on, t!("Use the computer's media controls"));
                });
            });
        });
        if on != s.config.media_keys {
            self.send(Cmd::SetMediaKeys(on));
        }
    }

    /// An optional once-a-day look for a newer Roomtone, and a *Check now* button.
    fn new_version_card(&mut self, ui: &mut Ui, s: &AppState) {
        use crate::version::Outcome;
        section_title(ui, t!("New versions"));
        let mut on = s.config.check_new_versions;
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(40.0);
                ui.vertical(|ui| {
                    ui.set_max_width((ui.available_width() - 64.0).max(120.0));
                    ui.label(RichText::new(t!("Tell me when a new version of Roomtone is out")).color(TEXT()));
                    ui.label(
                        RichText::new(t!(
                            "Once a day Roomtone looks at this project's release page on GitHub. Nothing is sent about you, and nothing is installed or downloaded for you."
                        ))
                        .size(12.0)
                        .color(TEXT_DIM()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle(ui, &mut on, t!("Tell me when a new version of Roomtone is out"));
                });
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let current = env!("CARGO_PKG_VERSION");
                if s.version_checking {
                    spinner_dots(ui, TEXT_DIM());
                    ui.label(RichText::new(t!("Looking…")).size(12.5).color(TEXT_DIM()));
                } else {
                    match &s.new_version {
                        Outcome::Newer(r) => {
                            ui.label(RichText::new(t!("Version {version} is available (you have {current})", version = r.version, current = current)).size(12.5).color(ACCENT()));
                            let url = if r.url.is_empty() { format!("{}/releases/latest", crate::help::REPO_URL) } else { r.url.clone() };
                            if pill(ui, None, t!("Open download page"), true).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                            }
                        }
                        Outcome::UpToDate => {
                            ui.label(RichText::new(t!("You have the latest version ({current})", current = current)).size(12.5).color(GOOD()));
                        }
                        Outcome::Failed(e) => {
                            ui.label(RichText::new(t!("Couldn't check: {error}", error = e)).size(12.5).color(BAD()));
                        }
                        Outcome::Unknown => {
                            ui.label(RichText::new(t!("You have version {current}", current = current)).size(12.5).color(TEXT_DIM()));
                        }
                    }
                    if pill(ui, Some(Icon::Refresh), t!("Check now"), false).clicked() {
                        self.send(Cmd::CheckVersionNow);
                    }
                }
            });
        });
        if on != s.config.check_new_versions {
            self.send(Cmd::SetVersionCheck(on));
        }
    }

    /// Switch listening history on or off, see it, or clear it.
    fn history_card(&mut self, ui: &mut Ui, s: &AppState) {
        section_title(ui, t!("Listening history"));
        let mut on = s.config.remember_history;
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(40.0);
                ui.vertical(|ui| {
                    ui.set_max_width((ui.available_width() - 64.0).max(120.0));
                    ui.label(RichText::new(t!("Remember what I've played")).color(TEXT()));
                    ui.label(
                        RichText::new(t!("Songs and stations you've played for a little while are kept on this computer, so Recently played and History work after a restart. Nothing is sent anywhere."))
                            .size(12.0)
                            .color(TEXT_DIM()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle(ui, &mut on, t!("Remember what I've played"));
                });
            });
            let n = crate::history::count();
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(tn!(n, "1 song remembered", "{n} songs remembered")).size(12.5).color(TEXT_DIM()));
                if n > 0 && pill(ui, None, t!("See history"), false).clicked() {
                    self.go(View::History);
                }
                if n > 0 && pill(ui, Some(Icon::Trash), t!("Clear history"), false).clicked() {
                    self.send(Cmd::ClearHistory);
                }
            });
        });
        if on != s.config.remember_history {
            self.send(Cmd::SetHistory(on));
        }
    }

    /// Everything that's been played, newest first, with how often and how long ago.
    fn history_page(&mut self, ui: &mut Ui, s: &AppState) {
        use crate::history::Ago;
        self.header(ui, t!("History"), t!("What you've played, kept on this computer"));
        let gid = s.selected_group.clone();
        let entries = crate::history::all();
        ui.horizontal(|ui| {
            if !s.config.remember_history {
                ui.label(RichText::new(t!("History is switched off in Settings, so nothing new is being added.")).size(12.5).color(TEXT_DIM()));
            }
            if !entries.is_empty() && pill(ui, Some(Icon::Trash), t!("Clear history"), false).clicked() {
                self.send(Cmd::ClearHistory);
            }
        });
        ui.add_space(8.0);
        if entries.is_empty() {
            ui.label(RichText::new(t!("Nothing yet. Songs and stations you play for a little while will show up here.")).color(TEXT_DIM()));
            return;
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        for (i, e) in entries.iter().take(300).enumerate() {
            let when = match crate::history::ago(e.last_played, now) {
                Ago::JustNow => t!("just now").to_string(),
                Ago::Minutes(n) => t!("{n} min ago", n = n).to_string(),
                Ago::Hours(n) => tn!(n, "1 hour ago", "{n} hours ago"),
                Ago::Yesterday => t!("yesterday").to_string(),
                Ago::Days(n) => t!("{n} days ago", n = n).to_string(),
                Ago::Date(d) => d,
            };
            let mut it = e.item.clone();
            let base = it.subtitle();
            let plays = if e.plays > 1 { format!(" · {}", tn!(e.plays, "1 play", "{n} plays")) } else { String::new() };
            it.description = format!("{base} · {when}{plays}");
            self.row(ui, &it, gid.as_deref(), ("history", i), None);
        }
    }

    /// Instant updates from the speakers (UPnP events) instead of asking again and again.
    fn events_card(&mut self, ui: &mut Ui, s: &AppState) {
        section_title(ui, t!("Speaker updates"));
        let mut on = s.config.speaker_events;
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(40.0);
                ui.vertical(|ui| {
                    // Leave room for the switch, so the explanation wraps instead of running under it.
                    ui.set_max_width((ui.available_width() - 64.0).max(120.0));
                    ui.label(RichText::new(t!("Instant updates from the speakers")).color(TEXT()));
                    ui.label(
                        RichText::new(t!("The speakers tell Roomtone when something changes, so changes made elsewhere show up at once and far less network traffic is needed. Needs this computer to accept connections from the speakers."))
                            .size(12.0)
                            .color(TEXT_DIM()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle(ui, &mut on, t!("Instant updates from the speakers"));
                });
            });
            if on {
                let e = &s.events;
                let (color, text) = if s.phase != Phase::Live {
                    (TEXT_DIM(), t!("Works with real speakers, not the demo system.").to_string())
                } else if e.working == 0 && e.silent == 0 && e.failed == 0 {
                    (TEXT_DIM(), t!("Starting…").to_string())
                } else if e.silent > 0 || (e.working == 0 && e.failed > 0) {
                    (BAD(), t!("Some speakers aren't reaching this computer. A firewall may be blocking incoming connections; Roomtone keeps asking them the usual way meanwhile.").to_string())
                } else if e.failed > 0 {
                    (TEXT_DIM(), tn!(e.working, "1 speaker is sending updates; the rest couldn't be reached yet.", "{n} speakers are sending updates; the rest couldn't be reached yet."))
                } else {
                    (GOOD(), tn!(e.working, "1 speaker is sending updates.", "{n} speakers are sending updates."))
                };
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, color);
                    ui.label(RichText::new(text).size(12.5).color(color));
                });
            }
        });
        if on != s.config.speaker_events {
            self.send(Cmd::SetEvents(on));
        }
    }

    fn appearance_card(&mut self, ui: &mut Ui, s: &AppState) {
        section_title(ui, t!("Appearance"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(t!("Light or dark")).color(TEXT()));
                    ui.label(RichText::new(t!("Automatic follows your computer's setting.")).size(12.0).color(TEXT_DIM()));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let codes = ["auto", "light", "dark"];
                    let current = codes.iter().position(|c| *c == s.config.theme).unwrap_or(0);
                    if let Some(i) = segmented(ui, &[t!("Automatic"), t!("Light"), t!("Dark")], current, 300.0) {
                        self.send(Cmd::SetTheme(codes[i].to_string()));
                    }
                });
            });
        });
    }

    fn language_card(&mut self, ui: &mut Ui, s: &AppState) {
        section_title(ui, t!("Language"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(t!("App language")).color(TEXT()));
                    ui.label(RichText::new(t!("Automatic follows your computer's language.")).size(12.0).color(TEXT_DIM()));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let chosen = s.config.language.clone();
                    let name_of = |code: &str| -> String {
                        if code == "auto" {
                            let now = crate::i18n::LANGUAGES.iter().find(|(c, _)| *c == crate::i18n::system_language()).map(|(_, n)| *n).unwrap_or("English");
                            format!("{} ({now})", t!("Automatic"))
                        } else {
                            crate::i18n::LANGUAGES.iter().find(|(c, _)| *c == code).map(|(_, n)| n.to_string()).unwrap_or_else(|| code.to_string())
                        }
                    };
                    egui::ComboBox::from_id_salt("language").width(200.0).selected_text(name_of(&chosen)).show_ui(ui, |ui| {
                        let mut pick = chosen.clone();
                        ui.selectable_value(&mut pick, "auto".to_string(), name_of("auto"));
                        for (code, name) in crate::i18n::LANGUAGES {
                            ui.selectable_value(&mut pick, code.to_string(), *name);
                        }
                        if pick != chosen {
                            self.send(Cmd::SetLanguage(pick));
                        }
                    });
                });
            });
        });
    }

    fn assistant_card(&mut self, ui: &mut Ui, s: &AppState) {
        section_title(ui, t!("Assistant link (MCP)"));
        let mut m = s.config.mcp.clone();
        let before = m.clone();
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let row = |ui: &mut Ui, title: &str, sub: &str, on: &mut bool| {
                ui.horizontal(|ui| {
                    ui.set_min_height(40.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).color(TEXT()));
                        ui.label(RichText::new(sub).size(12.0).color(TEXT_DIM()));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        toggle(ui, on, title);
                    });
                });
            };
            row(ui, t!("Let AI assistants use Roomtone"), t!("Assistants that support MCP can see your rooms and what's playing."), &mut m.enabled);
            if m.enabled {
                ui.horizontal(|ui| {
                    let (color, text) = match s.mcp_status.split_once(':') {
                        Some(("ok", addr)) => (GOOD(), t!("Ready at {address}", address = addr)),
                        Some(("err", e)) => (BAD(), t!("Couldn't open port {port}: {error}", port = m.port, error = e)),
                        _ => (TEXT_DIM(), t!("Starting…").to_string()),
                    };
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, color);
                    ui.label(RichText::new(text).size(12.5).color(color));
                });
                ui.add_space(6.0);
                ui.separator();
                row(ui, t!("Assistants can change things"), t!("Play, pause, change volume, edit the queue and group rooms. Off: they can only look."), &mut m.allow_control);
                ui.separator();
                row(ui, t!("Allow other computers"), t!("Assistants on other computers in your home can connect with the access key."), &mut m.allow_network);
                ui.separator();
                ui.add_space(4.0);
                ui.label(RichText::new(t!("Connect an assistant")).color(TEXT()));
                ui.label(
                    RichText::new(t!(
                        "Apps that start the link themselves (such as desktop assistants) run Roomtone with --mcp. Others connect to the web address with the access key."
                    ))
                    .size(12.0)
                    .color(TEXT_DIM()),
                );
                ui.add_space(6.0);
                let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "roomtone".into());
                let address = crate::mcp::address(&m);
                egui::Grid::new("mcp_info").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new(t!("Web address")).color(TEXT_DIM()));
                    ui.label(RichText::new(&address).monospace().color(TEXT()));
                    ui.end_row();
                    ui.label(RichText::new(t!("Port")).color(TEXT_DIM()));
                    ui.add(egui::DragValue::new(&mut m.port).range(1024..=65535));
                    ui.end_row();
                    ui.label(RichText::new(t!("Access key")).color(TEXT_DIM()));
                    let shown: String = m.token.chars().take(6).chain("••••••••••".chars()).collect();
                    ui.label(RichText::new(shown).monospace().color(TEXT()));
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if pill(ui, Some(Icon::Plus), t!("Copy setup"), true).on_hover_text(t!("A ready-made setup to paste into an assistant's MCP settings")).clicked() {
                        let setup = serde_json::json!({"mcpServers": {"roomtone": {"command": exe, "args": ["--mcp"]}}});
                        ui.ctx().copy_text(serde_json::to_string_pretty(&setup).unwrap_or_default());
                        self.toast_here(t!("Setup copied"));
                    }
                    if pill(ui, None, t!("Copy web address"), false).clicked() {
                        ui.ctx().copy_text(address.clone());
                        self.toast_here(t!("Web address copied"));
                    }
                    if pill(ui, None, t!("Copy access key"), false).clicked() {
                        ui.ctx().copy_text(m.token.clone());
                        self.toast_here(t!("Access key copied"));
                    }
                    if pill(ui, Some(Icon::Refresh), t!("New key"), false).on_hover_text(t!("Assistants using the old key will need the new one")).clicked() {
                        m.token = crate::config::new_token();
                    }
                });
                ui.add_space(8.0);
                self.connect_help(ui, &address);
            }
        });
        if m != before {
            self.send(Cmd::SetMcp(m));
        }
    }

    /// Steps for this platform, with commands to copy.
    fn connect_help(&mut self, ui: &mut Ui, address: &str) {
        let exe = crate::help::exe();
        egui::CollapsingHeader::new(RichText::new(t!("How to connect on {platform}", platform = crate::help::platform())).size(13.5).color(ACCENT())).id_salt("mcp_help").show(
            ui,
            |ui| {
                let steps: Vec<(String, String)> = vec![
                    (
                        t!("Desktop assistants: choose Copy setup and paste it into their MCP settings file, for example:").to_string(),
                        crate::help::assistant_config_file().to_string(),
                    ),
                    (t!("Claude Code, in a terminal:").to_string(), format!("claude mcp add roomtone -- {exe} --mcp")),
                    (t!("Assistants that connect to a web address: keep the app open, or run this in a terminal:").to_string(), format!("{exe} --headless")),
                    (t!("Then connect to this address with the access key as a Bearer header:").to_string(), address.to_string()),
                    (t!("The full guide for this computer, including starting automatically in the background:").to_string(), format!("{exe} --help mcp")),
                ];
                for (text, command) in steps {
                    ui.add_space(4.0);
                    ui.label(RichText::new(text).size(12.5).color(TEXT_DIM()));
                    ui.horizontal(|ui| {
                        let w = ui.available_width() - 70.0;
                        ui.add_sized([w, 20.0], egui::Label::new(RichText::new(&command).monospace().size(12.0).color(TEXT())).truncate());
                        if ui.add(egui::Button::new(RichText::new(t!("Copy")).size(12.0)).corner_radius(10.0)).clicked() {
                            ui.ctx().copy_text(command.clone());
                            self.toast_here(t!("Copied"));
                        }
                    });
                }
            },
        );
    }

    fn room_page(&mut self, ui: &mut Ui, s: &AppState, uuid: &str) {
        let Some((h, r)) = s.find_room(uuid) else {
            self.go(View::Settings);
            return;
        };
        if r.settings.is_none() {
            self.ask_once(&format!("room:{uuid}"), Duration::from_secs(8), Cmd::LoadRoomSettings(uuid.to_string()));
        }
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Back, 18.0, TEXT()).clicked() {
                self.go(View::Settings);
            }
            ui.label(RichText::new(t!("Settings")).size(13.0).color(TEXT_DIM()));
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&r.name).size(30.0).strong().color(TEXT()));
            ui.add_space(6.0);
            if pill(ui, None, t!("Rename"), false).clicked() {
                self.rename = Some((uuid.to_string(), r.name.clone()));
            }
        });
        ui.horizontal(|ui| {
            chip(ui, &t!("{system} system", system = h.system.label()), system_tint(h.system));
            if !r.model.is_empty() {
                chip(ui, &r.model, TEXT_DIM());
            }
        });
        ui.add_space(14.0);

        section_title(ui, t!("Volume"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let (a, b) = (uuid.to_string(), uuid.to_string());
            self.volume_line(ui, &r.name, &format!("r:{uuid}"), r.volume, r.muted, false, move |v| Cmd::RoomVolume(a.clone(), v), move |m| Cmd::RoomMute(b.clone(), m));
        });
        ui.add_space(14.0);

        section_title(ui, t!("Sound"));
        let Some(st) = r.settings.clone() else {
            spinner_dots(ui, TEXT_DIM());
            return;
        };
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let w = ui.available_width();
            for (label, value, is_bass) in [(t!("Bass"), st.bass, true), (t!("Treble"), st.treble, false)] {
                ui.horizontal(|ui| {
                    ui.set_height(34.0);
                    ui.allocate_ui_with_layout(vec2(90.0, 20.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.set_width(90.0);
                        ui.label(RichText::new(label).color(TEXT()));
                    });
                    ui.label(RichText::new("−").color(TEXT_FAINT()));
                    let mut v = (value + 10) as f32 / 20.0;
                    let (_, changed) = slider(ui, label, label, &mut v, w - 190.0, ACCENT());
                    ui.label(RichText::new("+").color(TEXT_FAINT()));
                    let nv = (v * 20.0).round() as i32 - 10;
                    ui.label(RichText::new(format!("{nv:+}")).size(12.5).color(TEXT_DIM()));
                    if changed && nv != value {
                        self.send(if is_bass { Cmd::SetBass(uuid.to_string(), nv) } else { Cmd::SetTreble(uuid.to_string(), nv) });
                    }
                });
            }
            ui.separator();
            let mut toggles: Vec<(&str, &str, bool, u8)> = vec![(t!("Loudness"), t!("Boosts bass and treble at low volume"), st.loudness, 0)];
            if let Some(n) = st.night {
                toggles.push((t!("Night sound"), t!("Softens loud moments for late-night TV"), n, 1));
            }
            if let Some(sp) = st.speech {
                toggles.push((t!("Speech enhancement"), t!("Makes dialogue clearer"), sp, 2));
            }
            toggles.push((t!("Status light"), t!("The light on the speaker"), st.status_light, 3));
            for (label, sub, mut on, kind) in toggles {
                ui.horizontal(|ui| {
                    ui.set_height(44.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(label).color(TEXT()));
                        ui.label(RichText::new(sub).size(12.0).color(TEXT_DIM()));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if toggle(ui, &mut on, label).changed() {
                            let u = uuid.to_string();
                            self.send(match kind {
                                0 => Cmd::SetLoudness(u, on),
                                1 => Cmd::SetNight(u, on),
                                2 => Cmd::SetSpeech(u, on),
                                _ => Cmd::SetStatusLight(u, on),
                            });
                        }
                    });
                });
            }
        });
        ui.add_space(14.0);
        section_title(ui, t!("About this speaker"));
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("about_room").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                for (k, v) in [
                    (t!("Model"), r.model.clone()),
                    (t!("Software version"), r.version.clone()),
                    (t!("System"), format!("{} ({})", h.system.label(), if h.system == System::S1 { t!("legacy") } else { t!("current") })),
                    (t!("IP address"), if r.ip.is_empty() { t!("Demo").into() } else { r.ip.clone() }),
                    (t!("Speaker ID"), r.uuid.clone()),
                ] {
                    ui.label(RichText::new(k).color(TEXT_DIM()));
                    ui.label(RichText::new(if v.is_empty() { "—".to_string() } else { v }).color(TEXT()));
                    ui.end_row();
                }
            });
        });
    }
}

/// A neutral lettered badge for a service (no logos).
fn service_badge(ui: &mut Ui, name: &str, ready: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
    let c = seed_color(name);
    ui.painter().rect_filled(r, 12.0, if ready { c.gamma_multiply(0.45) } else { c.gamma_multiply(0.18) });
    let letter: String = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(19.0), if ready { TEXT() } else { c });
}

/// Open a folder in the platform's file browser.
fn show_folder(path: &str) {
    let _ = std::fs::create_dir_all(path);
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(path).spawn();
}
