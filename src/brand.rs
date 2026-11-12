//! The app's icon, drawn in code (no image files): the orange orb from the sidebar on a
//! dark rounded square. Used for the window/Dock/taskbar icon and the macOS About box.

/// The icon as RGBA pixels, `size` × `size`.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    // 4×4 samples per pixel for smooth edges.
    const N: u32 = 4;
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..N {
                for sx in 0..N {
                    let px = (x as f32 + (sx as f32 + 0.5) / N as f32) / s;
                    let py = (y as f32 + (sy as f32 + 0.5) / N as f32) / s;
                    let c = sample(px, py);
                    // Premultiply so edges blend correctly.
                    for i in 0..3 {
                        acc[i] += c[i] * c[3];
                    }
                    acc[3] += c[3];
                }
            }
            let n = (N * N) as f32;
            let a = acc[3] / n;
            let i = ((y * size + x) * 4) as usize;
            for k in 0..3 {
                out[i + k] = if a > 0.0 { (acc[k] / acc[3] * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
            }
            out[i + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

/// The icon as PNG bytes.
pub fn icon_png(size: u32) -> Vec<u8> {
    let mut png = Vec::new();
    let img = image::RgbaImage::from_raw(size, size, icon_rgba(size)).unwrap_or_default();
    let _ = image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
    png
}

/// The colour at a point of the icon (0..1 on both axes): RGBA, 0..1.
fn sample(x: f32, y: f32) -> [f32; 4] {
    // Rounded square with a margin, like other app icons.
    let (m, r) = (0.09, 0.2);
    let (cx, cy) = (x.clamp(m + r, 1.0 - m - r), y.clamp(m + r, 1.0 - m - r));
    let outside = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() > r || x < m || x > 1.0 - m || y < m || y > 1.0 - m;
    if outside {
        return [0.0; 4];
    }
    // Background: dark, slightly lighter at the top.
    let t = (y - m) / (1.0 - 2.0 * m);
    let mut c = [lerp(0.13, 0.07, t), lerp(0.14, 0.075, t), lerp(0.18, 0.095, t), 1.0];
    // The orb: orange disc with a soft peach highlight, as in the sidebar.
    let (ox, oy, or) = (0.5, 0.5, 0.27);
    let d = ((x - ox).powi(2) + (y - oy).powi(2)).sqrt();
    if d < or {
        let accent = [1.0, 140.0 / 255.0, 82.0 / 255.0];
        c = [accent[0], accent[1], accent[2], 1.0];
        let (hx, hy, hr) = (ox + 0.08, oy - 0.08, 0.14);
        let hd = ((x - hx).powi(2) + (y - hy).powi(2)).sqrt();
        if hd < hr {
            let peach = [1.0, 196.0 / 255.0, 140.0 / 255.0];
            c = [peach[0], peach[1], peach[2], 1.0];
        }
    } else if d < or + 0.012 {
        // Faint rim.
        c = [lerp(c[0], 1.0, 0.16), lerp(c[1], 1.0, 0.16), lerp(c[2], 1.0, 0.16), 1.0];
    }
    c
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
