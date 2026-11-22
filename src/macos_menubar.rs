//! macOS only: Roomtone in the menu bar.
//!
//! A speaker icon at the top of the screen opens a menu: what's playing in the chosen room, play
//! or pause, next and previous, volume, the list of rooms to choose from, *Pause all rooms*,
//! *Open Roomtone* and *Quit*. The menu is built each time it is opened (from `menuNeedsUpdate:`),
//! so it is always current and nothing has to be kept up to date while the window is closed.
//! With *Keep running when the window is closed* on, closing the window hides the app instead of
//! quitting it (`hide_app`), and the menu is how to get it back.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::NSString;

use crate::backend::{Backend, Cmd};
use crate::model::Transport;

/// What a menu item does. Items carry their place in the list as their tag.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    PlayPause(String),
    Next(String),
    Previous(String),
    Louder(String),
    Quieter(String),
    Mute(String),
    Select(String),
    PauseAll(Vec<String>),
    Open,
    Quit,
}

/// What the menu shows, worked out from the app's state (kept apart from the menu itself so it
/// can be tested): each entry is a title and what choosing it does (`None` is a plain label),
/// with `Sep` between sections.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Label(String),
    Item(String, Action, bool),
    Sub(String, Vec<Entry>),
    Sep,
}

/// The menu for the current state.
pub fn entries(s: &crate::model::AppState) -> Vec<Entry> {
    use Entry::*;
    let mut out = vec![];
    let Some(h) = s.household() else {
        out.push(Label(crate::t!("No speakers found yet").into()));
        out.push(Sep);
        out.push(Item(crate::t!("Open Roomtone").into(), Action::Open, false));
        out.push(Item(crate::t!("Quit Roomtone").into(), Action::Quit, false));
        return out;
    };
    let chosen = s.selected_group.as_ref().and_then(|g| h.groups.iter().find(|x| &x.id == g)).or_else(|| h.groups.first());
    match chosen {
        Some(g) => {
            let room = h.group_name(g);
            let playing = g.now.transport == Transport::Playing;
            if g.now.title.is_empty() {
                out.push(Label(crate::t!("Nothing playing").into()));
            } else {
                out.push(Label(g.now.title.clone()));
                if !g.now.artist.is_empty() {
                    out.push(Label(g.now.artist.clone()));
                }
            }
            out.push(Label(room));
            out.push(Sep);
            out.push(Item(if playing { crate::t!("Pause").into() } else { crate::t!("Play").into() }, Action::PlayPause(g.id.clone()), false));
            out.push(Item(crate::t!("Next").into(), Action::Next(g.id.clone()), false));
            out.push(Item(crate::t!("Previous").into(), Action::Previous(g.id.clone()), false));
            out.push(Sub(
                crate::t!("Volume {n}", n = if g.muted { 0 } else { g.volume }),
                vec![
                    Item(crate::t!("Louder").into(), Action::Louder(g.id.clone()), false),
                    Item(crate::t!("Quieter").into(), Action::Quieter(g.id.clone()), false),
                    Item(crate::t!("Mute").into(), Action::Mute(g.id.clone()), g.muted),
                ],
            ));
            out.push(Sep);
        }
        None => {
            out.push(Label(crate::t!("No rooms found yet").into()));
            out.push(Sep);
        }
    }
    if h.groups.len() > 1 {
        out.push(Label(crate::t!("Rooms").into()));
        for g in &h.groups {
            out.push(Item(h.group_name(g), Action::Select(g.id.clone()), chosen.is_some_and(|c| c.id == g.id)));
        }
        out.push(Sep);
    }
    let playing: Vec<String> = h.groups.iter().filter(|g| g.now.transport == Transport::Playing).map(|g| g.id.clone()).collect();
    if !playing.is_empty() {
        out.push(Item(crate::t!("Pause all rooms").into(), Action::PauseAll(playing), false));
    }
    out.push(Item(crate::t!("Open Roomtone").into(), Action::Open, false));
    out.push(Item(crate::t!("Quit Roomtone").into(), Action::Quit, false));
    out
}

