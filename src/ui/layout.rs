//! Speaker setup (experimental): make and split stereo pairs, and bond a sub or rear speakers to
//! a soundbar and take them off again. Nothing happens without the switch being on and a
//! confirmation window, and every change can be undone from the same card.

use eframe::egui::{self, Align, Layout, RichText, Ui};

use super::pages::experimental_title;
use super::theme::*;
use super::widgets::*;
use super::{modal_frame, App};
use crate::backend::Cmd;
use crate::layout::{is_sub, pairable, Role};
use crate::model::*;

/// The choices made so far in the card (room ids).
#[derive(Default)]
pub struct Form {
    left: String,
    right: String,
    bar: String,
    sub: String,
    rear_left: String,
    rear_right: String,
}

impl Form {
    /// For picture checks: choose the soundbar.
    pub(super) fn set_bar(&mut self, uuid: &str) {
        self.bar = uuid.to_string();
    }
}

fn name_of(h: &Household, uuid: &str) -> String {
    h.rooms.iter().find(|r| r.uuid == uuid).map(|r| r.name.clone()).unwrap_or_else(|| t!("Unknown room").into())
}

fn role_label(r: Role) -> String {
    match r {
        Role::Sub => t!("Sub").into(),
        Role::RearLeft => t!("Rear left").into(),
        Role::RearRight => t!("Rear right").into(),
    }
}

/// A drop-down of rooms; `current` holds the chosen room's id (empty for none yet).
fn room_combo(ui: &mut Ui, id: &str, current: &mut String, rooms: &[(String, String)]) {
    let shown = rooms.iter().find(|(u, _)| u == current).map(|(_, n)| n.clone()).unwrap_or_else(|| t!("Choose a room").into());
    egui::ComboBox::from_id_salt(id).width(200.0).selected_text(shown).show_ui(ui, |ui| {
        for (uuid, name) in rooms {
            ui.selectable_value(current, uuid.clone(), name);
        }
    });
    if !current.is_empty() && !rooms.iter().any(|(u, _)| u == current) {
        current.clear();
    }
}

impl App {
    /// One plain line about a layout change, for the confirmation window.
    pub(super) fn step_line(&self, s: &AppState, cmd: &Cmd) -> String {
        let name = |uuid: &str| {
            s.find_room(uuid)
                .map(|(_, r)| r.name.clone())
                .or_else(|| s.layout.as_ref().and_then(|l| l.pairs.iter().find(|p| p.left == uuid || p.right == uuid).map(|p| p.name.clone())))
                .unwrap_or_else(|| t!("a speaker").to_string())
        };
        match cmd {
            Cmd::CreatePair(l, _) => t!("Make {name} a stereo pair", name = name(l)),
            Cmd::AddSatellites(bar, add) => {
                let what: Vec<String> = add.iter().map(|(_, r)| role_label(*r)).collect();
                t!("Add {what} to {bar}", what = what.join(", "), bar = name(bar))
            }
            _ => String::new(),
        }
    }

