//! macOS only: the small player that opens when the menu-bar icon is clicked.
//!
//! A native pop-over (AppKit draws it, so focus, dismissal with Esc or a click elsewhere, dark
//! mode and VoiceOver all behave as on any Mac): the rooms to choose from, the cover, the song,
//! a seek bar, previous, play or pause and next, the volume, and ways to open the window or get
//! the full menu. What it shows is `traymenu::player`; this file only draws it and passes on what
//! the person does.

use std::cell::RefCell;
use std::sync::Mutex;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSButton, NSColor, NSFont, NSImage, NSImageScaling, NSImageView, NSPopUpButton, NSPopover, NSPopoverBehavior, NSSlider, NSTextAlignment, NSTextField, NSView, NSViewController,
};
use objc2_foundation::{NSData, NSPoint, NSRect, NSSize, NSString};

use crate::backend::Backend;
use crate::traymenu::{player, seek, set_volume, Player};

const W: f64 = 300.0;
const H: f64 = 492.0;

/// The pieces of the player that change while it is open.
struct Parts {
    popover: Retained<NSPopover>,
    rooms: Retained<NSPopUpButton>,
    cover: Retained<NSImageView>,
    title: Retained<NSTextField>,
    artist: Retained<NSTextField>,
    elapsed: Retained<NSTextField>,
    remaining: Retained<NSTextField>,
    seek: Retained<NSSlider>,
    play: Retained<NSButton>,
    volume: Retained<NSSlider>,
    /// The pictures and the room the controls were last filled in for.
    shown: Player,
    art_url: Option<String>,
}

thread_local! {
    static PARTS: RefCell<Option<Parts>> = const { RefCell::new(None) };
    static TARGET: RefCell<Option<Retained<Target>>> = const { RefCell::new(None) };
    static BACKEND: RefCell<Option<Backend>> = const { RefCell::new(None) };
}

/// Cover pictures arrive from a background thread: (address, bytes).
static ART: Mutex<Option<(String, Vec<u8>)>> = Mutex::new(None);

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this class doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RoomtonePlayerTarget"]
    struct Target;

    unsafe impl NSObjectProtocol for Target {}

    impl Target {
        #[unsafe(method(room:))]
        fn room(&self, sender: &NSPopUpButton) {
            let i = sender.indexOfSelectedItem();
            let (Some(b), true) = (BACKEND.with(|b| b.borrow().clone()), i >= 0) else { return };
            let id = PARTS.with(|p| p.borrow().as_ref().and_then(|p| p.shown.rooms.get(i as usize).map(|r| r.0.clone())));
            if let Some(id) = id {
                b.lock().selected_group = Some(id);
                refresh();
            }
        }
        #[unsafe(method(playpause:))]
        fn playpause(&self, _s: Option<&AnyObject>) {
            self.go(|b, g, playing| {
                let _ = b.send(if playing { crate::backend::Cmd::Pause(g) } else { crate::backend::Cmd::Play(g) });
            });
        }
        #[unsafe(method(next:))]
        fn next(&self, _s: Option<&AnyObject>) {
            self.go(|b, g, _| {
                let _ = b.send(crate::backend::Cmd::Next(g));
            });
        }
        #[unsafe(method(previous:))]
        fn previous(&self, _s: Option<&AnyObject>) {
            self.go(|b, g, _| {
                let _ = b.send(crate::backend::Cmd::Previous(g));
            });
        }
        #[unsafe(method(openapp:))]
        fn open_app(&self, _s: Option<&AnyObject>) {
            close();
            crate::macos_menubar::show_app();
        }
        #[unsafe(method(more:))]
        fn more(&self, _s: Option<&AnyObject>) {
            close();
            crate::macos_menubar::show_menu();
        }
        #[unsafe(method(seek:))]
        fn seek_to(&self, sender: &NSSlider) {
            let to = sender.doubleValue() as f32;
            self.go(move |b, g, _| seek(b, &g, to));
        }
        #[unsafe(method(volume:))]
        fn volume_to(&self, sender: &NSSlider) {
            let to = sender.doubleValue().round().clamp(0.0, 100.0) as u8;
            self.go(move |b, g, _| set_volume(b, &g, to));
        }
    }
);