/// Carry out an action.
pub fn run(b: &Backend, a: Action) {
    match a {
        Action::PlayPause(g) => {
            let playing = b.lock().find_group(&g).is_some_and(|(_, x)| x.now.transport == Transport::Playing);
            let _ = b.send(if playing { Cmd::Pause(g) } else { Cmd::Play(g) });
        }
        Action::Next(g) => {
            let _ = b.send(Cmd::Next(g));
        }
        Action::Previous(g) => {
            let _ = b.send(Cmd::Previous(g));
        }
        Action::Louder(g) => nudge(b, g, true),
        Action::Quieter(g) => nudge(b, g, false),
        Action::Mute(g) => {
            let muted = b.lock().find_group(&g).is_some_and(|(_, x)| x.muted);
            let _ = b.send(Cmd::GroupMute(g, !muted));
        }
        Action::Select(g) => b.lock().selected_group = Some(g),
        Action::PauseAll(gs) => {
            for g in gs {
                let _ = b.send(Cmd::Pause(g));
            }
        }
        Action::Open => show_app(),
        Action::Quit => {
            if let Some(mtm) = MainThreadMarker::new() {
                NSApplication::sharedApplication(mtm).terminate(None);
            }
        }
    }
}

/// Turn a group up or down by the volume step setting.
fn nudge(b: &Backend, g: String, up: bool) {
    let (now, step) = {
        let s = b.lock();
        (s.find_group(&g).map(|(_, x)| x.volume).unwrap_or(0), s.config.volume_step.max(1))
    };
    let v = if up { now.saturating_add(step).min(100) } else { now.saturating_sub(step) };
    let _ = b.send(Cmd::GroupVolume(g, v));
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this class doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RoomtoneMenuBarTarget"]
    struct Target;

    unsafe impl NSObjectProtocol for Target {}

    unsafe impl NSMenuDelegate for Target {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            if let Some(mtm) = MainThreadMarker::new() {
                rebuild(mtm, menu);
            }
        }
    }

    impl Target {
        #[unsafe(method(act:))]
        fn act(&self, sender: &NSMenuItem) {
            let tag = sender.tag();
            let action = ACTIONS.with(|a| a.borrow().get(tag as usize).cloned());
            let backend = BACKEND.with(|b| b.borrow().clone());
            if let (Some(action), Some(backend)) = (action, backend) {
                run(&backend, action);
            }
        }
    }
);

thread_local! {
    /// Menu items only point weakly at their target, so keep it alive here.
    static TARGET: RefCell<Option<Retained<Target>>> = const { RefCell::new(None) };
    static ITEM: RefCell<Option<Retained<NSStatusItem>>> = const { RefCell::new(None) };
    static BACKEND: RefCell<Option<Backend>> = const { RefCell::new(None) };
    /// The actions of the menu as last built, by item tag.
    static ACTIONS: RefCell<Vec<Action>> = const { RefCell::new(Vec::new()) };
}

fn target(mtm: MainThreadMarker) -> Retained<Target> {
    TARGET.with(|slot| {
        if slot.borrow().is_none() {
            let this = mtm.alloc::<Target>().set_ivars(());
            let obj: Retained<Target> = unsafe { msg_send![super(this), init] };
            *slot.borrow_mut() = Some(obj);
        }
        slot.borrow().clone().expect("just made")
    })
}

fn make_item(mtm: MainThreadMarker, title: &str, action: Option<(usize, bool)>) -> Retained<NSMenuItem> {
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(mtm.alloc(), &NSString::from_str(title), action.map(|_| sel!(act:)), &NSString::from_str("")) };
    match action {
        Some((tag, ticked)) => {
            unsafe { item.setTarget(Some(&target(mtm))) };
            item.setTag(tag as isize);
            item.setEnabled(true);
            item.setState(if ticked { 1 } else { 0 });
        }
        None => item.setEnabled(false),
    }
    item
}

fn fill(mtm: MainThreadMarker, menu: &NSMenu, list: &[Entry], actions: &mut Vec<Action>) {
    for e in list {
        match e {
            Entry::Sep => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            Entry::Label(t) => menu.addItem(&make_item(mtm, t, None)),
            Entry::Item(t, a, ticked) => {
                actions.push(a.clone());
                let item = make_item(mtm, t, Some((actions.len() - 1, *ticked)));
                if *a == Action::Quit {
                    item.setKeyEquivalent(&NSString::from_str("q"));
                }
                menu.addItem(&item);
            }
            Entry::Sub(t, inner) => {
                let item = make_item(mtm, t, None);
                item.setEnabled(true);
                let sub = NSMenu::new(mtm);
                sub.setAutoenablesItems(false);
                fill(mtm, &sub, inner, actions);
                item.setSubmenu(Some(&sub));
                menu.addItem(&item);
            }
        }
    }
}

