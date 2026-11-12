//! macOS only: the application menu's "About Roomtone" box and menu wording.
//!
//! The app runs as a plain program rather than a packaged `.app`, so macOS's standard About
//! box would show the file name and a folder icon. This replaces the About item's action with
//! one that opens the standard panel filled in (name, version, icon, description, credits,
//! copyright), sets the Dock icon, and gives the application menu proper, translated titles.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAboutPanelOptionApplicationIcon, NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion, NSAboutPanelOptionCredits, NSAboutPanelOptionVersion,
    NSApplication, NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSImage, NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSTextAlignment,
};
use objc2_foundation::{NSAttributedString, NSData, NSDictionary, NSString};

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this class doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RoomtoneAboutTarget"]
    struct AboutTarget;

    impl AboutTarget {
        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            if let Some(mtm) = MainThreadMarker::new() {
                show_panel(mtm);
            }
        }
    }
);

thread_local! {
    /// Menu items only point weakly at their target, so keep it alive here.
    static TARGET: RefCell<Option<Retained<AboutTarget>>> = const { RefCell::new(None) };
    static ICON: RefCell<Option<Retained<NSImage>>> = const { RefCell::new(None) };
    /// The language the menu was last titled in.
    static TITLED: RefCell<String> = const { RefCell::new(String::new()) };
}

fn icon(_mtm: MainThreadMarker) -> Option<Retained<NSImage>> {
    ICON.with(|slot| {
        if slot.borrow().is_none() {
            let data = NSData::with_bytes(&crate::brand::icon_png(512));
            *slot.borrow_mut() = NSImage::initWithData(NSImage::alloc(), &data);
        }
        slot.borrow().clone()
    })
}

/// Call every frame (cheap): sets things up once and re-titles the menu when the language changes.
pub fn install() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let lang = crate::i18n::current().to_string();
    if TITLED.with(|t| *t.borrow() == lang) {
        return;
    }
    let app = NSApplication::sharedApplication(mtm);
    let Some(menu) = app.mainMenu() else { return };
    let Some(app_menu) = menu.itemAtIndex(0).and_then(|i| i.submenu()) else { return };

    let target = TARGET.with(|slot| {
        if slot.borrow().is_none() {
            let this = AboutTarget::alloc(mtm).set_ivars(());
            let obj: Retained<AboutTarget> = unsafe { msg_send![super(this), init] };
            *slot.borrow_mut() = Some(obj);
        }
        slot.borrow().clone()
    });
    if let Some(img) = icon(mtm) {
        unsafe { app.setApplicationIconImage(Some(&img)) };
    }

    for i in 0..app_menu.numberOfItems() {
        let Some(item) = app_menu.itemAtIndex(i) else { continue };
        if item.isSeparatorItem() {
            continue;
        }
        let action = item.action();
        let title = item.title().to_string();
        let new_title = if action == Some(sel!(orderFrontStandardAboutPanel:)) || action == Some(sel!(showAbout:)) {
            if let Some(t) = &target {
                unsafe {
                    item.setTarget(Some(t));
                    item.setAction(Some(sel!(showAbout:)));
                }
            }
            t!("About Roomtone").to_string()
        } else if action == Some(sel!(hide:)) {
            t!("Hide Roomtone").to_string()
        } else if action == Some(sel!(hideOtherApplications:)) {
            t!("Hide Others").to_string()
        } else if action == Some(sel!(unhideAllApplications:)) {
            t!("Show All").to_string()
        } else if action == Some(sel!(terminate:)) {
            t!("Quit Roomtone").to_string()
        } else if item.submenu().is_some() && (title == "Services" || title == crate::i18n::tr("Services")) {
            t!("Services").to_string()
        } else {
            title.replace("roomtone", "Roomtone")
        };
        item.setTitle(&NSString::from_str(&new_title));
    }
    TITLED.with(|t| *t.borrow_mut() = lang);
}

