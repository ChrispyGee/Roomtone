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

/// The icon as PNG bytes (for the macOS About box and the installers).
pub fn icon_png(size: u32) -> Vec<u8> {
    let mut png = Vec::new();
    let img = image::RgbaImage::from_raw(size, size, icon_rgba(size)).unwrap_or_default();
    let _ = image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
    png
}

/// The icon as a Windows `.ico` file: PNG pictures at the usual sizes.
pub fn icon_ico() -> Vec<u8> {
    let sizes = [16u32, 32, 48, 64, 128, 256];
    let pngs: Vec<Vec<u8>> = sizes.iter().map(|&s| icon_png(s)).collect();
    let mut out = vec![0, 0, 1, 0];
    out.extend((sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, png) in sizes.iter().zip(&pngs) {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        out.extend([dim, dim, 0, 0, 1, 0, 32, 0]);
        out.extend((png.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in pngs {
        out.extend(png);
    }
    out
}

/// Save the icon for the installers: `.ico` for Windows, otherwise a PNG of `size` pixels.
pub fn write_icon(path: &std::path::Path, size: u32) -> std::io::Result<()> {
    let is_ico = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ico"));
    std::fs::write(path, if is_ico { icon_ico() } else { icon_png(size.clamp(16, 1024)) })
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

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn icon_has_clear_corners_and_an_orange_middle() {
        let n = 64usize;
        let px = icon_rgba(n as u32);
        assert_eq!(px.len(), n * n * 4);
        assert_eq!(px[3], 0, "the top-left corner is transparent");
        let mid = ((n / 2) * n + n / 2 + 6) * 4;
        assert_eq!(px[mid + 3], 255);
        assert!(px[mid] > 200 && px[mid + 2] < 160, "orange in the middle");
        assert!(icon_png(32).starts_with(&[0x89, b'P', b'N', b'G']));
        let ico = icon_ico();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 6, 0], "an icon file holding six pictures");
        assert_eq!(&ico[6 + 6 * 16 + 1..][..3], b"PNG", "the first picture follows the directory");
    }
}