    pub(super) fn layout_card(&mut self, ui: &mut Ui, s: &AppState) {
        let Some(h) = s.household() else { return };
        if !h.is_set_up() {
            return;
        }
        experimental_title(ui, t!("Speaker setup"));
        let mut allow = s.config.allow_layout;
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(40.0);
                ui.vertical(|ui| {
                    ui.set_max_width((ui.available_width() - 76.0).max(120.0));
                    ui.label(RichText::new(t!("Allow changing speaker setup")).color(TEXT()));
                    ui.label(
                        RichText::new(t!("Make or split stereo pairs, and add a sub or rear speakers to a soundbar. Not tried on real speakers yet: it can clear a room's Trueplay tuning, and music in the rooms stops."))
                            .size(12.0)
                            .color(TEXT_DIM()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle(ui, &mut allow, t!("Allow changing speaker setup"));
                });
            });
            if allow {
                ui.separator();
                self.layout_body(ui, s, h);
            }
        });
        if allow != s.config.allow_layout {
            self.send(Cmd::SetFeature(crate::config::Feature::Layout, allow));
        }
    }

    fn layout_body(&mut self, ui: &mut Ui, s: &AppState, h: &Household) {
        self.ask_once("layout", std::time::Duration::from_secs(15), Cmd::LoadLayout);
        let bonds = s.layout.clone().unwrap_or_default();
        let taken = bonds.bonded_uuids();
        let free: Vec<&Room> = h.rooms.iter().filter(|r| !taken.contains(&r.uuid)).collect();

        // ---- stereo pairs ----
        ui.label(RichText::new(t!("Stereo pairs")).strong().color(TEXT()));
        if bonds.pairs.is_empty() {
            ui.label(RichText::new(t!("No stereo pairs yet.")).size(12.5).color(TEXT_DIM()));
        }
        for p in &bonds.pairs {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("{room}: a left and a right speaker", room = p.name)).color(TEXT()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Split"), false).clicked() {
                        self.layout_confirm =
                            Some((t!("Split {room} into two rooms?", room = p.name), t!("The pair becomes two separate rooms again.").to_string(), Cmd::SplitPair(p.left.clone())));
                    }
                });
            });
        }
        let lefts: Vec<(String, String)> = free.iter().map(|r| (r.uuid.clone(), r.name.clone())).collect();
        let left_model = free.iter().find(|r| r.uuid == self.layout_form.left).map(|r| r.model.clone()).unwrap_or_default();
        let rights: Vec<(String, String)> =
            free.iter().filter(|r| r.uuid != self.layout_form.left && pairable(&left_model, &r.model)).map(|r| (r.uuid.clone(), r.name.clone())).collect();
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t!("Left")).color(TEXT_DIM()));
            room_combo(ui, "pair_left", &mut self.layout_form.left, &lefts);
            ui.label(RichText::new(t!("Right")).color(TEXT_DIM()));
            room_combo(ui, "pair_right", &mut self.layout_form.right, &rights);
            if !self.layout_form.left.is_empty() && !self.layout_form.right.is_empty() && pill(ui, None, t!("Make a pair"), false).clicked() {
                let (l, r) = (self.layout_form.left.clone(), self.layout_form.right.clone());
                self.layout_confirm = Some((
                    t!("Make {left} and {right} a stereo pair?", left = name_of(h, &l), right = name_of(h, &r)),
                    t!("They become one room, named after the left speaker. Only two identical speakers can be paired.").to_string(),
                    Cmd::CreatePair(l, r),
                ));
            }
        });
        if !self.layout_form.left.is_empty() && rights.is_empty() {
            ui.label(RichText::new(t!("No other room is the same model, so there is nothing to pair it with.")).size(12.5).color(TEXT_DIM()));
        }

        // ---- soundbar speakers ----
        ui.add_space(10.0);
        ui.label(RichText::new(t!("Soundbar speakers")).strong().color(TEXT()));
        for set in &bonds.sets {
            ui.label(RichText::new(set.name.clone()).color(TEXT()));
            for sat in &set.satellites {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new(t!("{role}: {room}", role = role_label(sat.role), room = sat.name)).color(TEXT_DIM()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if pill(ui, None, t!("Remove"), false).clicked() {
                            self.layout_confirm = Some((
                                t!("Take the {role} off {bar}?", role = role_label(sat.role), bar = set.name),
                                t!("It becomes a room of its own again.").to_string(),
                                Cmd::RemoveSatellites(set.bar.clone(), vec![sat.uuid.clone()]),
                            ));
                        }
                    });
                });
            }
        }
        let bars: Vec<(String, String)> = free.iter().filter(|r| model_is_soundbar(&r.model)).map(|r| (r.uuid.clone(), r.name.clone())).collect();
        let bars: Vec<(String, String)> = bars.into_iter().chain(bonds.sets.iter().map(|b| (b.bar.clone(), b.name.clone()))).collect();
        if bars.is_empty() {
            ui.label(RichText::new(t!("No soundbar found in this system.")).size(12.5).color(TEXT_DIM()));
            return;
        }
        let subs: Vec<(String, String)> = free.iter().filter(|r| is_sub(&r.model)).map(|r| (r.uuid.clone(), r.name.clone())).collect();
        let others: Vec<(String, String)> = free.iter().filter(|r| !model_is_soundbar(&r.model) && !is_sub(&r.model)).map(|r| (r.uuid.clone(), r.name.clone())).collect();
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t!("Soundbar")).color(TEXT_DIM()));
            room_combo(ui, "bar", &mut self.layout_form.bar, &bars);
        });
        if self.layout_form.bar.is_empty() {
            return;
        }
        let bar = self.layout_form.bar.clone();
        let bar_name = name_of(h, &bar);
        let rears_there = bonds.set_for(&bar).is_some_and(|b| b.satellites.iter().any(|x| x.role != Role::Sub));
        let sub_there = bonds.set_for(&bar).is_some_and(|b| b.satellites.iter().any(|x| x.role == Role::Sub));
        if !sub_there {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(t!("Sub")).color(TEXT_DIM()));
                room_combo(ui, "sub", &mut self.layout_form.sub, &subs);
                if !self.layout_form.sub.is_empty() && pill(ui, None, t!("Add the sub"), false).clicked() {
                    let sub = self.layout_form.sub.clone();
                    self.layout_confirm = Some((
                        t!("Add {room} as the sub of {bar}?", room = name_of(h, &sub), bar = bar_name.clone()),
                        t!("It plays the low sounds of {bar} and stops being a room of its own.", bar = bar_name.clone()),
                        Cmd::AddSatellites(bar.clone(), vec![(sub, Role::Sub)]),
                    ));
                }
            });
            if subs.is_empty() {
                ui.label(RichText::new(t!("No sub is free to add.")).size(12.5).color(TEXT_DIM()));
            }
        }
        if !rears_there {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(t!("Rear left")).color(TEXT_DIM()));
                room_combo(ui, "rear_left", &mut self.layout_form.rear_left, &others);
                ui.label(RichText::new(t!("Rear right")).color(TEXT_DIM()));
                let rights: Vec<(String, String)> = others.iter().filter(|(u, _)| *u != self.layout_form.rear_left).cloned().collect();
                room_combo(ui, "rear_right", &mut self.layout_form.rear_right, &rights);
                if !self.layout_form.rear_left.is_empty() && !self.layout_form.rear_right.is_empty() && pill(ui, None, t!("Add rear speakers"), false).clicked() {
                    let (l, r) = (self.layout_form.rear_left.clone(), self.layout_form.rear_right.clone());
                    self.layout_confirm = Some((
                        t!("Add {left} and {right} as the rear speakers of {bar}?", left = name_of(h, &l), right = name_of(h, &r), bar = bar_name.clone()),
                        t!("They play the surround sounds of {bar} and stop being rooms of their own. Use two identical speakers.", bar = bar_name.clone()),
                        Cmd::AddSatellites(bar.clone(), vec![(l, Role::RearLeft), (r, Role::RearRight)]),
                    ));
                }
            });
        }
    }

    /// "Are you sure?" before anything is bonded or split.
    pub(super) fn layout_modal(&mut self, ctx: &egui::Context) {
        let Some((title, what, cmd)) = self.layout_confirm.clone() else { return };
        let mut keep = true;
        let modal = egui::Modal::new(egui::Id::unique("layout_confirm")).frame(modal_frame()).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label(RichText::new(title).size(20.0).strong().color(TEXT()));
            ui.add_space(6.0);
            ui.label(RichText::new(what).color(TEXT_DIM()));
            ui.add_space(6.0);
            ui.label(
                RichText::new(t!("This can clear a room's measured Trueplay tuning, and music in these rooms stops. You can undo it in Settings › Speakers › Speaker setup."))
                    .size(12.5)
                    .color(TEXT_DIM()),
            );
            ui.add_space(4.0);
            ui.label(RichText::new(t!("Not tried on real speakers yet.")).size(12.5).color(TEXT_FAINT()));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill(ui, None, t!("Do it"), true).clicked() {
                        self.out.push(cmd.clone());
                        self.layout_form = Form::default();
                        keep = false;
                    }
                    if pill(ui, None, t!("Cancel"), false).clicked() {
                        keep = false;
                    }
                });
            });
        });
        if modal.should_close() || !keep {
            self.layout_confirm = None;
        }
    }
}
