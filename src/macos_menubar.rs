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

use crate::backend::Backend;
use crate::traymenu::{entries, run, Action, Entry};

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
                run(&backend, action, &show_app, &quit_app);
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

/// Quit the app (the same as ⌘Q).
fn quit_app() {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}