impl Target {
    /// Do something to the chosen room, then show the result soon.
    fn go(&self, f: impl FnOnce(&Backend, String, bool)) {
        let Some(b) = BACKEND.with(|b| b.borrow().clone()) else { return };
        let p = player(&b.lock());
        if p.group.is_empty() {
            return;
        }
        f(&b, p.group, p.playing);
    }
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

fn rect(x: f64, top: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, H - top - h), NSSize::new(w, h))
}

fn label(mtm: MainThreadMarker, view: &NSView, frame: NSRect, size: f64, bold: bool, dim: bool, align: NSTextAlignment) -> Retained<NSTextField> {
    let l = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    l.setFrame(frame);
    let font = if bold { NSFont::boldSystemFontOfSize(size) } else { NSFont::systemFontOfSize(size) };
    l.setFont(Some(&font));
    let colour = if dim { NSColor::secondaryLabelColor() } else { NSColor::labelColor() };
    l.setTextColor(Some(&colour));
    l.setAlignment(align);
    l.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
    view.addSubview(&l);
    l
}

fn symbol_button(mtm: MainThreadMarker, view: &NSView, frame: NSRect, symbol: &str, name: &str, action: objc2::runtime::Sel) -> Retained<NSButton> {
    let b = NSButton::new(mtm);
    b.setFrame(frame);
    b.setBordered(false);
    b.setBezelStyle(objc2_app_kit::NSBezelStyle::Automatic);
    if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(symbol), Some(&NSString::from_str(name))) {
        b.setImage(Some(&img));
    }
    b.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    b.setToolTip(Some(&NSString::from_str(name)));
    unsafe {
        b.setTarget(Some(&target(mtm)));
        b.setAction(Some(action));
    }
    view.addSubview(&b);
    b
}

fn slider(mtm: MainThreadMarker, view: &NSView, frame: NSRect, max: f64, name: &str, action: objc2::runtime::Sel, continuous: bool) -> Retained<NSSlider> {
    let s = NSSlider::new(mtm);
    s.setFrame(frame);
    s.setMinValue(0.0);
    s.setMaxValue(max);
    s.setContinuous(continuous);
    s.setToolTip(Some(&NSString::from_str(name)));
    unsafe {
        s.setTarget(Some(&target(mtm)));
        s.setAction(Some(action));
    }
    view.addSubview(&s);
    s
}

