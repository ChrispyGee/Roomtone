//! Windows only: Roomtone in the notification area (the tray next to the clock).
//!
//! An icon whose menu is the same as the Mac menu bar's (`traymenu.rs`): what's playing in the
//! chosen room, play and pause, next, previous, volume, the rooms, *Pause all rooms*, *Open
//! Roomtone* and *Quit Roomtone*. The menu is rebuilt each time it is opened (from the icon's
//! click event, which arrives just before the menu is shown), so it is always current. A left
//! click opens the window. With *Keep running when the window is closed* on, the close button
//! hides the window (`hide_window`) instead of quitting; everything here works with the window
//! hidden, because the icon, its menu and the window's show and hide are system calls that don't
//! need the window to be drawing.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, Once};

use tray_icon::menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, PostMessageW, SetForegroundWindow, ShowWindow, SW_HIDE, SW_RESTORE, SW_SHOW, WM_CLOSE};

use crate::backend::Backend;
use crate::traymenu::{entries, run, Action, Entry};

thread_local! {
    /// The icon lives on the thread that made it (the window's), and so do its click events.
    static TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
}

/// What the handlers need, which they can't be given directly (they run from the icon's events).
static BACKEND: Mutex<Option<Backend>> = Mutex::new(None);
/// The actions of the menu as last built, by item id.
static ACTIONS: Mutex<Vec<Action>> = Mutex::new(Vec::new());
static WINDOW: AtomicIsize = AtomicIsize::new(0);
static QUITTING: AtomicBool = AtomicBool::new(false);
static HANDLERS: Once = Once::new();

/// Remember the window to show and hide (its handle), once it exists.
pub fn set_window(hwnd: isize) {
    WINDOW.store(hwnd, Ordering::SeqCst);
}

/// Is the app on its way out (so the close button should really close)?
pub fn quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

fn window() -> Option<HWND> {
    let h = WINDOW.load(Ordering::SeqCst);
    (h != 0).then_some(h as HWND)
}

/// Hide the window so Roomtone keeps running from the icon.
pub fn hide_window() {
    if let Some(h) = window() {
        // SAFETY: `h` is the handle of this program's own window.
        unsafe { ShowWindow(h, SW_HIDE) };
    }
}

/// Bring the window back to the front.
pub fn show_window() {
    if let Some(h) = window() {
        // SAFETY: `h` is the handle of this program's own window.
        unsafe {
            ShowWindow(h, if IsIconic(h) != 0 { SW_RESTORE } else { SW_SHOW });
            SetForegroundWindow(h);
        }
    }
}

/// Quit: ask the window to close (it is shown first so it can answer), and if that hasn't ended
/// the program a few seconds later, end it.
fn quit() {
    QUITTING.store(true, Ordering::SeqCst);
    if let Some(h) = window() {
        // SAFETY: `h` is the handle of this program's own window.
        unsafe {
            ShowWindow(h, SW_SHOW);
            PostMessageW(h, WM_CLOSE, 0, 0);
        }
    }
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(4));
        std::process::exit(0);
    });
}

fn current_backend() -> Option<Backend> {
    BACKEND.lock().ok().and_then(|b| b.clone())
}

/// Build the menu for the current state, remembering what each item does.
fn build_menu() -> Option<Menu> {
    let backend = current_backend()?;
    let list = entries(&backend.lock());
    let menu = Menu::new();
    let mut actions = vec![];
    for e in &list {
        append(&menu, e, &mut actions);
    }
    *ACTIONS.lock().ok()? = actions;
    Some(menu)
}

/// Something that can hold menu items: the menu itself, or a submenu.
trait Holder {
    fn add(&self, item: &dyn IsMenuItem);
}
impl Holder for Menu {
    fn add(&self, item: &dyn IsMenuItem) {
        let _ = self.append(item);
    }
}
impl Holder for Submenu {
    fn add(&self, item: &dyn IsMenuItem) {
        let _ = self.append(item);
    }
}

fn append(holder: &dyn Holder, e: &Entry, actions: &mut Vec<Action>) {
    match e {
        Entry::Sep => holder.add(&PredefinedMenuItem::separator()),
        Entry::Label(t) => holder.add(&MenuItem::new(t, false, None)),
        Entry::Item(t, a, ticked) => {
            actions.push(a.clone());
            let id = format!("act:{}", actions.len() - 1);
            if *ticked {
                holder.add(&CheckMenuItem::with_id(id, t, true, true, None));
            } else {
                holder.add(&MenuItem::with_id(id, t, true, None));
            }
        }
        Entry::Sub(t, inner) => {
            let sub = Submenu::new(t, true);
            for i in inner {
                append(&sub, i, actions);
            }
            holder.add(&sub);
        }
    }
}

/// Put the icon in the notification area while `on`, and take it away when not. Call every frame
/// (cheap), on the thread that runs the window.
pub fn install(backend: &Backend, on: bool) {
    let have = TRAY.with(|t| t.borrow().is_some());
    if on == have {
        return;
    }
    if !on {
        TRAY.with(|t| t.borrow_mut().take());
        return;
    }
    if let Ok(mut b) = BACKEND.lock() {
        *b = Some(backend.clone());
    }
    HANDLERS.call_once(|| {
        MenuEvent::set_event_handler(Some(|e: MenuEvent| {
            let Some(n) = e.id.0.strip_prefix("act:").and_then(|n| n.parse::<usize>().ok()) else { return };
            let action = ACTIONS.lock().ok().and_then(|a| a.get(n).cloned());
            if let (Some(action), Some(b)) = (action, current_backend()) {
                run(&b, action, &show_window, &quit);
            }
        }));
        TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| match e {
            // Just before the menu appears: make it current.
            TrayIconEvent::Click { button: MouseButton::Right, button_state: MouseButtonState::Up, .. } => {
                if let Some(menu) = build_menu() {
                    TRAY.with(|t| {
                        if let Some(tray) = t.borrow().as_ref() {
                            tray.set_menu(Some(Box::new(menu)));
                        }
                    });
                }
            }
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                show_window()
            }
            _ => {}
        }));
    });
    let icon = Icon::from_rgba(crate::brand::icon_rgba(32), 32, 32).ok();
    let mut builder = TrayIconBuilder::new().with_tooltip("Roomtone").with_menu_on_left_click(false);
    if let Some(icon) = icon {
        builder = builder.with_icon(icon);
    }
    if let Some(menu) = build_menu() {
        builder = builder.with_menu(Box::new(menu));
    }
    if let Ok(tray) = builder.build() {
        TRAY.with(|t| *t.borrow_mut() = Some(tray));
    }
}
