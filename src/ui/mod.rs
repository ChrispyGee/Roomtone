//! The window: sidebar, page area, rooms/queue panel and the now-playing bar.

mod art;
mod pages;
pub mod theme;
pub mod widgets;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Align, Color32, Layout, Rect, RichText, Sense, Stroke};

use crate::backend::{Backend, Cmd, Waker};
use crate::model::*;
use crate::net::speaker::PlayHow;
use art::ArtCache;
use theme::*;
use widgets::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum View {
    Home,
    Search,
    Favorites,
    Playlists,
    Library,
    Radio,
    Inputs,
    Alarms,
    Settings,
    NowPlaying,
    Room(String),
    Services,
}

pub struct App {
    backend: Backend,
    art: ArtCache,
    view: View,
    stack: Vec<Item>,
    search: String,
    searched: String,
    search_edit_at: Option<Instant>,
    right_tab: usize,
    group_editor: Option<(String, Vec<String>)>,
    alarm_editor: Option<Alarm>,
    rename: Option<(String, String)>,
    save_queue: Option<(String, String)>,
    manual_ip: String,
    seek_drag: Option<f32>,
    local_vol: HashMap<String, (f32, Instant)>,
    asked: HashMap<String, Instant>,
    out: Vec<Cmd>,
    select: Option<String>,
    pick_household: Option<usize>,
    shot: Option<PathBuf>,
    frame: u64,
    list_filter: String,
    list_sort: usize,
    list_genre: Option<String>,
    list_key: String,
    search_sources: std::collections::HashSet<String>,
    search_types: std::collections::HashSet<String>,
    search_sort: usize,
    search_genre: Option<String>,
    station_editor: Option<crate::config::Station>,
    radio_tag: String,
    radio_search: String,
    radio_searched: String,
    library_scope: usize,
    subscribed: std::collections::HashSet<String>,
    link_dismissed: bool,
    speakers_open: bool,
    merged_source: usize,
    current_group: Option<String>,
    /// Saved favorites this frame: (address, favorite id) on the speakers, and ids kept in the app.
    speaker_favs: Vec<(String, String)>,
    app_favs: std::collections::HashSet<String>,
    /// A named group being created or edited: (old name if editing, the group).
    group_preset: Option<(Option<String>, crate::config::SavedGroup)>,
    /// Short notes to show once the shared state is free again.
    notes: Vec<String>,
    /// The appearance last applied ("auto", "light", "dark").
    applied_theme: String,
    /// Set once the `--screenshot` picture has been saved.
    shot_saved: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// For each group playing along with one in another system: "S1 · Office".
    along: HashMap<String, String>,
    /// A software update waiting for the person to confirm: (household id, system, version).
    update_confirm: Option<(String, System, String)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>, demo: bool, view: Option<&str>) -> App {
        let preset_search = view.and_then(|v| v.strip_prefix("search:")).map(String::from);
        let open_speakers = view == Some("speakers");
        let open_new_group = view == Some("newgroup");
        // Test hook: "--view open:<list id>" opens that list in the library.
        let open_list = view.and_then(|v| v.strip_prefix("open:")).map(|id| Item {
            id: id.to_string(),
            title: id.rsplit('/').next().unwrap_or(id).trim_start_matches("ALL:").trim_start_matches("ARTIST:").to_string(),
            class: if id.contains("ARTIST/") || id.starts_with("ARTIST:") {
                "object.container.person.musicArtist".into()
            } else if id.contains("ALBUM/") {
                "object.container.album.musicAlbum".into()
            } else {
                "object.container".into()
            },
            source: if id.starts_with("LOCAL:") { "local".into() } else { String::new() },
            is_container: true,
            ..Default::default()
        });
        let view = if open_list.is_some() { Some("library") } else { view };
        let view = match view.map(|v| if v.starts_with("search:") { "search" } else { v }).unwrap_or("home") {
            "search" => View::Search,
            "favorites" => View::Favorites,
            "playlists" => View::Playlists,
            "library" => View::Library,
            "radio" => View::Radio,
            "inputs" => View::Inputs,
            "alarms" => View::Alarms,
            "settings" => View::Settings,
            "nowplaying" => View::NowPlaying,
            "services" => View::Services,
            "speakers" => View::Home,
            "room" => View::Room("D-LIVING".into()),
            _ => View::Home,
        };
        theme::apply(&cc.egui_ctx);
        let backend = Backend::start(Waker::window(cc.egui_ctx.clone()), demo);
        // Picture checks mustn't take the assistant link's port from a running copy.
        if shot.is_none() {
            crate::mcp::start_http(crate::mcp::Mcp::new(backend.clone()), Waker::window(cc.egui_ctx.clone()));
        }
        App {
            backend,
            art: ArtCache::new(&cc.egui_ctx),
            view,
            stack: open_list.into_iter().collect(),
            search: preset_search.clone().unwrap_or_default(),
            searched: String::new(),
            search_edit_at: preset_search.map(|_| Instant::now() - Duration::from_secs(2)),
            right_tab: 0,
            group_editor: None,
            alarm_editor: None,
            rename: None,
            save_queue: None,
            manual_ip: String::new(),
            seek_drag: None,
            local_vol: HashMap::new(),
            asked: HashMap::new(),
            out: vec![],
            select: None,
            pick_household: None,
            shot,
            frame: 0,
            list_filter: String::new(),
            list_sort: 0,
            list_genre: None,
            list_key: String::new(),
            search_sources: Default::default(),
            search_types: Default::default(),
            search_sort: 0,
            search_genre: None,
            station_editor: None,
            radio_tag: String::from("TOP"),
            radio_search: String::new(),
            radio_searched: String::new(),
            library_scope: 0,
            subscribed: Default::default(),
            link_dismissed: false,
            speakers_open: open_speakers,
            merged_source: 0,
            current_group: None,
            speaker_favs: vec![],
            app_favs: Default::default(),
            notes: vec![],
            along: HashMap::new(),
            shot_saved: Default::default(),
            applied_theme: String::new(),
            update_confirm: None,
            group_preset: open_new_group
                .then(|| (None, crate::config::SavedGroup { name: "Downstairs".into(), household: "demo-s2".into(), rooms: vec!["D-LIVING".into(), "D-KITCHEN".into()] })),
        }
    }

    fn send(&mut self, cmd: Cmd) {
        self.out.push(cmd);
    }

    fn toast_here(&mut self, text: &str) {
        self.notes.push(text.to_string());
    }

    /// Ask the backend for something at most once every `every`.
    fn ask_once(&mut self, key: &str, every: Duration, cmd: Cmd) {
        let due = self.asked.get(key).is_none_or(|t| t.elapsed() > every);
        if due {
            self.asked.insert(key.to_string(), Instant::now());
            self.out.push(cmd);
        }
    }

    fn browse<'a>(&mut self, s: &'a AppState, id: &str) -> Option<&'a Browse> {
        match s.browse.get(id) {
            Some(b) => Some(b),
            None => {
                self.ask_once(&format!("browse:{id}"), Duration::from_secs(3), Cmd::Browse(id.to_string()));
                None
            }
        }
    }

    fn go(&mut self, view: View) {
        self.view = view;
        self.stack.clear();
    }

    fn open(&mut self, item: &Item) {
        self.stack.push(item.clone());
    }

    /// The volume to draw: the person's own drag wins for a moment over polled values.
    fn shown_volume(&mut self, key: &str, actual: u8) -> f32 {
        match self.local_vol.get(key) {
            Some((v, t)) if t.elapsed() < Duration::from_millis(1500) => *v,
            _ => actual as f32 / 100.0,
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot.clone() else { return };
        self.frame += 1;
        ctx.request_repaint();
        let at = std::env::var("ROOMTONE_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(if std::env::var_os("ROOMTONE_LIVE").is_some() { 600 } else { 120 });
        // Test hook: group some rooms early in a screenshot run (comma-separated room ids).
        if self.frame == 30 {
            if let Ok(rooms) = std::env::var("ROOMTONE_TEST_GROUP") {
                self.backend.send(Cmd::GroupRooms(rooms.split(',').map(String::from).collect()));
            }
        }
        // Test hook: open the group editor for the selected room.
        if self.frame == 20 && std::env::var_os("ROOMTONE_TEST_EDITOR").is_some() {
            let s = self.backend.lock();
            if let Some((_, g)) = s.selected_group.as_ref().and_then(|g| s.find_group(g)) {
                self.group_editor = Some((g.id.clone(), g.members.clone()));
            }
        }
        // Test hook: open the macOS About box and save a picture of it beside the screenshot.
        #[cfg(target_os = "macos")]
        if std::env::var_os("ROOMTONE_TEST_ABOUT").is_some() && (self.frame == 40 || self.frame == 100) {
            crate::macos_about::test_snapshot(self.frame == 40, &path.with_extension("about.png"));
        }
        // Test hook: ask for a reconnect part-way through a live screenshot.
        if self.frame == 200 && std::env::var_os("ROOMTONE_TEST_RECONNECT").is_some() {
            self.backend.send(Cmd::Reconnect);
        }
        if self.frame == at {
            let (done, save_to, repaint) = (self.shot_saved.clone(), path.clone(), ctx.clone());
            let callback = egui::ScreenshotCallback::new(move |img| {
                if let Some(dir) = save_to.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let [w, h] = img.size;
                match image::save_buffer(&save_to, img.as_raw(), w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                    Ok(()) => eprintln!("Saved screenshot to {}", save_to.display()),
                    Err(e) => eprintln!("Couldn't save screenshot: {e}"),
                }
                done.store(true, std::sync::atomic::Ordering::SeqCst);
                repaint.request_repaint();
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(callback));
        }
        if self.shot_saved.load(std::sync::atomic::Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.frame > at + 900 {
            eprintln!("Screenshot timed out");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn draw(&mut self, root: &mut egui::Ui, s: &AppState) {
        let ctx = &root.ctx().clone();
        let household = s.household();
        let group = s.selected_group.as_ref().and_then(|g| s.find_group(g));

        egui::Panel::left("sidebar")
            .exact_size(232.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(SIDEBAR()).inner_margin(egui::Margin { left: 16, right: 16, top: 22, bottom: 16 }))
            .show(root, |ui| self.sidebar(ui, s));

        egui::Panel::bottom("nowbar")
            .exact_size(92.0)
            .frame(egui::Frame::new().fill(SIDEBAR()).stroke(Stroke::new(1.0_f32, LINE())).inner_margin(egui::Margin { left: 18, right: 22, top: 12, bottom: 12 }))
            .show(root, |ui| self.now_bar(ui, s, group));

        egui::Panel::right("rooms")
            .exact_size(340.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(BG()).inner_margin(egui::Margin { left: 8, right: 18, top: 22, bottom: 12 }))
            .show(root, |ui| self.right_panel(ui, s));

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG()).inner_margin(egui::Margin { left: 30, right: 22, top: 22, bottom: 0 })).show(root, |ui| {
            if household.is_none() {
                self.searching_page(ui, s);
                return;
            }
            let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt(("page", format!("{:?}", self.view), self.stack.len()));
            // Test hook for screenshots of lower parts of a page.
            if let Some(px) = std::env::var("ROOMTONE_SCROLL").ok().and_then(|v| v.parse::<f32>().ok()) {
                area = area.vertical_scroll_offset(px);
            }
            area.show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                self.page(ui, s);
                ui.add_space(30.0);
            });
        });

        self.modals(ctx, s);
        self.station_modal(ctx);
        self.link_modal(ctx, s);
        self.speakers_modal(ctx, s);
        self.group_preset_modal(ctx, s);
        self.update_modal(ctx);
        self.toasts(ctx, s);
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, s: &AppState) {
        // Brand mark: a soft gradient orb and the app's name.
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 15.0, ACCENT());
            ui.painter().circle_filled(r.center() + vec2(4.0, -4.0), 8.0, Color32::from_rgb(255, 196, 140));
            ui.painter().circle_stroke(r.center(), 15.0, Stroke::new(1.0_f32, Color32::from_white_alpha(40)));
            ui.add_space(2.0);
            ui.label(RichText::new(t!("Roomtone")).size(20.0).strong().color(TEXT()));
        });
        ui.add_space(22.0);

        let items: [(View, Icon, &str); 10] = [
            (View::Home, Icon::Home, t!("Home")),
            (View::Search, Icon::Search, t!("Search")),
            (View::Favorites, Icon::Star, t!("Favorites")),
            (View::Playlists, Icon::Playlist, t!("Playlists")),
            (View::Library, Icon::Library, t!("Music Library")),
            (View::Radio, Icon::Radio, t!("Radio")),
            (View::Services, Icon::Services, t!("Music Services")),
            (View::Inputs, Icon::Input, t!("Line-In & TV")),
            (View::Alarms, Icon::Alarm, t!("Alarms")),
            (View::Settings, Icon::Gear, t!("Settings")),
        ];
        for (view, icon, label) in items {
            let active = self.view == view || (view == View::Settings && matches!(self.view, View::Room(_)));
            if nav_item(ui, icon, label, active).clicked() {
                self.go(view);
            }
        }

        ui.add_space(18.0);
        ui.label(RichText::new(t!("SYSTEMS")).size(11.0).color(TEXT_FAINT()).strong());
        ui.add_space(4.0);
        let mut counts: HashMap<System, usize> = HashMap::new();
        for (i, h) in s.households.iter().enumerate() {
            let n = counts.entry(h.system).or_insert(0);
            *n += 1;
            let label = if *n > 1 { t!("{system} system {n}", system = h.system.label(), n = n) } else { t!("{system} system", system = h.system.label()) };
            let playing = h.groups.iter().filter(|g| g.now.transport == Transport::Playing).count();
            let sub =
                format!("{}{}", tn!(h.rooms.len(), "1 room", "{n} rooms"), if playing > 0 { format!(" · {}", tn!(playing, "1 playing", "{n} playing")) } else { String::new() });
            if system_item(ui, &label, &sub, system_tint(h.system), i == s.active).clicked() {
                self.pick_household = Some(i);
                self.stack.clear();
            }
        }

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            let (dot, text) = match s.phase {
                Phase::Searching => (TEXT_DIM(), t!("Looking for speakers…").to_string()),
                Phase::Live if s.reconnecting => (ACCENT(), t!("Reconnecting…").to_string()),
                Phase::Live => {
                    let n: usize = s.households.iter().map(|h| h.rooms.len()).sum();
                    let answering = s.households.iter().flat_map(|h| h.rooms.iter()).filter(|r| r.online != Some(false)).count();
                    if answering == n {
                        (GOOD(), tn!(n, "Connected · 1 speaker", "Connected · {n} speakers"))
                    } else if answering == 0 {
                        (BAD(), t!("Speakers not responding").to_string())
                    } else {
                        (ACCENT(), t!("{answering} of {n} speakers responding", answering = answering, n = n))
                    }
                }
                Phase::Demo => (ACCENT(), t!("Demo system").to_string()),
                Phase::NothingFound => (BAD(), t!("No speakers found").to_string()),
            };
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(rect, 8.0, SURFACE());
            }
            ui.painter().circle_filled(pos2(rect.min.x + 10.0, rect.center().y), 4.0, dot);
            paint_text(ui, pos2(rect.min.x + 22.0, rect.center().y - 8.0), &text, 12.0, TEXT_DIM(), rect.width() - 40.0);
            paint_icon(ui.painter(), Rect::from_center_size(pos2(rect.max.x - 12.0, rect.center().y), vec2(12.0, 12.0)), Icon::Up, TEXT_FAINT());
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(t!("Show your speakers and their status")).clicked() {
                self.speakers_open = true;
            }
        });
    }

    fn right_panel(&mut self, ui: &mut egui::Ui, s: &AppState) {
        let Some(h) = s.household() else {
            ui.label(RichText::new(t!("Rooms")).size(19.0).strong());
            ui.add_space(8.0);
            spinner_dots(ui, TEXT_DIM());
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(if self.right_tab == 0 { t!("Rooms") } else { t!("Queue") }).size(19.0).strong().color(TEXT()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(i) = segmented(ui, &[t!("Rooms"), t!("Queue")], self.right_tab, if crate::i18n::current() == "en" { 150.0 } else { 220.0 }) {
                    self.right_tab = i;
                }
            });
        });
        ui.add_space(10.0);
        if self.right_tab == 0 {
            egui::ScrollArea::vertical().id_salt("rooms_scroll").auto_shrink([false, false]).show(ui, |ui| {
                for g in &h.groups {
                    let selected = s.selected_group.as_deref() == Some(g.id.as_str());
                    self.room_card(ui, h, g, selected);
                    ui.add_space(8.0);
                }
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if h.groups.len() > 1 && pill(ui, Some(Icon::Rooms), t!("Group all"), false).clicked() {
                        if let Some(gid) = s.selected_group.clone() {
                            let all = h.rooms.iter().map(|r| r.uuid.clone()).collect();
                            self.send(Cmd::SetGroupMembers(gid, all));
                        }
                    }
                    let playing: Vec<String> = h.groups.iter().filter(|g| g.now.transport == Transport::Playing).map(|g| g.id.clone()).collect();
                    if !playing.is_empty() && pill(ui, Some(Icon::Pause), t!("Pause all"), false).clicked() {
                        for g in playing {
                            self.send(Cmd::Pause(g));
                        }
                    }
                    if h.groups.iter().any(|g| g.members.len() > 1) && pill(ui, None, t!("Ungroup"), false).clicked() {
                        for g in h.groups.iter().filter(|g| g.members.len() > 1) {
                            self.send(Cmd::SetGroupMembers(g.id.clone(), vec![g.id.clone()]));
                        }
                    }
                });
                ui.add_space(14.0);
                self.saved_groups(ui, s, h);
                ui.add_space(6.0);
                ui.label(RichText::new(t!("{system} speakers can only be grouped with rooms in the same system.", system = h.system.label())).size(11.5).color(TEXT_FAINT()));
            });
        } else {
            self.queue_panel(ui, s);
        }
    }

    fn room_card(&mut self, ui: &mut egui::Ui, h: &Household, g: &Group, selected: bool) {
        let frame = egui::Frame::new()
            .fill(if selected { SURFACE_HI() } else { SURFACE() })
            .corner_radius(16.0)
            .stroke(if selected { Stroke::new(1.5_f32, ACCENT().gamma_multiply(0.8)) } else { Stroke::NONE })
            .inner_margin(12.0);
        let mut inner_hot = false;
        let resp = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
                let seed = if g.now.album.is_empty() { g.now.title.clone() } else { g.now.album.clone() };
                if g.now.is_empty() {
                    ui.painter().rect_filled(r, 10.0, SURFACE_HOVER());
                    paint_icon(ui.painter(), Rect::from_center_size(r.center(), vec2(22.0, 22.0)), Icon::Speaker, TEXT_FAINT());
                } else {
                    paint_art(ui, &mut self.art, r, g.now.art.as_deref(), &seed, 10.0);
                }
                ui.vertical(|ui| {
                    let w = ui.available_width() - 44.0;
                    ui.add_space(3.0);
                    clipped_label(ui, &h.group_name(g), 15.0, TEXT(), w, true);
                    let line = if g.now.is_empty() {
                        t!("Nothing playing").to_string()
                    } else if g.now.artist.is_empty() {
                        g.now.title.clone()
                    } else {
                        format!("{} · {}", g.now.title, g.now.artist)
                    };
                    clipped_label(ui, &line, 12.5, TEXT_DIM(), w, false);
                    if g.members.len() > 1 {
                        let names: Vec<&str> = g.members.iter().filter_map(|m| h.room(m).map(|r| r.name.as_str())).collect();
                        clipped_label(ui, &names.join(", "), 11.5, TEXT_FAINT(), w, false);
                    }
                    if let Some(other) = self.along.get(&g.id) {
                        clipped_label(ui, &t!("Plays along with {room}", room = other), 11.5, ACCENT(), w, false);
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let playing = g.now.transport == Transport::Playing;
                    let b = round_button(
                        ui,
                        if playing { Icon::Pause } else { Icon::Play },
                        34.0,
                        if selected { ACCENT() } else { SURFACE_HOVER() },
                        if selected { Color32::BLACK } else { TEXT() },
                    );
                    inner_hot |= b.hovered();
                    if b.clicked() {
                        self.send(if playing { Cmd::Pause(g.id.clone()) } else { Cmd::Play(g.id.clone()) });
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let m = icon_button(ui, if g.muted { Icon::Muted } else { Icon::Speaker }, 16.0, if g.muted { BAD() } else { TEXT_DIM() });
                inner_hot |= m.hovered();
                if m.clicked() {
                    self.send(Cmd::GroupMute(g.id.clone(), !g.muted));
                }
                let key = format!("g:{}", g.id);
                let mut v = self.shown_volume(&key, g.volume);
                let w = ui.available_width() - 76.0;
                let (sr, changed) = slider(ui, &key, &mut v, w, if selected { ACCENT() } else { TEXT_DIM() });
                inner_hot |= sr.hovered() || sr.dragged();
                if changed {
                    self.local_vol.insert(key.clone(), (v, Instant::now()));
                    self.send(Cmd::GroupVolume(g.id.clone(), (v * 100.0).round() as u8));
                }
                ui.label(RichText::new(format!("{}", (v * 100.0).round() as u8)).size(12.0).color(TEXT_DIM()));
                let gb = icon_button(ui, Icon::Rooms, 16.0, TEXT_DIM()).on_hover_text(t!("Group rooms"));
                inner_hot |= gb.hovered();
                if gb.clicked() {
                    self.group_editor = Some((g.id.clone(), g.members.clone()));
                }
            });
        });
        let rect = resp.response.rect;
        let card = ui.interact(rect, ui.scope_id().with(("card", &g.id)), Sense::hover());
        if !inner_hot && card.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if ui.input(|i| i.pointer.primary_clicked()) {
                self.select = Some(g.id.clone());
            }
        }
    }

    fn queue_panel(&mut self, ui: &mut egui::Ui, s: &AppState) {
        let Some(gid) = s.selected_group.clone() else {
            ui.label(RichText::new(t!("Pick a room to see its queue.")).color(TEXT_DIM()));
            return;
        };
        let Some((h, g)) = s.find_group(&gid) else { return };
        if s.phase == Phase::Live {
            self.ask_once(&format!("queue:{gid}"), Duration::from_secs(12), Cmd::LoadQueue(gid.clone()));
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(h.group_name(g)).color(TEXT_DIM()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, Icon::Trash, 16.0, TEXT_DIM()).on_hover_text(t!("Clear queue")).clicked() {
                    self.send(Cmd::ClearQueue(gid.clone()));
                }
                if icon_button(ui, Icon::Save, 16.0, TEXT_DIM()).on_hover_text(t!("Save as playlist")).clicked() {
                    self.save_queue = Some((gid.clone(), String::from(t!("My playlist"))));
                }
            });
        });
        let Some(queue) = &g.queue else {
            spinner_dots(ui, TEXT_DIM());
            return;
        };
        if queue.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                paint_icon(ui.painter(), Rect::from_center_size(ui.cursor().min + vec2(ui.available_width() / 2.0, 20.0), vec2(36.0, 36.0)), Icon::Queue, TEXT_FAINT());
                ui.add_space(48.0);
                ui.label(RichText::new(t!("The queue is empty")).color(TEXT_DIM()));
                ui.label(RichText::new(t!("Add songs, albums or playlists from your music.")).size(12.0).color(TEXT_FAINT()));
            });
            return;
        }
        let current = if g.now.source == "Queue" { g.now.track_no as usize } else { 0 };
        ui.label(RichText::new(tn!(queue.len(), "1 song", "{n} songs")).size(12.0).color(TEXT_FAINT()));
        egui::ScrollArea::vertical().id_salt("queue_scroll").auto_shrink([false, false]).show(ui, |ui| {
            for (i, it) in queue.iter().enumerate() {
                let is_current = current == i + 1;
                let w = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 50.0), Sense::click());
                if resp.hovered() || is_current {
                    ui.painter().rect_filled(rect, 10.0, if is_current { ACCENT_SOFT() } else { SURFACE() });
                }
                let art_r = Rect::from_min_size(rect.min + vec2(6.0, 7.0), vec2(36.0, 36.0));
                let seed = if it.album.is_empty() { &it.title } else { &it.album };
                paint_art(ui, &mut self.art, art_r, it.art.as_deref(), seed, 7.0);
                let tx = art_r.max.x + 10.0;
                let tw = rect.max.x - tx - if resp.hovered() { 92.0 } else { 46.0 };
                paint_text(ui, pos2(tx, rect.min.y + 8.0), &it.title, 13.5, if is_current { ACCENT() } else { TEXT() }, tw);
                paint_text(ui, pos2(tx, rect.min.y + 27.0), &it.artist, 12.0, TEXT_DIM(), tw);
                if resp.hovered() {
                    let mut x = rect.max.x - 30.0;
                    for (icon, action) in [(Icon::Close, 0), (Icon::Down, 1), (Icon::Up, 2)] {
                        let br = Rect::from_center_size(pos2(x, rect.center().y), vec2(26.0, 26.0));
                        let b = ui.interact(br, ui.scope_id().with(("q", i, action)), Sense::click());
                        if b.hovered() {
                            ui.painter().circle_filled(br.center(), 13.0, SURFACE_HOVER());
                        }
                        paint_icon(ui.painter(), br.shrink(6.0), icon, TEXT_DIM());
                        if b.clicked() {
                            match action {
                                0 => self.send(Cmd::RemoveFromQueue(gid.clone(), i)),
                                1 if i + 1 < queue.len() => self.send(Cmd::MoveInQueue(gid.clone(), i, i + 1)),
                                2 if i > 0 => self.send(Cmd::MoveInQueue(gid.clone(), i, i - 1)),
                                _ => {}
                            }
                        }
                        x -= 28.0;
                    }
                } else if it.duration > 0.0 {
                    ui.painter().text(pos2(rect.max.x - 10.0, rect.center().y), egui::Align2::RIGHT_CENTER, fmt_time(it.duration), egui::FontId::proportional(12.0), TEXT_FAINT());
                }
                if resp.clicked() {
                    self.send(Cmd::PlayQueueIndex(gid.clone(), i));
                }
                resp.context_menu(|ui| {
                    if ui.button(t!("▶  Play this")).clicked() {
                        self.out.push(Cmd::PlayQueueIndex(gid.clone(), i));
                        ui.close();
                    }
                    self.favorite_menu_entry(ui, it);
                    if ui.button(t!("Remove from queue")).clicked() {
                        self.out.push(Cmd::RemoveFromQueue(gid.clone(), i));
                        ui.close();
                    }
                });
            }
        });
    }

    fn now_bar(&mut self, ui: &mut egui::Ui, s: &AppState, group: Option<(&Household, &Group)>) {
        let Some((h, g)) = group else {
            ui.centered_and_justified(|ui| ui.label(RichText::new(t!("Pick a room to start listening")).color(TEXT_DIM())));
            return;
        };
        let full = ui.max_rect();
        let gid = g.id.clone();

        // Left: art and titles.
        let left = Rect::from_min_size(full.min, vec2(full.width() * 0.3, full.height()));
        let art_r = Rect::from_min_size(left.min + vec2(0.0, 2.0), vec2(64.0, 64.0));
        let seed = if g.now.album.is_empty() { g.now.title.clone() } else { g.now.album.clone() };
        if g.now.is_empty() {
            ui.painter().rect_filled(art_r, 10.0, SURFACE_HI());
        } else {
            paint_art(ui, &mut self.art, art_r, g.now.art.as_deref(), &seed, 10.0);
        }
        let art_click = ui.interact(art_r, ui.scope_id().with("np_art"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        if art_click.clicked() {
            self.go(View::NowPlaying);
        }
        let tx = art_r.max.x + 14.0;
        let tw = left.max.x - tx - 10.0;
        let title = if g.now.is_empty() { t!("Nothing playing") } else { g.now.title.as_str() };
        paint_text(ui, pos2(tx, full.min.y + 6.0), title, 15.0, TEXT(), tw);
        paint_text(ui, pos2(tx, full.min.y + 27.0), &g.now.artist, 13.0, TEXT_DIM(), tw);
        let room_line = format!("{}  ·  {}", h.group_name(g), h.system.label());
        paint_text(ui, pos2(tx, full.min.y + 47.0), &room_line, 11.5, ACCENT(), tw);

        // Centre: transport and progress.
        let cw = (full.width() * 0.4).clamp(320.0, 520.0);
        let center = Rect::from_center_size(full.center(), vec2(cw, full.height()));
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(center).layout(Layout::top_down(Align::Center)));
        cui.horizontal(|ui| {
            let total = 5.0 * 36.0 + 44.0;
            ui.add_space(((cw - total) / 2.0).max(0.0));
            let sh = icon_button(ui, Icon::Shuffle, 18.0, if g.shuffle { ACCENT() } else { TEXT_DIM() }).on_hover_text(t!("Shuffle"));
            if sh.clicked() {
                self.send(Cmd::SetPlayMode(gid.clone(), !g.shuffle, g.repeat));
            }
            if icon_button(ui, Icon::Prev, 18.0, TEXT()).clicked() {
                self.send(Cmd::Previous(gid.clone()));
            }
            let playing = g.now.transport == Transport::Playing;
            if round_button(ui, if playing { Icon::Pause } else { Icon::Play }, 40.0, TEXT(), ON_TEXT()).clicked() {
                self.send(if playing { Cmd::Pause(gid.clone()) } else { Cmd::Play(gid.clone()) });
            }
            if icon_button(ui, Icon::Next, 18.0, TEXT()).clicked() {
                self.send(Cmd::Next(gid.clone()));
            }
            let rp = icon_button(ui, if g.repeat == Repeat::One { Icon::RepeatOne } else { Icon::Repeat }, 18.0, if g.repeat != Repeat::Off { ACCENT() } else { TEXT_DIM() })
                .on_hover_text(t!("Repeat"));
            if rp.clicked() {
                self.send(Cmd::SetPlayMode(gid.clone(), g.shuffle, g.repeat.next()));
            }
        });
        cui.horizontal(|ui| {
            let pos = g.now.live_position();
            let dur = g.now.duration;
            let label_w = 40.0;
            ui.add_sized([label_w, 16.0], egui::Label::new(RichText::new(fmt_time(self.seek_drag.map(|v| v * dur).unwrap_or(pos))).size(11.5).color(TEXT_FAINT())));
            let mut v = if dur > 0.0 { self.seek_drag.unwrap_or(pos / dur) } else { 0.0 };
            let w = ui.available_width() - label_w - 10.0;
            if dur > 0.0 {
                let (r, changed) = slider(ui, "seek", &mut v, w, TEXT());
                if changed {
                    self.seek_drag = Some(v);
                }
                if r.drag_stopped() || (r.clicked() && !r.dragged()) {
                    if let Some(v) = self.seek_drag.take() {
                        self.send(Cmd::Seek(gid.clone(), v * dur));
                    }
                }
            } else {
                let (r, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
                let label = if g.now.is_stream { t!("Live") } else { "" };
                ui.painter().rect_filled(Rect::from_center_size(r.center(), vec2(w, 4.0)), 2.0, overlay(18));
                if !label.is_empty() {
                    chip_at(ui, r.center(), label);
                }
            }
            ui.add_sized([label_w, 16.0], egui::Label::new(RichText::new(if dur > 0.0 { fmt_time(dur) } else { String::new() }).size(11.5).color(TEXT_FAINT())));
        });

        // Right: queue, group, volume, expand.
        let right = Rect::from_min_max(pos2(full.max.x - full.width() * 0.28, full.min.y), full.max);
        let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)));
        if icon_button(&mut rui, Icon::Up, 18.0, TEXT_DIM()).on_hover_text(t!("Now playing")).clicked() {
            self.go(if self.view == View::NowPlaying { View::Home } else { View::NowPlaying });
        }
        let key = format!("g:{gid}");
        let mut v = self.shown_volume(&key, g.volume);
        let sw = (right.width() - 150.0).clamp(80.0, 150.0);
        let (_, changed) = slider(&mut rui, "bar_vol", &mut v, sw, TEXT());
        if changed {
            self.local_vol.insert(key, (v, Instant::now()));
            self.send(Cmd::GroupVolume(gid.clone(), (v * 100.0).round() as u8));
        }
        if icon_button(&mut rui, if g.muted { Icon::Muted } else { Icon::Speaker }, 18.0, if g.muted { BAD() } else { TEXT_DIM() }).clicked() {
            self.send(Cmd::GroupMute(gid.clone(), !g.muted));
        }
        if icon_button(&mut rui, Icon::Rooms, 18.0, TEXT_DIM()).on_hover_text(t!("Group rooms")).clicked() {
            self.group_editor = Some((gid.clone(), g.members.clone()));
        }
        if icon_button(&mut rui, Icon::Queue, 18.0, if self.right_tab == 1 { ACCENT() } else { TEXT_DIM() }).on_hover_text(t!("Queue")).clicked() {
            self.right_tab = 1 - self.right_tab;
        }
        self.now_star(&mut rui, &g.now, 18.0);
        let _ = s;
    }

    fn searching_page(&mut self, ui: &mut egui::Ui, s: &AppState) {
        ui.add_space(ui.available_height() * 0.3);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(t!("Looking for speakers on your network")).size(22.0).color(TEXT()));
            ui.add_space(8.0);
            ui.label(RichText::new(t!("S1 and S2 systems are both found automatically.")).color(TEXT_DIM()));
            ui.add_space(18.0);
            spinner_dots(ui, ACCENT());
            let _ = s;
        });
    }

    fn modals(&mut self, ctx: &egui::Context, s: &AppState) {
        if let Some((gid, mut members)) = self.group_editor.take() {
            let mut keep = true;
            let modal = egui::Modal::new(egui::Id::unique("group_editor")).frame(modal_frame()).show(ctx, |ui| {
                ui.set_width(380.0);
                let Some((h, _)) = s.find_group(&gid) else {
                    keep = false;
                    return;
                };
                ui.label(RichText::new(t!("Group rooms")).size(20.0).strong());
                let lead = h.room(&gid).map(|r| r.name.clone()).unwrap_or_default();
                ui.label(RichText::new(t!("Rooms play together with {lead}.", lead = lead)).color(TEXT_DIM()));
                ui.add_space(10.0);
                for r in &h.rooms {
                    let is_lead = r.uuid == gid;
                    let mut on = members.contains(&r.uuid);
                    ui.horizontal(|ui| {
                        ui.set_height(40.0);
                        paint_icon(ui.painter(), Rect::from_min_size(ui.cursor().min + vec2(0.0, 10.0), vec2(20.0, 20.0)), Icon::Speaker, TEXT_DIM());
                        ui.add_space(30.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&r.name).color(TEXT()));
                            let sub = if is_lead { t!("Playing source").to_string() } else { r.model.clone() };
                            ui.label(RichText::new(sub).size(11.5).color(TEXT_FAINT()));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if is_lead {
                                paint_icon(ui.painter(), Rect::from_min_size(ui.cursor().min + vec2(-26.0, 10.0), vec2(20.0, 20.0)), Icon::Check, ACCENT());
                                ui.add_space(30.0);
                            } else if toggle(ui, &mut on).changed() {
                                if on {
                                    members.push(r.uuid.clone());
                                } else {
                                    members.retain(|m| m != &r.uuid);
                                }
                            }
                        });
                    });
                }
                // S1 and S2 can't group; offer to play along instead.
                let others: Vec<&Household> = s.households.iter().filter(|o| o.id != h.id).collect();
                if !others.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(RichText::new(t!("Play along from your other system")).color(TEXT()).strong());
                    ui.label(
                        RichText::new(t!(
                            "S1 and S2 speakers can't join one group, so these play the same music started at the same moment. They can drift up to a second or two apart."
                        ))
                        .size(11.5)
                        .color(TEXT_FAINT()),
                    );
                    ui.add_space(4.0);
                    for o in others {
                        for og in &o.groups {
                            let mut on = s.links.iter().any(|(l, f)| l == &gid && f == &og.id);
                            let taken = !on && s.linked_with(&og.id).is_some();
                            ui.horizontal(|ui| {
                                ui.set_height(36.0);
                                chip(ui, o.system.label(), system_tint(o.system));
                                ui.add_space(4.0);
                                ui.label(RichText::new(o.group_name(og)).color(if taken { TEXT_FAINT() } else { TEXT() }));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if !taken && toggle(ui, &mut on).changed() {
                                        self.out.push(if on { Cmd::LinkGroups(gid.clone(), og.id.clone()) } else { Cmd::Unlink(og.id.clone()) });
                                    }
                                });
                            });
                        }
                    }
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if pill(ui, None, t!("Select all"), false).clicked() {
                        members = h.rooms.iter().map(|r| r.uuid.clone()).collect();
                    }
                    if pill(ui, None, t!("Clear"), false).clicked() {
                        members = vec![gid.clone()];
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if pill(ui, None, t!("Done"), true).clicked() {
                            self.out.push(Cmd::SetGroupMembers(gid.clone(), members.clone()));
                            keep = false;
                        }
                        if members.len() > 1 && pill(ui, Some(Icon::Save), t!("Save as group"), false).on_hover_text(t!("Keep these rooms as a named group")).clicked() {
                            self.out.push(Cmd::SetGroupMembers(gid.clone(), members.clone()));
                            self.group_preset = Some((None, crate::config::SavedGroup { name: String::new(), household: h.id.clone(), rooms: members.clone() }));
                            keep = false;
                        }
                        if pill(ui, None, t!("Cancel"), false).clicked() {
                            keep = false;
                        }
                    });
                });
            });
            if modal.should_close() {
                keep = false;
            }
            if keep {
                self.group_editor = Some((gid, members));
            }
        }

        if let Some(mut alarm) = self.alarm_editor.take() {
            let mut keep = true;
            let modal = egui::Modal::new(egui::Id::unique("alarm_editor")).frame(modal_frame()).show(ctx, |ui| {
                ui.set_width(420.0);
                keep = self.alarm_form(ui, s, &mut alarm);
            });
            if modal.should_close() {
                keep = false;
            }
            if keep {
                self.alarm_editor = Some(alarm);
            }
        }

        if let Some((uuid, mut name)) = self.rename.take() {
            let mut keep = true;
            let modal = egui::Modal::new(egui::Id::unique("rename")).frame(modal_frame()).show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(RichText::new(t!("Rename room")).size(20.0).strong());
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY).margin(vec2(10.0, 8.0)));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if pill(ui, None, t!("Save"), true).clicked() && !name.trim().is_empty() {
                            self.out.push(Cmd::Rename(uuid.clone(), name.trim().to_string()));
                            keep = false;
                        }
                        if pill(ui, None, t!("Cancel"), false).clicked() {
                            keep = false;
                        }
                    });
                });
            });
            if modal.should_close() {
                keep = false;
            }
            if keep {
                self.rename = Some((uuid, name));
            }
        }

        if let Some((gid, mut name)) = self.save_queue.take() {
            let mut keep = true;
            let modal = egui::Modal::new(egui::Id::unique("save_queue")).frame(modal_frame()).show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(RichText::new(t!("Save queue as playlist")).size(20.0).strong());
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY).margin(vec2(10.0, 8.0)));
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Save"), true).clicked() && !name.trim().is_empty() {
                        self.out.push(Cmd::SaveQueue(gid.clone(), name.trim().to_string()));
                        keep = false;
                    }
                    if pill(ui, None, t!("Cancel"), false).clicked() {
                        keep = false;
                    }
                });
            });
            if modal.should_close() {
                keep = false;
            }
            if keep {
                self.save_queue = Some((gid, name));
            }
        }
    }

    /// Signing in to a streaming service: its web page, the code to type there, and progress.
    fn link_modal(&mut self, ctx: &egui::Context, s: &AppState) {
        let Some(link) = s.link.clone() else { return };
        if self.link_dismissed {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("link_modal")).frame(modal_frame()).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(RichText::new(t!("Connect {service}", service = link.name)).size(20.0).strong());
            ui.add_space(6.0);
            if !link.url.is_empty() && !link.done {
                ui.label(RichText::new(t!("1. Open the sign-in page and log in to your account.")).color(TEXT()));
                ui.add_space(4.0);
                if pill(ui, Some(Icon::Up), t!("Open sign-in page"), true).clicked() {
                    ctx.open_url(egui::OpenUrl::new_tab(link.url.clone()));
                }
                if !link.code.is_empty() {
                    ui.add_space(10.0);
                    ui.label(RichText::new(t!("2. If the page asks for a code, enter:")).color(TEXT()));
                    ui.label(RichText::new(&link.code).size(28.0).strong().color(ACCENT()));
                }
                ui.add_space(10.0);
                ui.label(RichText::new(&link.url).size(11.0).color(TEXT_FAINT()));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if !link.done && !link.failed {
                    spinner_dots(ui, TEXT_DIM());
                }
                ui.label(RichText::new(&link.status).size(13.0).color(if link.failed {
                    BAD()
                } else if link.done {
                    GOOD()
                } else {
                    TEXT_DIM()
                }));
            });
            ui.add_space(14.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if pill(ui, None, if link.done || link.failed { t!("Close") } else { t!("Cancel") }, link.done).clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() {
            close = true;
        }
        if close {
            self.link_dismissed = true;
            self.out.push(Cmd::CancelLink);
        }
    }

    /// The named groups for this system: group them with one click, edit or add more.
    fn saved_groups(&mut self, ui: &mut egui::Ui, s: &AppState, h: &Household) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("Groups")).size(16.0).strong().color(TEXT()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if h.rooms.len() > 1 && pill(ui, Some(Icon::Plus), t!("New group"), false).clicked() {
                    self.group_preset = Some((None, crate::config::SavedGroup { name: String::new(), household: h.id.clone(), rooms: vec![] }));
                }
            });
        });
        let mine: Vec<&crate::config::SavedGroup> = s.config.groups.iter().filter(|g| g.household == h.id).collect();
        if mine.is_empty() {
            ui.label(RichText::new(t!("Save sets of rooms you often play together, like “Downstairs” or “Party”.")).size(12.0).color(TEXT_FAINT()));
            return;
        }
        for g in mine {
            let rooms: Vec<String> = g.rooms.iter().filter(|r| h.room(r).is_some()).cloned().collect();
            let names: Vec<&str> = rooms.iter().filter_map(|r| h.room(r).map(|x| x.name.as_str())).collect();
            let mut wanted = rooms.clone();
            wanted.sort();
            let active = h.groups.iter().any(|lg| {
                let mut m = lg.members.clone();
                m.sort();
                m == wanted
            });
            egui::Frame::new()
                .fill(if active { SURFACE_HI() } else { SURFACE() })
                .corner_radius(14.0)
                .stroke(if active { Stroke::new(1.0_f32, GOOD().gamma_multiply(0.6)) } else { Stroke::NONE })
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                        paint_icon(ui.painter(), r, Icon::Rooms, if active { GOOD() } else { TEXT_DIM() });
                        ui.vertical(|ui| {
                            let w = ui.available_width() - 120.0;
                            clipped_label(ui, &g.name, 14.5, TEXT(), w, true);
                            clipped_label(ui, &names.join(", "), 11.5, TEXT_FAINT(), w, false);
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon_button(ui, Icon::Sliders, 14.0, TEXT_DIM()).on_hover_text(t!("Edit group")).clicked() {
                                self.group_preset = Some((Some(g.name.clone()), g.clone()));
                            }
                            if active {
                                chip(ui, t!("Grouped"), GOOD());
                            } else if rooms.len() > 1 && pill(ui, None, t!("Group"), true).on_hover_text(t!("Group these rooms now")).clicked() {
                                self.send(Cmd::GroupRooms(rooms.clone()));
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
    }

    /// Create or edit a named group: its name and rooms.
    fn group_preset_modal(&mut self, ctx: &egui::Context, s: &AppState) {
        let Some((old, mut g)) = self.group_preset.take() else { return };
        let Some(h) = s.households.iter().find(|h| h.id == g.household).or(s.household()) else { return };
        let mut keep = true;
        let modal = egui::Modal::new(egui::Id::unique("group_preset")).frame(modal_frame()).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new(if old.is_some() { t!("Edit group") } else { t!("New group") }).size(20.0).strong());
            ui.label(RichText::new(t!("Pick the rooms that play together, and give them a name.")).size(12.5).color(TEXT_DIM()));
            ui.add_space(10.0);
            ui.add(egui::TextEdit::singleline(&mut g.name).hint_text(t!("e.g. Downstairs")).desired_width(f32::INFINITY).margin(vec2(10.0, 8.0)));
            ui.add_space(8.0);
            for r in &h.rooms {
                let mut on = g.rooms.contains(&r.uuid);
                ui.horizontal(|ui| {
                    ui.set_height(34.0);
                    let (ir, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                    paint_icon(ui.painter(), ir, Icon::Speaker, TEXT_DIM());
                    ui.label(RichText::new(&r.name).color(TEXT()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if toggle(ui, &mut on).changed() {
                            if on {
                                g.rooms.push(r.uuid.clone());
                            } else {
                                g.rooms.retain(|x| x != &r.uuid);
                            }
                        }
                    });
                });
            }
            ui.add_space(12.0);
            let name_taken = s.config.groups.iter().any(|x| x.name.eq_ignore_ascii_case(g.name.trim()) && Some(&x.name) != old.as_ref());
            let valid = !g.name.trim().is_empty() && g.rooms.len() > 1 && !name_taken;
            if name_taken {
                ui.label(RichText::new(t!("There's already a group with that name.")).size(12.0).color(BAD()));
            } else if g.rooms.len() < 2 {
                ui.label(RichText::new(t!("Choose at least two rooms.")).size(12.0).color(TEXT_FAINT()));
            }
            ui.horizontal(|ui| {
                if let Some(name) = &old {
                    if pill(ui, Some(Icon::Trash), t!("Delete"), false).clicked() {
                        self.out.push(Cmd::DeleteGroup(name.clone()));
                        keep = false;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let save = ui.add_enabled_ui(valid, |ui| pill(ui, None, t!("Save"), true)).inner;
                    if save.clicked() && valid {
                        g.name = g.name.trim().to_string();
                        g.household = h.id.clone();
                        self.out.push(Cmd::SaveGroup(old.clone(), g.clone()));
                        keep = false;
                    }
                    let save_group = ui.add_enabled_ui(valid, |ui| pill(ui, Some(Icon::Rooms), t!("Save & group now"), false)).inner;
                    if save_group.clicked() && valid {
                        g.name = g.name.trim().to_string();
                        g.household = h.id.clone();
                        self.out.push(Cmd::SaveGroup(old.clone(), g.clone()));
                        self.out.push(Cmd::GroupRooms(g.rooms.clone()));
                        keep = false;
                    }
                    if pill(ui, None, t!("Cancel"), false).clicked() {
                        keep = false;
                    }
                });
            });
        });
        if modal.should_close() {
            keep = false;
        }
        if keep {
            self.group_preset = Some((old, g));
        }
    }

    /// Every speaker with whether it's answering, plus ways to reconnect.
    fn speakers_modal(&mut self, ctx: &egui::Context, s: &AppState) {
        if !self.speakers_open {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("speakers_modal")).frame(modal_frame()).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(RichText::new(t!("Your speakers")).size(20.0).strong());
            let summary = match s.phase {
                Phase::Demo => t!("These are made-up demo rooms; no speakers answered on this network.").to_string(),
                Phase::Searching => t!("Looking for speakers on your network…").to_string(),
                _ if s.reconnecting => t!("Reconnecting to your speakers…").to_string(),
                _ => t!("Speakers are checked every few seconds while the app is open.").to_string(),
            };
            ui.label(RichText::new(summary).size(12.5).color(TEXT_DIM()));
            ui.add_space(10.0);
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for h in &s.households {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t!("{system} system", system = h.system.label())).size(15.0).strong().color(TEXT()));
                        chip(ui, &tn!(h.rooms.len(), "1 room", "{n} rooms"), system_tint(h.system));
                    });
                    for r in &h.rooms {
                        let (color, state) = match (s.phase, r.online) {
                            (Phase::Demo, _) => (ACCENT(), t!("Demo").to_string()),
                            (_, Some(true)) => (
                                GOOD(),
                                match r.last_seen {
                                    Some(t) if t.elapsed().as_secs() < 5 => t!("Responding").to_string(),
                                    Some(t) => t!("Responding · last reply {n} s ago", n = t.elapsed().as_secs()),
                                    None => t!("Responding").to_string(),
                                },
                            ),
                            (_, Some(false)) => (BAD(), t!("Not responding").to_string()),
                            (_, None) => (TEXT_FAINT(), t!("Not checked yet").to_string()),
                        };
                        card_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (d, _) = ui.allocate_exact_size(vec2(12.0, 36.0), Sense::hover());
                                ui.painter().circle_filled(d.center(), 5.0, color);
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&r.name).size(14.5).color(TEXT()));
                                    let mut info = vec![];
                                    if !r.model.is_empty() {
                                        info.push(r.model.clone());
                                    }
                                    if !r.version.is_empty() {
                                        info.push(t!("version {version}", version = r.version));
                                    }
                                    if !r.ip.is_empty() {
                                        info.push(r.ip.clone());
                                    }
                                    ui.label(RichText::new(info.join(" · ")).size(12.0).color(TEXT_DIM()));
                                    ui.label(RichText::new(&state).size(12.0).color(color));
                                    if r.online == Some(false) && !r.problem.is_empty() {
                                        ui.add(egui::Label::new(RichText::new(&r.problem).size(11.0).color(TEXT_FAINT())).wrap());
                                    }
                                });
                            });
                        });
                        ui.add_space(6.0);
                    }
                    ui.add_space(6.0);
                }
                if s.households.is_empty() {
                    ui.label(RichText::new(t!("No speakers yet.")).color(TEXT_DIM()));
                }
            });
            if let Some(addr) = crate::sources::local::base_url() {
                ui.add_space(4.0);
                ui.label(RichText::new(t!("This computer plays your folders to the speakers from {address}", address = addr)).size(11.0).color(TEXT_FAINT()));
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if s.phase == Phase::Live && pill(ui, Some(Icon::Refresh), if s.reconnecting { t!("Reconnecting…") } else { t!("Reconnect") }, true).clicked() && !s.reconnecting
                {
                    self.out.push(Cmd::Reconnect);
                }
                if pill(ui, Some(Icon::Search), t!("Search network again"), false).clicked() {
                    self.out.push(Cmd::Discover);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Close"), false).clicked() {
                        close = true;
                    }
                });
            });
        });
        if modal.should_close() || close {
            self.speakers_open = false;
        }
    }

    fn station_modal(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.station_editor.take() else { return };
        let mut keep = true;
        let modal = egui::Modal::new(egui::Id::unique("station_editor")).frame(modal_frame()).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(RichText::new(t!("Add a station or stream")).size(20.0).strong());
            ui.label(RichText::new(t!("Paste the stream's web address. Playlist links (.pls, .m3u) are followed automatically.")).size(12.5).color(TEXT_DIM()));
            ui.add_space(10.0);
            egui::Grid::new("station_grid").num_columns(2).spacing([14.0, 10.0]).show(ui, |ui| {
                ui.label(RichText::new(t!("Name")).color(TEXT_DIM()));
                ui.add(egui::TextEdit::singleline(&mut st.name).hint_text(t!("My favourite station")).desired_width(280.0).margin(vec2(10.0, 7.0)));
                ui.end_row();
                ui.label(RichText::new(t!("Address")).color(TEXT_DIM()));
                ui.add(egui::TextEdit::singleline(&mut st.url).hint_text(t!("http://stream.example.com/live.mp3")).desired_width(280.0).margin(vec2(10.0, 7.0)));
                ui.end_row();
                ui.label(RichText::new(t!("Genre")).color(TEXT_DIM()));
                ui.add(egui::TextEdit::singleline(&mut st.genre).hint_text(t!("Optional, e.g. Jazz")).desired_width(280.0).margin(vec2(10.0, 7.0)));
                ui.end_row();
            });
            ui.add_space(14.0);
            let valid = st.url.trim().starts_with("http://") || st.url.trim().starts_with("https://");
            ui.horizontal(|ui| {
                if !valid && !st.url.trim().is_empty() {
                    ui.label(RichText::new(t!("The address should start with http:// or https://")).size(12.0).color(BAD()));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Save"), true).clicked() && valid {
                        self.out.push(Cmd::AddStation(st.clone()));
                        keep = false;
                    }
                    if pill(ui, None, t!("Cancel"), false).clicked() {
                        keep = false;
                    }
                });
            });
        });
        if modal.should_close() {
            keep = false;
        }
        if keep {
            self.station_editor = Some(st);
        }
    }

    /// "Update all speakers?" before anything is installed.
    fn update_modal(&mut self, ctx: &egui::Context) {
        let Some((hid, system, version)) = self.update_confirm.clone() else { return };
        let mut keep = true;
        let modal = egui::Modal::new(egui::Id::unique("update_confirm")).frame(modal_frame()).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(RichText::new(t!("Update your {system} system?", system = system.label())).size(20.0).strong());
            ui.add_space(6.0);
            ui.label(
                RichText::new(t!(
                    "Every speaker in this system downloads version {version} and restarts. Music stops for a few minutes. Keep the speakers plugged in until they're back.",
                    version = version
                ))
                .color(TEXT_DIM()),
            );
            if system == System::S1 {
                ui.add_space(4.0);
                ui.label(RichText::new(t!("This is an S1 update, so your speakers stay on S1.")).size(12.5).color(TEXT_FAINT()));
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Update now"), true).clicked() {
                        self.out.push(Cmd::InstallUpdate(hid.clone()));
                        keep = false;
                    }
                    if pill(ui, None, t!("Cancel"), false).clicked() {
                        keep = false;
                    }
                });
            });
        });
        if modal.should_close() || !keep {
            self.update_confirm = None;
        }
    }

    fn toasts(&mut self, ctx: &egui::Context, s: &AppState) {
        if s.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::unique("toasts")).anchor(egui::Align2::RIGHT_BOTTOM, vec2(-360.0, -108.0)).order(egui::Order::Foreground).interactable(false).show(ctx, |ui| {
            for t in &s.toasts {
                egui::Frame::new()
                    .fill(SURFACE_HI())
                    .corner_radius(12.0)
                    .stroke(Stroke::new(1.0_f32, if t.error { BAD().gamma_multiply(0.6) } else { LINE() }))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) })
                    .show(ui, |ui| {
                        ui.set_max_width(340.0);
                        ui.label(RichText::new(&t.text).size(13.0).color(TEXT()));
                    });
                ui.add_space(6.0);
            }
        });
    }

    /// Play now / next / later menu shared by tiles and rows.
    fn item_menu(&mut self, ui: &mut egui::Ui, item: &Item, gid: Option<&str>) {
        let Some(gid) = gid else {
            ui.label(t!("Pick a room first"));
            return;
        };
        let stream = item.is_stream();
        if ui.button(t!("▶  Play now")).clicked() {
            self.out.push(Cmd::PlayItem(gid.to_string(), item.clone(), PlayHow::Now));
            ui.close();
        }
        if !stream {
            if ui.button(t!("Play next")).clicked() {
                self.out.push(Cmd::PlayItem(gid.to_string(), item.clone(), PlayHow::Next));
                ui.close();
            }
            if ui.button(t!("Add to end of queue")).clicked() {
                self.out.push(Cmd::PlayItem(gid.to_string(), item.clone(), PlayHow::End));
                ui.close();
            }
            if ui.button(t!("Replace queue")).clicked() {
                self.out.push(Cmd::PlayItem(gid.to_string(), item.clone(), PlayHow::Replace));
                ui.close();
            }
        }
        if item.browsable() && ui.button(t!("Open")).clicked() {
            self.open(item);
            ui.close();
        }
        self.source_menu(ui, item);
    }

    /// Extra actions that depend on where an item came from.
    /// The saved favorite matching this item, if any (on the speakers or kept in the app).
    pub(super) fn favorite_of(&self, item: &Item) -> Option<String> {
        if item.is_favorite() || self.app_favs.contains(&item.id) && !item.id.is_empty() {
            return Some(item.id.clone());
        }
        if item.uri.is_empty() {
            return None;
        }
        self.speaker_favs.iter().find(|(u, _)| *u == item.uri).map(|(_, id)| id.clone())
    }

    /// Add or remove, whichever applies.
    pub(super) fn toggle_favorite(&mut self, item: &Item) {
        match self.favorite_of(item) {
            Some(id) => self.out.push(Cmd::RemoveFavorite(id, item.title.clone())),
            None if item.can_be_favorite() => self.out.push(Cmd::AddFavorite(self.current_group.clone(), item.clone())),
            None => {}
        }
    }

    /// The Favorites line of a menu.
    pub(super) fn favorite_menu_entry(&mut self, ui: &mut egui::Ui, item: &Item) {
        if self.favorite_of(item).is_some() {
            if ui.button(t!("★  Remove from Favorites")).clicked() {
                self.toggle_favorite(item);
                ui.close();
            }
        } else if item.can_be_favorite() && ui.button(t!("☆  Add to Favorites")).clicked() {
            self.toggle_favorite(item);
            ui.close();
        }
    }

    /// A star button for whatever is playing (hidden when it can't be a favorite).
    pub(super) fn now_star(&mut self, ui: &mut egui::Ui, now: &NowPlaying, size: f32) {
        let Some(item) = now.as_item() else { return };
        let saved = self.favorite_of(&item).is_some();
        let r =
            icon_button(ui, Icon::Star, size, if saved { ACCENT() } else { TEXT_DIM() }).on_hover_text(if saved { t!("Remove from Favorites") } else { t!("Add to Favorites") });
        if r.clicked() {
            self.toggle_favorite(&item);
        }
    }

    fn source_menu(&mut self, ui: &mut egui::Ui, item: &Item) {
        self.favorite_menu_entry(ui, item);
        if item.is_stream() && item.source != "mystations" && item.album.starts_with("http") && ui.button(t!("Save to My stations")).clicked() {
            self.out.push(Cmd::AddStation(crate::config::Station { name: item.title.clone(), url: item.album.clone(), genre: item.genre.clone() }));
            ui.close();
        }
        if item.source == "mystations" {
            if ui.button(t!("Edit station")).clicked() {
                self.station_editor = Some(crate::config::Station { name: item.title.clone(), url: item.album.clone(), genre: item.genre.clone() });
                ui.close();
            }
            if ui.button(t!("Remove from My stations")).clicked() {
                self.out.push(Cmd::RemoveStation(item.album.clone()));
                ui.close();
            }
        }
        if let Some(p) = crate::sources::podcasts::podcast_of(item) {
            let subscribed = self.subscribed.contains(&p.feed);
            if !subscribed && ui.button(t!("Subscribe")).clicked() {
                self.out.push(Cmd::Subscribe(p));
                ui.close();
            } else if subscribed && ui.button(t!("Unsubscribe")).clicked() {
                self.out.push(Cmd::Unsubscribe(p.feed));
                ui.close();
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        // Appearance: the person's setting, or a one-off --theme; Automatic follows the computer.
        let wanted = std::env::var("ROOMTONE_THEME").ok().unwrap_or_else(|| self.backend.lock().config.theme.clone());
        if wanted != self.applied_theme {
            theme::set_appearance(ctx, &wanted);
            self.applied_theme = wanted;
        }
        theme::sync(ctx);
        self.handle_screenshot(ctx);
        #[cfg(target_os = "macos")]
        crate::macos_about::install();
        let backend = self.backend.clone();
        {
            let s = backend.lock();
            if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                if let Some((_, g)) = s.selected_group.as_ref().and_then(|g| s.find_group(g)) {
                    let cmd = if g.now.transport == Transport::Playing { Cmd::Pause(g.id.clone()) } else { Cmd::Play(g.id.clone()) };
                    self.out.push(cmd);
                }
            }
            self.subscribed = s.config.podcasts.iter().map(|p| p.feed.clone()).collect();
            self.current_group = s.selected_group.clone();
            self.speaker_favs = match s.browse.get("FV:2") {
                Some(Browse::Ready(v)) => v.iter().filter(|f| !f.uri.is_empty()).map(|f| (f.uri.clone(), f.id.clone())).collect(),
                _ => vec![],
            };
            self.app_favs = s.config.favorites.iter().map(|f| f.id.clone()).collect();
            self.along = s
                .households
                .iter()
                .flat_map(|h| h.groups.iter())
                .filter_map(|g| {
                    let other = s.linked_with(&g.id)?;
                    let (oh, og) = s.find_group(other)?;
                    Some((g.id.clone(), format!("{} · {}", oh.system.label(), oh.group_name(og))))
                })
                .collect();
            self.draw(root, &s);
        }
        for cmd in std::mem::take(&mut self.out) {
            backend.send(cmd);
        }
        for note in std::mem::take(&mut self.notes) {
            backend.lock().toast(note, false);
        }
        if let Some(g) = self.select.take() {
            backend.lock().selected_group = Some(g);
        }
        if let Some(h) = self.pick_household.take() {
            let mut s = backend.lock();
            if s.active != h {
                s.active = h;
                s.selected_group = None;
                s.ensure_selection();
                if s.phase == Phase::Live {
                    s.browse.clear();
                    s.alarms = None;
                    self.asked.clear();
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn modal_frame() -> egui::Frame {
    egui::Frame::new().fill(SURFACE()).corner_radius(18.0).inner_margin(22.0).stroke(Stroke::new(1.0_f32, LINE()))
}

fn nav_item(ui: &mut egui::Ui, icon: Icon, label: &str, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, 10.0, SURFACE_HI());
        ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 10.0), vec2(3.0, 18.0)), 2.0, ACCENT());
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 10.0, SURFACE());
    }
    let color = if active { TEXT() } else { TEXT_DIM() };
    paint_icon(ui.painter(), Rect::from_min_size(rect.min + vec2(14.0, 10.0), vec2(18.0, 18.0)), icon, if active { ACCENT() } else { color });
    ui.painter().text(pos2(rect.min.x + 44.0, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.5), color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn system_item(ui: &mut egui::Ui, label: &str, sub: &str, tint: Color32, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, 10.0, SURFACE_HI());
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 10.0, SURFACE());
    }
    let badge = Rect::from_min_size(rect.min + vec2(10.0, 10.0), vec2(28.0, 28.0));
    ui.painter().rect_filled(badge, 8.0, tint.gamma_multiply(if active { 0.9 } else { 0.25 }));
    let short = if label.starts_with("S1") { "S1" } else { "S2" };
    ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, short, egui::FontId::proportional(11.5), if active { Color32::BLACK } else { tint });
    ui.painter().text(pos2(rect.min.x + 48.0, rect.min.y + 16.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.0), if active { TEXT() } else { TEXT_DIM() });
    ui.painter().text(pos2(rect.min.x + 48.0, rect.min.y + 33.0), egui::Align2::LEFT_CENTER, sub, egui::FontId::proportional(11.5), TEXT_FAINT());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Paint one line of text at `pos`, cut with an ellipsis at `width`.
pub fn paint_text(ui: &egui::Ui, pos: egui::Pos2, text: &str, size: f32, color: Color32, width: f32) {
    if text.is_empty() || width <= 4.0 {
        return;
    }
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat { font_id: egui::FontId::proportional(size), color, ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: width, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(pos, galley, color);
}

fn chip_at(ui: &egui::Ui, center: egui::Pos2, text: &str) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(10.5), Color32::BLACK);
    let r = Rect::from_center_size(center, galley.size() + vec2(12.0, 4.0));
    ui.painter().rect_filled(r, 8.0, ACCENT());
    ui.painter().galley(r.min + vec2(6.0, 2.0), galley, Color32::BLACK);
}