/// The standard About panel, filled in.
fn show_panel(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    let name = NSString::from_str("Roomtone");
    let version = NSString::from_str(env!("CARGO_PKG_VERSION"));
    let build = NSString::from_str(&format!("{} · {}", crate::help::platform(), std::env::consts::ARCH));
    let copyright = NSString::from_str("© 2026 Roomtone");
    let text = [
        crate::i18n::tr("A modern controller for S1 and S2 speaker systems"),
        crate::i18n::tr("Assistant link (MCP): AI assistants can see and control your speakers, switched on or off in Settings"),
        crate::i18n::tr("Play along: rooms in an S1 and an S2 system play the same music together"),
        crate::i18n::tr("Free music directories: Radio Browser (radio-browser.info), the Apple Podcasts directory search and the Internet Archive (archive.org)."),
        crate::i18n::tr("Built with Rust and the open-source libraries egui/eframe, winit, ureq, roxmltree, serde, image, lofty and rfd."),
        crate::i18n::tr("Roomtone is an independent app. It isn't made or endorsed by the speaker maker or by any music service; their names belong to their owners."),
        crate::help::REPO_URL,
    ]
    .join("\n\n");
    let credits = credits(&text);

    let mut keys: Vec<&NSString> =
        vec![unsafe { NSAboutPanelOptionApplicationName }, unsafe { NSAboutPanelOptionApplicationVersion }, unsafe { NSAboutPanelOptionVersion }, unsafe {
            NSAboutPanelOptionCredits
        }];
    let mut values: Vec<&AnyObject> = vec![name.as_ref(), version.as_ref(), build.as_ref(), credits.as_ref()];
    // "Copyright" is the panel's key for the line under the version.
    let copyright_key = NSString::from_str("Copyright");
    keys.push(&copyright_key);
    values.push(copyright.as_ref());
    let img = icon(mtm);
    if let Some(img) = &img {
        keys.push(unsafe { NSAboutPanelOptionApplicationIcon });
        values.push(img.as_ref());
    }
    let options = NSDictionary::from_slices(&keys, &values);
    unsafe { app.orderFrontStandardAboutPanelWithOptions(&options) };
    app.activate();
}

/// Centred, small, in the system's text colour (so it reads in light and dark mode).
fn credits(text: &str) -> Retained<NSAttributedString> {
    let style = NSMutableParagraphStyle::new();
    style.setAlignment(NSTextAlignment::Center);
    let font = NSFont::systemFontOfSize(11.0);
    let color = NSColor::secondaryLabelColor();
    let keys: [&NSString; 3] = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName, NSParagraphStyleAttributeName] };
    let values: [&AnyObject; 3] = [font.as_ref(), color.as_ref(), style.as_ref()];
    let attrs = NSDictionary::from_slices(&keys, &values);
    unsafe { NSAttributedString::new_with_attributes(&NSString::from_str(text), &attrs) }
}

/// Test hook: open the About box (`open`), then save a picture of it and print the menu titles.
pub fn test_snapshot(open: bool, path: &std::path::Path) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    if open {
        show_panel(mtm);
        return;
    }
    if let Some(menu) = app.mainMenu().and_then(|m| m.itemAtIndex(0)).and_then(|i| i.submenu()) {
        let titles: Vec<String> = (0..menu.numberOfItems()).filter_map(|i| menu.itemAtIndex(i)).filter(|i| !i.isSeparatorItem()).map(|i| i.title().to_string()).collect();
        eprintln!("App menu: {}", titles.join(" | "));
    }
    let windows = app.windows();
    for w in windows.iter() {
        let size = w.frame().size;
        if size.width < 700.0 && w.isVisible() {
            let mut ok = false;
            if let Some(view) = w.contentView() {
                let bounds = view.bounds();
                if let Some(rep) = view.bitmapImageRepForCachingDisplayInRect(bounds) {
                    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
                    let props = NSDictionary::<NSString, AnyObject>::new();
                    if let Some(png) = unsafe { rep.representationUsingType_properties(objc2_app_kit::NSBitmapImageFileType::PNG, &props) } {
                        ok = std::fs::write(path, png.to_vec()).is_ok();
                    }
                }
            }
            eprintln!("About box {}x{} saved: {ok}", size.width, size.height);
            w.close();
        }
    }
}
