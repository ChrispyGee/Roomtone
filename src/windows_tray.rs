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
use std::time::{Duration, Instant};

use tray_icon::menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, IsIconic, IsWindowVisible, PostMessageW, SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongPtrW, ShowWindow, SystemParametersInfoW, GWL_EXSTYLE,
    LWA_ALPHA, SPI_GETWORKAREA, SW_HIDE, SW_RESTORE, SW_SHOW, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, WM_CLOSE, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

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

// The small player (`ui/popup.rs`): whether it is open, where, and what was done to the main window
// so the player can be drawn while that is hidden.
static CTX: Mutex<Option<eframe::egui::Context>> = Mutex::new(None);
static POPUP_OPEN: AtomicBool = AtomicBool::new(false);
static POPUP_AT: Mutex<Option<eframe::egui::Pos2>> = Mutex::new(None);
static POPUP_CLOSED: Mutex<Option<Instant>> = Mutex::new(None);
/// The main window was hidden (or minimised) when the player opened, so it was shown without being
/// seen (see `cover_window`); this is how it was, to put it back.
static COVERED: AtomicBool = AtomicBool::new(false);
static WAS_MINIMISED: AtomicBool = AtomicBool::new(false);
static SAVED_STYLE: AtomicIsize = AtomicIsize::new(0);

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
    uncover_window();
    if let Some(h) = window() {
        // SAFETY: `h` is the handle of this program's own window.
        unsafe {
            ShowWindow(h, if IsIconic(h) != 0 { SW_RESTORE } else { SW_SHOW });
            SetForegroundWindow(h);
        }
    }
}

/// The window is only drawn (and the app only runs) while it is shown, or while a second window it
/// owns is. The small player is that second window, but it is created from the main window's
/// drawing, so when the main window is hidden it is first shown without being seen: see-through,
/// ignoring the mouse and with no taskbar button. `uncover_window` puts it back as it was.
fn cover_window() {
    let Some(h) = window() else { return };
    // SAFETY: `h` is the handle of this program's own window.
    unsafe {
        let minimised = IsIconic(h) != 0;
        if IsWindowVisible(h) != 0 && !minimised {
            return;
        }
        let style = GetWindowLongPtrW(h, GWL_EXSTYLE);
        SAVED_STYLE.store(style, Ordering::SeqCst);
        WAS_MINIMISED.store(minimised, Ordering::SeqCst);
        SetWindowLongPtrW(h, GWL_EXSTYLE, style | (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW) as isize);
        SetLayeredWindowAttributes(h, 0, 0, LWA_ALPHA);
        COVERED.store(true, Ordering::SeqCst);
        ShowWindow(h, SW_SHOWNOACTIVATE);
    }
}

fn uncover_window() {
    if !COVERED.swap(false, Ordering::SeqCst) {
        return;
    }
    let Some(h) = window() else { return };
    // SAFETY: `h` is the handle of this program's own window.
    unsafe {
        ShowWindow(h, if WAS_MINIMISED.load(Ordering::SeqCst) { SW_SHOWMINNOACTIVE } else { SW_HIDE });
        SetWindowLongPtrW(h, GWL_EXSTYLE, SAVED_STYLE.load(Ordering::SeqCst));
    }
}

/// Remember the drawing context, so the icon's events can ask for a redraw.
pub fn set_context(ctx: &eframe::egui::Context) {
    if let Ok(mut c) = CTX.lock() {
        if c.is_none() {
            *c = Some(ctx.clone());
        }
    }
}

fn wake() {
    if let Some(c) = CTX.lock().ok().and_then(|c| c.clone()) {
        c.request_repaint();
    }
}

/// Is the small player open?
pub fn popup_open() -> bool {
    POPUP_OPEN.load(Ordering::SeqCst)
}

/// Where the small player goes (top-left, in points).
pub fn popup_at() -> eframe::egui::Pos2 {
    POPUP_AT.lock().ok().and_then(|p| *p).unwrap_or_default()
}

/// Close the small player (it has closed itself, or the icon was clicked again).
pub fn close_popup() {
    if POPUP_OPEN.swap(false, Ordering::SeqCst) {
        if let Ok(mut c) = POPUP_CLOSED.lock() {
            *c = Some(Instant::now());
        }
        uncover_window();
        wake();
    }
}

/// Bring the main window forward from the small player.
pub fn open_app() {
    close_popup();
    show_window();
}

/// Open the small player beside the icon at `icon` (left, top, right, bottom, in pixels), or close
/// it if it is open.
fn toggle_popup(icon: (f32, f32, f32, f32)) {
    if popup_open() {
        close_popup();
        return;
    }
    // Clicking the icon moves the focus off the player, which closes it just before this click
    // arrives; that click is the one that closed it, not a request to open it again.
    if POPUP_CLOSED.lock().ok().and_then(|c| *c).is_some_and(|t| t.elapsed() < Duration::from_millis(400)) {
        return;
    }
    let ctx = CTX.lock().ok().and_then(|c| c.clone());
    let ppp = ctx.as_ref().map(|c| c.pixels_per_point()).unwrap_or(1.0).max(0.5);
    let mut work = RECT { left: 0, top: 0, right: 1920, bottom: 1080 };
    // SAFETY: `work` is a RECT, which is what this request fills in.
    unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut RECT as *mut _, 0) };
    let size = (crate::ui::popup_size().0 * ppp, crate::ui::popup_size().1 * ppp);
    let (x, y) = crate::ui::popup_place(icon, (work.left as f32, work.top as f32, work.right as f32, work.bottom as f32), size);
    if let Ok(mut p) = POPUP_AT.lock() {
        *p = Some(eframe::egui::pos2(x / ppp, y / ppp));
    }
    cover_window();
    POPUP_OPEN.store(true, Ordering::SeqCst);
    wake();
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
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, position, .. } => {
                if current_backend().is_some_and(|b| b.lock().config.menu_bar_player) {
                    let (x, y) = (rect.position.x as f32, rect.position.y as f32);
                    let icon = if rect.size.width > 0 {
                        (x, y, x + rect.size.width as f32, y + rect.size.height as f32)
                    } else {
                        (position.x as f32, position.y as f32, position.x as f32, position.y as f32)
                    };
                    toggle_popup(icon);
                } else {
                    show_window()
                }
            }
            TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                if !current_backend().is_some_and(|b| b.lock().config.menu_bar_player) {
                    show_window()
                }
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
