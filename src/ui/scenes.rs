//! Scenes: a page that lists them with *Do it*, *Edit* and *Delete*, a row of them on Home, and
//! the window for making or changing one.

use eframe::egui::{self, vec2, Align, Layout, RichText, Ui};

use super::theme::*;
use super::widgets::*;
use super::{modal_frame, App, View};
use crate::backend::Cmd;
use crate::config::{Scene, SceneRoom};
use crate::model::*;

/// One line saying what a scene does.
fn summary(sc: &Scene) -> String {
    let rooms: Vec<String> = sc
        .rooms
        .iter()
        .map(|r| match r.volume {
            Some(v) => t!("{room} at {volume}", room = r.room, volume = v),
            None => r.room.clone(),
        })
        .collect();
    let mut parts = vec![rooms.join(", ")];
    if !sc.play.is_empty() {
        parts.push(t!("plays {name}", name = sc.play));
    }
    if sc.sleep_minutes > 0 {
        parts.push(t!("stops after {n} min", n = sc.sleep_minutes));
    }
    parts.join(" · ")
}

impl App {
    /// The scenes of the system being looked at (a scene made with no system shows everywhere).
    fn scenes_here<'a>(&self, s: &'a AppState) -> Vec<&'a Scene> {
        let hid = s.household().map(|h| h.id.clone()).unwrap_or_default();
        s.config.scenes.iter().filter(|sc| sc.household.is_empty() || sc.household == hid).collect()
    }

    pub(super) fn new_scene(&mut self, s: &AppState) -> Scene {
        let household = s.household().map(|h| h.id.clone()).unwrap_or_default();
        Scene { name: String::new(), household, rooms: vec![], group: true, play: String::new(), sleep_minutes: 0 }
    }

    /// A short row of scene buttons for Home.
    pub(super) fn scenes_row(&mut self, ui: &mut Ui, s: &AppState) {
        let scenes = self.scenes_here(s);
        if scenes.is_empty() {
            return;
        }
        if self.section_with_link(ui, t!("Scenes"), t!("Manage")) {
            self.go(View::Scenes);
        }
        ui.horizontal_wrapped(|ui| {
            for sc in scenes {
                if pill(ui, Some(Icon::Sliders), &sc.name, false).on_hover_text(summary(sc)).clicked() {
                    self.send(Cmd::ApplyScene(sc.name.clone()));
                }
            }
        });
        ui.add_space(18.0);
    }

    pub(super) fn scenes_page(&mut self, ui: &mut Ui, s: &AppState) {
        self.header(ui, t!("Scenes"), t!("One click sets up your rooms: who plays together, how loud, what plays, and when it stops"));
        if pill(ui, Some(Icon::Plus), t!("New scene"), true).clicked() {
            let sc = self.new_scene(s);
            self.scene_editor = Some((None, sc));
        }
        ui.add_space(12.0);
        let scenes: Vec<Scene> = self.scenes_here(s).into_iter().cloned().collect();
        if scenes.is_empty() {
            ui.label(
                RichText::new(t!("No scenes yet. A scene might be “Movie night”: the living room and kitchen together, quiet, playing a playlist, stopping after two hours."))
                    .color(TEXT_DIM()),
            );
        }
        for sc in scenes {
            card_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width((ui.available_width() - 270.0).max(160.0));
                        ui.label(RichText::new(&sc.name).size(16.0).color(TEXT()));
                        ui.add(egui::Label::new(RichText::new(summary(&sc)).size(12.5).color(TEXT_DIM())).wrap());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, Icon::Trash, 16.0, TEXT_DIM()).on_hover_text(t!("Delete scene")).clicked() {
                            self.send(Cmd::DeleteScene(sc.name.clone()));
                        }
                        if pill(ui, None, t!("Edit"), false).clicked() {
                            self.scene_editor = Some((Some(sc.name.clone()), sc.clone()));
                        }
                        if pill(ui, Some(Icon::Play), t!("Do it"), true).clicked() {
                            self.send(Cmd::ApplyScene(sc.name.clone()));
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
    }

    /// The window for making or changing a scene.
    pub(super) fn scene_modal(&mut self, ctx: &egui::Context, s: &AppState) {
        let Some((old, mut sc)) = self.scene_editor.take() else { return };
        let Some(h) = s.household() else { return };
        // Make sure the favorites and playlists it can start are being loaded.
        let _ = (self.browse(s, "FV:2"), self.browse(s, "SQ:"));
        // The things it can start: favorites and playlists (loaded when the window opens).
        let mut playable: Vec<String> = vec![];
        for id in ["FV:2", "SQ:"] {
            if let Some(Browse::Ready(v)) = s.browse.get(id) {
                playable.extend(v.iter().map(|i| i.title.clone()));
            }
        }
        playable.extend(s.config.favorites.iter().map(|f| f.title.clone()));
        playable.sort_by_key(|n| n.to_lowercase());
        playable.dedup();
        let mut keep = true;
        let mut save = false;
        let modal = egui::Modal::new(egui::Id::unique("scene_modal")).frame(modal_frame()).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(RichText::new(if old.is_some() { t!("Edit scene") } else { t!("New scene") }).size(20.0).strong());
            ui.add_space(8.0);
            ui.add(egui::TextEdit::singleline(&mut sc.name).hint_text(t!("Name, e.g. Movie night")).desired_width(f32::INFINITY).margin(vec2(10.0, 7.0)));
            ui.add_space(10.0);
            ui.label(RichText::new(t!("Rooms")).strong().color(TEXT()));
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for r in &h.rooms {
                    let pos = sc.rooms.iter().position(|x| x.room == r.name);
                    let mut on = pos.is_some();
                    ui.horizontal(|ui| {
                        ui.set_min_height(30.0);
                        if toggle(ui, &mut on, &r.name).changed() {
                            if on {
                                sc.rooms.push(SceneRoom { room: r.name.clone(), volume: None });
                            } else if let Some(p) = pos {
                                sc.rooms.remove(p);
                            }
                        }
                        ui.label(RichText::new(&r.name).color(TEXT()));
                        if let Some(p) = sc.rooms.iter().position(|x| x.room == r.name) {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let mut set = sc.rooms[p].volume.is_some();
                                let mut v = sc.rooms[p].volume.unwrap_or(r.volume) as f32 / 100.0;
                                let (_, changed) = slider(ui, ("scene_vol", &r.uuid), t!("Volume"), &mut v, 120.0, if set { ACCENT() } else { TEXT_FAINT() });
                                if changed {
                                    set = true;
                                    sc.rooms[p].volume = Some((v * 100.0).round() as u8);
                                }
                                if ui.checkbox(&mut set, t!("Set volume")).changed() {
                                    sc.rooms[p].volume = set.then(|| (v * 100.0).round() as u8);
                                }
                            });
                        }
                    });
                }
            });
            ui.add_space(8.0);
            let mut together = sc.group;
            if ui.checkbox(&mut together, t!("Play these rooms together")).changed() {
                sc.group = together;
            }
            ui.add_space(8.0);
            ui.label(RichText::new(t!("Start")).strong().color(TEXT()));
            let current = if sc.play.is_empty() { t!("Nothing: leave what's playing").to_string() } else { sc.play.clone() };
            egui::ComboBox::from_id_salt("scene_play").selected_text(current).width(320.0).show_ui(ui, |ui| {
                if ui.selectable_label(sc.play.is_empty(), t!("Nothing: leave what's playing")).clicked() {
                    sc.play.clear();
                }
                for name in &playable {
                    if ui.selectable_label(&sc.play == name, name).clicked() {
                        sc.play = name.clone();
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let mut timer = sc.sleep_minutes > 0;
                if ui.checkbox(&mut timer, t!("Stop the music after")).changed() {
                    sc.sleep_minutes = if timer { 60 } else { 0 };
                }
                if sc.sleep_minutes > 0 {
                    ui.add(egui::DragValue::new(&mut sc.sleep_minutes).range(1..=1440).suffix(format!(" {}", t!("minutes"))));
                }
            });
            ui.add_space(14.0);
            let valid = !sc.name.trim().is_empty() && !sc.rooms.is_empty();
            ui.horizontal(|ui| {
                if !valid {
                    ui.label(RichText::new(t!("Give it a name and pick at least one room.")).size(12.0).color(TEXT_FAINT()));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Save"), true).clicked() && valid {
                        save = true;
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
        if save {
            sc.name = sc.name.trim().to_string();
            // Rooms in the order the system lists them, so the first one leads.
            let order: Vec<&str> = h.rooms.iter().map(|r| r.name.as_str()).collect();
            sc.rooms.sort_by_key(|r| order.iter().position(|n| *n == r.room).unwrap_or(usize::MAX));
            self.send(Cmd::SaveScene(old.clone(), sc.clone()));
        }
        if keep {
            self.scene_editor = Some((old, sc));
        }
    }
}