fn build(mtm: MainThreadMarker, backend: &Backend) {
    BACKEND.with(|b| *b.borrow_mut() = Some(backend.clone()));
    let view = NSView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(W, H)));
    let center = NSTextAlignment::Center;
    let rooms = NSPopUpButton::initWithFrame_pullsDown(mtm.alloc(), rect(20.0, 16.0, W - 40.0, 26.0), false);
    unsafe {
        rooms.setTarget(Some(&target(mtm)));
        rooms.setAction(Some(sel!(room:)));
    }
    view.addSubview(&rooms);
    let cover = NSImageView::new(mtm);
    cover.setFrame(rect(50.0, 54.0, 200.0, 200.0));
    cover.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    view.addSubview(&cover);
    let layer: Option<Retained<AnyObject>> = unsafe {
        let _: () = msg_send![&*cover, setWantsLayer: true];
        msg_send![&*cover, layer]
    };
    if let Some(layer) = layer {
        unsafe {
            let _: () = msg_send![&*layer, setCornerRadius: 10.0f64];
            let _: () = msg_send![&*layer, setMasksToBounds: true];
        }
    }
    let title = label(mtm, &view, rect(20.0, 266.0, W - 40.0, 22.0), 16.0, true, false, center);
    let artist = label(mtm, &view, rect(20.0, 290.0, W - 40.0, 18.0), 12.5, false, true, center);
    let seek_bar = slider(mtm, &view, rect(20.0, 316.0, W - 40.0, 18.0), 100.0, "Position in the song", sel!(seek:), false);
    let elapsed = label(mtm, &view, rect(20.0, 336.0, 80.0, 16.0), 11.0, false, true, NSTextAlignment::Left);
    let remaining = label(mtm, &view, rect(W - 100.0, 336.0, 80.0, 16.0), 11.0, false, true, NSTextAlignment::Right);
    let _prev = symbol_button(mtm, &view, rect(78.0, 360.0, 40.0, 40.0), "backward.fill", "Previous", sel!(previous:));
    let play = symbol_button(mtm, &view, rect(126.0, 356.0, 48.0, 48.0), "pause.circle.fill", "Pause", sel!(playpause:));
    let _next = symbol_button(mtm, &view, rect(182.0, 360.0, 40.0, 40.0), "forward.fill", "Next", sel!(next:));
    let volume = slider(mtm, &view, rect(52.0, 416.0, W - 104.0, 18.0), 100.0, "Volume", sel!(volume:), true);
    for (x, sym) in [(22.0, "speaker.fill"), (W - 46.0, "speaker.wave.3.fill")] {
        if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(sym), None) {
            let v = NSImageView::imageViewWithImage(&img, mtm);
            v.setFrame(rect(x, 414.0, 24.0, 22.0));
            v.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
            view.addSubview(&v);
        }
    }
    for (x, w, text, action) in [(20.0, 130.0, crate::t!("Open Roomtone"), sel!(openapp:)), (W - 100.0, 80.0, crate::t!("More…"), sel!(more:))] {
        let b = NSButton::new(mtm);
        b.setFrame(rect(x, 454.0, w, 24.0));
        b.setTitle(&NSString::from_str(text));
        b.setBezelStyle(objc2_app_kit::NSBezelStyle::Automatic);
        b.setControlSize(objc2_app_kit::NSControlSize::Small);
        unsafe {
            b.setTarget(Some(&target(mtm)));
            b.setAction(Some(action));
        }
        view.addSubview(&b);
    }
    let controller = NSViewController::new(mtm);
    controller.setView(&view);
    let popover = NSPopover::new(mtm);
    popover.setContentSize(NSSize::new(W, H));
    popover.setBehavior(NSPopoverBehavior::Transient);
    popover.setAnimates(true);
    popover.setContentViewController(Some(&controller));
    PARTS.with(|p| {
        *p.borrow_mut() = Some(Parts { popover, rooms, cover, title, artist, elapsed, remaining, seek: seek_bar, play, volume, shown: Player::default(), art_url: None });
    });
}

/// Core Graphics' colour, only ever passed along from `NSColor` to a layer.
#[repr(C)]
struct CGColor {
    _private: [u8; 0],
}
// SAFETY: it is an opaque Core Graphics type, always used behind a pointer.
unsafe impl objc2::Encode for CGColor {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGColor", &[]);
}
unsafe impl objc2::RefEncode for CGColor {
    const ENCODING_REF: objc2::Encoding = objc2::Encoding::Pointer(&<Self as objc2::Encode>::ENCODING);
}

/// A colour made from the title with a music note on it.
fn set_placeholder(cover: &NSImageView, seed: &str) {
    let c = crate::ui::art::seed_color(seed);
    let colour = NSColor::colorWithSRGBRed_green_blue_alpha(f64::from(c.r()) / 255.0, f64::from(c.g()) / 255.0, f64::from(c.b()) / 255.0, 1.0);
    unsafe {
        let layer: Option<Retained<AnyObject>> = msg_send![cover, layer];
        if let Some(layer) = layer {
            let cg: *const CGColor = msg_send![&*colour, CGColor];
            let _: () = msg_send![&*layer, setBackgroundColor: cg];
        }
    }
    if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str("music.note"), None) {
        cover.setImageScaling(NSImageScaling::ScaleNone);
        cover.setImage(Some(&img));
        cover.setContentTintColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.55)));
    }
}

fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0) as u32;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Fill the player in from the app's state. Cheap; called every second while it is open.
pub fn refresh() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(backend) = BACKEND.with(|b| b.borrow().clone()) else { return };
    let p = player(&backend.lock());
    PARTS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(parts) = slot.as_mut() else { return };
        // The room list, only when it changed.
        if parts.shown.rooms != p.rooms {
            parts.rooms.removeAllItems();
            for (_, name, _) in &p.rooms {
                parts.rooms.addItemWithTitle(&NSString::from_str(name));
            }
        }
        if let Some(i) = p.rooms.iter().position(|r| r.2) {
            parts.rooms.selectItemAtIndex(i as isize);
        }
        let nothing = crate::t!("Nothing playing");
        parts.title.setStringValue(&NSString::from_str(if p.title.is_empty() { nothing } else { &p.title }));
        parts.artist.setStringValue(&NSString::from_str(&p.artist));
        // Leave the sliders alone while the mouse is down on one.
        let dragging = objc2_app_kit::NSEvent::pressedMouseButtons() != 0;
        if !dragging {
            parts.seek.setEnabled(p.duration > 0.0);
            parts.seek.setMaxValue(f64::from(p.duration.max(1.0)));
            parts.seek.setDoubleValue(f64::from(p.position));
            parts.volume.setDoubleValue(if p.muted { 0.0 } else { f64::from(p.volume) });
        }
        parts.elapsed.setStringValue(&NSString::from_str(&if p.duration > 0.0 { clock(p.position) } else { String::new() }));
        parts.remaining.setStringValue(&NSString::from_str(&if p.duration > 0.0 { format!("-{}", clock(p.duration - p.position)) } else { String::new() }));
        let (symbol, name) = if p.playing { ("pause.circle.fill", crate::t!("Pause")) } else { ("play.circle.fill", crate::t!("Play")) };
        if let Some(img) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(symbol), Some(&NSString::from_str(name))) {
            parts.play.setImage(Some(&img));
        }
        parts.play.setToolTip(Some(&NSString::from_str(name)));
        // The cover: fetched in the background, shown when it arrives. Until then (or when there
        // is none) a soft colour made from the title, with a note on it, as in the window.
        if parts.art_url != p.art || parts.shown.title != p.title {
            set_placeholder(&parts.cover, &p.title);
        }
        if parts.art_url != p.art {
            parts.art_url = p.art.clone();
            if let Some(url) = p.art.clone() {
                std::thread::spawn(move || {
                    if let Ok(bytes) = crate::net::soap::get_bytes(&url) {
                        *ART.lock().unwrap_or_else(|e| e.into_inner()) = Some((url, bytes));
                    }
                });
            }
        }
        let arrived = ART.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some((url, bytes)) = arrived {
            if parts.art_url.as_deref() == Some(url.as_str()) {
                let data = NSData::with_bytes(&bytes);
                if let Some(img) = NSImage::initWithData(mtm.alloc(), &data) {
                    parts.cover.setContentTintColor(None);
                    parts.cover.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
                    parts.cover.setImage(Some(&img));
                }
            }
        }
        parts.shown = p;
    });
}

/// Open the player under the icon (or close it if it is open).
pub fn toggle(mtm: MainThreadMarker, backend: &Backend, button: &NSView) {
    if PARTS.with(|p| p.borrow().is_none()) {
        build(mtm, backend);
    }
    let open = PARTS.with(|p| p.borrow().as_ref().is_some_and(|p| p.popover.isShown()));
    if open {
        close();
        return;
    }
    refresh();
    PARTS.with(|p| {
        if let Some(parts) = p.borrow().as_ref() {
            parts.popover.showRelativeToRect_ofView_preferredEdge(button.bounds(), button, objc2_foundation::NSRectEdge::MinY);
        }
    });
}

pub fn close() {
    PARTS.with(|p| {
        if let Some(parts) = p.borrow().as_ref() {
            parts.popover.close();
        }
    });
}

/// Is the player on screen? (The menu-bar timer keeps it current while it is.)
pub fn is_open() -> bool {
    PARTS.with(|p| p.borrow().as_ref().is_some_and(|p| p.popover.isShown()))
}
