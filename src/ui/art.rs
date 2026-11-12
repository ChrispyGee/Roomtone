//! Album art: downloaded in the background from the speakers, or a soft gradient
//! made from the title when there is none.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};

use crate::net::soap;

enum Slot {
    Loading,
    Ready(TextureHandle),
    Failed,
}

pub struct ArtCache {
    slots: HashMap<String, Slot>,
    placeholders: HashMap<String, TextureHandle>,
    tx: Sender<String>,
    done: Finished,
}

/// Downloads that have finished: (address, picture if it could be read).
type Finished = Arc<Mutex<Vec<(String, Option<ColorImage>)>>>;

impl ArtCache {
    pub fn new(ctx: &egui::Context) -> ArtCache {
        let (tx, rx) = channel::<String>();
        let rx = Arc::new(Mutex::new(rx));
        let done = Arc::new(Mutex::new(vec![]));
        for _ in 0..3 {
            let rx = rx.clone();
            let done = done.clone();
            let ctx = ctx.clone();
            thread::spawn(move || loop {
                let url = {
                    let Ok(r) = rx.lock() else { return };
                    match r.recv() {
                        Ok(u) => u,
                        Err(_) => return,
                    }
                };
                let img = soap::get_bytes(&url).ok().and_then(|b| image::load_from_memory(&b).ok()).map(|img| {
                    let img = img.thumbnail(400, 400).to_rgba8();
                    let size = [img.width() as usize, img.height() as usize];
                    ColorImage::from_rgba_unmultiplied(size, img.as_raw())
                });
                if let Ok(mut d) = done.lock() {
                    d.push((url, img));
                }
                ctx.request_repaint();
            });
        }
        ArtCache { slots: HashMap::new(), placeholders: HashMap::new(), tx, done }
    }

    fn collect(&mut self, ctx: &egui::Context) {
        let finished: Vec<_> = match self.done.lock() {
            Ok(mut d) => d.drain(..).collect(),
            Err(_) => return,
        };
        for (url, img) in finished {
            let slot = match img {
                Some(img) => Slot::Ready(ctx.load_texture(&url, img, TextureOptions::LINEAR)),
                None => Slot::Failed,
            };
            self.slots.insert(url, slot);
        }
    }

    /// The picture for `url`, or a placeholder drawn from `seed` while it loads.
    pub fn texture(&mut self, ctx: &egui::Context, url: Option<&str>, seed: &str) -> egui::TextureId {
        self.collect(ctx);
        if let Some(url) = url.filter(|u| u.starts_with("http")) {
            match self.slots.get(url) {
                Some(Slot::Ready(t)) => return t.id(),
                Some(_) => {}
                None => {
                    self.slots.insert(url.to_string(), Slot::Loading);
                    let _ = self.tx.send(url.to_string());
                }
            }
        }
        self.placeholder(ctx, seed)
    }

    pub fn is_ready(&self, url: Option<&str>) -> bool {
        url.is_some_and(|u| matches!(self.slots.get(u), Some(Slot::Ready(_))))
    }

    #[allow(dead_code)]
    pub fn failed(&self, url: &str) -> bool {
        matches!(self.slots.get(url), Some(Slot::Failed))
    }

    fn placeholder(&mut self, ctx: &egui::Context, seed: &str) -> egui::TextureId {
        if let Some(t) = self.placeholders.get(seed) {
            return t.id();
        }
        let img = gradient(seed);
        let t = ctx.load_texture(format!("ph:{seed}"), img, TextureOptions::LINEAR);
        let id = t.id();
        self.placeholders.insert(seed.to_string(), t);
        id
    }
}

pub fn hash(s: &str) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

pub fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match (i as i32).rem_euclid(6) {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

pub fn seed_color(seed: &str) -> Color32 {
    let h = hash(seed);
    let c = hsv((h % 360) as f32 / 360.0, 0.55, 0.85);
    Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8)
}

/// A two-colour diagonal gradient with a soft glow, unique per title.
fn gradient(seed: &str) -> ColorImage {
    let n = 96usize;
    let h = hash(seed);
    let h1 = (h % 360) as f32 / 360.0;
    let h2 = (h1 + 0.12 + ((h >> 12) % 100) as f32 / 400.0) % 1.0;
    let a = hsv(h1, 0.62, 0.78);
    let b = hsv(h2, 0.75, 0.38);
    let gx = 0.25 + ((h >> 20) % 50) as f32 / 100.0;
    let gy = 0.2 + ((h >> 28) % 40) as f32 / 100.0;
    let mut px = Vec::with_capacity(n * n);
    for y in 0..n {
        for x in 0..n {
            let fx = x as f32 / n as f32;
            let fy = y as f32 / n as f32;
            let t = ((fx + fy) / 2.0).clamp(0.0, 1.0);
            let d = ((fx - gx).powi(2) + (fy - gy).powi(2)).sqrt();
            let glow = (1.0 - d * 1.8).max(0.0).powi(2) * 0.35;
            let mix = |i: usize| ((a[i] * (1.0 - t) + b[i] * t + glow).min(1.0) * 255.0) as u8;
            px.push(Color32::from_rgb(mix(0), mix(1), mix(2)));
        }
    }
    ColorImage::new([n, n], px)
}