fn rebuild(mtm: MainThreadMarker, menu: &NSMenu) {
    let Some(backend) = BACKEND.with(|b| b.borrow().clone()) else { return };
    let list = entries(&backend.lock());
    menu.removeAllItems();
    let mut actions = vec![];
    fill(mtm, menu, &list, &mut actions);
    ACTIONS.with(|a| *a.borrow_mut() = actions);
}

/// Call every frame (cheap): puts the icon in the menu bar while `on`, and takes it away when not.
pub fn install(backend: &Backend, on: bool) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let have = ITEM.with(|i| i.borrow().is_some());
    if on == have {
        return;
    }
    if !on {
        if let Some(item) = ITEM.with(|i| i.borrow_mut().take()) {
            NSStatusBar::systemStatusBar().removeStatusItem(&item);
        }
        return;
    }
    BACKEND.with(|b| *b.borrow_mut() = Some(backend.clone()));
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = item.button(mtm) {
        let name = NSString::from_str("hifispeaker.fill");
        let description = NSString::from_str("Roomtone");
        match NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, Some(&description)) {
            Some(img) => {
                img.setTemplate(true);
                button.setImage(Some(&img));
            }
            None => button.setTitle(&NSString::from_str("♪")),
        }
        button.setToolTip(Some(&NSString::from_str("Roomtone")));
    }
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let t = target(mtm);
    menu.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*t)));
    rebuild(mtm, &menu);
    if std::env::var_os("ROOMTONE_TEST_MENUBAR").is_some() {
        eprintln!("MENUBAR: icon added, {} items, button: {}", menu.numberOfItems(), item.button(mtm).is_some());
    }
    item.setMenu(Some(&menu));
    ITEM.with(|i| *i.borrow_mut() = Some(item));
}

/// Hide the app (as Hide does), so closing the window leaves Roomtone running in the menu bar.
pub fn hide_app() {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).hide(None::<&AnyObject>);
    }
}

/// Bring the window back to the front.
pub fn show_app() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    app.unhide(None::<&AnyObject>);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    for w in app.windows().iter() {
        if w.isMiniaturized() {
            w.deminiaturize(None::<&AnyObject>);
        }
        w.makeKeyAndOrderFront(None::<&AnyObject>);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(list: &[Entry]) -> Vec<String> {
        list.iter()
            .map(|e| match e {
                Entry::Label(t) => format!("· {t}"),
                Entry::Item(t, _, ticked) => format!("{}{t}", if *ticked { "✓ " } else { "" }),
                Entry::Sub(t, _) => format!("> {t}"),
                Entry::Sep => "---".into(),
            })
            .collect()
    }

    #[test]
    fn the_menu_shows_what_is_playing_the_rooms_and_the_ways_out() {
        let b = crate::test_support::demo_backend();
        let list = entries(&b.lock());
        let text = flat(&list);
        assert!(text.iter().any(|l| l == "Pause"), "something is playing in the chosen room: {text:?}");
        assert!(text.iter().any(|l| l.starts_with("> Volume ")), "{text:?}");
        assert!(text.contains(&"Open Roomtone".to_string()) && text.contains(&"Quit Roomtone".to_string()));
        assert!(text.contains(&"Pause all rooms".to_string()), "a room is playing, so there is something to pause");
        assert_eq!(list.iter().filter(|e| matches!(e, Entry::Item(_, Action::Select(_), true))).count(), 1, "exactly one room is ticked");
    }

    #[test]
    fn choosing_play_pause_volume_and_a_room_does_what_it_says() {
        let b = crate::test_support::demo_backend();
        let (gid, other, vol) = {
            let s = b.lock();
            let h = s.household().unwrap();
            let g = h.groups.iter().find(|g| g.now.transport == Transport::Playing).unwrap();
            (g.id.clone(), h.groups.iter().find(|x| x.id != g.id).unwrap().id.clone(), g.volume)
        };
        run(&b, Action::PlayPause(gid.clone()));
        assert!(crate::test_support::wait_for(5, || b.lock().find_group(&gid).unwrap().1.now.transport != Transport::Playing), "it paused");
        run(&b, Action::Louder(gid.clone()));
        assert!(crate::test_support::wait_for(5, || b.lock().find_group(&gid).unwrap().1.volume > vol), "it got louder");
        run(&b, Action::Select(other.clone()));
        assert_eq!(b.lock().selected_group.as_deref(), Some(other.as_str()));
    }
}
