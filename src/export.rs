use crate::model::Timeline;
use crate::render::{
    compute_logical_size, render_timeline, Canvas, Layout, TextAnchor, EXPORT_SCALE,
};
use ab_glyph::{FontArc, PxScale};
use egui::{Color32, Pos2, Rect, Vec2};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut, text_size};
use imageproc::rect::Rect as ImgRect;
use std::path::Path;

/// Common Windows fonts, tried in order. This app targets the user's own
/// desktop and reads the font directly from the OS at runtime rather than
/// bundling one, so no font file needs to be shipped with the project.
const FONT_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\segoeui.ttf",
    r"C:\Windows\Fonts\arial.ttf",
    r"C:\Windows\Fonts\calibri.ttf",
    r"C:\Windows\Fonts\tahoma.ttf",
];

fn load_font() -> Result<FontArc, String> {
    for path in FONT_CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(font) = FontArc::try_from_vec(bytes) {
                return Ok(font);
            }
        }
    }
    Err("Could not find a font on this system (Segoe UI, Arial, Calibri, Tahoma).".to_owned())
}

fn to_rgba(color: Color32) -> Rgba<u8> {
    Rgba([color.r(), color.g(), color.b(), color.a()])
}

/// Whether a point at local pixel coordinates `(x, y)` within a `w`x`h` box
/// falls inside that box's rounded-rect shape (radius `r`), via the
/// standard nearest-corner-circle test.
fn inside_rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> bool {
    if r <= 0.0 {
        return true;
    }
    let dx = if x < r {
        r - x
    } else if x > w - r {
        x - (w - r)
    } else {
        0.0
    };
    let dy = if y < r {
        r - y
    } else if y > h - r {
        y - (h - r)
    } else {
        0.0
    };
    dx * dx + dy * dy <= r * r
}

/// Paints `color` onto pixel `(x, y)`, alpha-compositing over the existing
/// pixel when `color` is translucent (`imageproc`'s draw helpers overwrite
/// rather than blend, which would break the shadow/highlight glass effect).
fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, color: Color32) {
    let a = color.a();
    if a == 0 {
        return;
    }
    if a == 255 {
        image.put_pixel(x, y, to_rgba(color));
        return;
    }
    let dst = *image.get_pixel(x, y);
    let af = a as f32 / 255.0;
    let mix = |s: u8, d: u8| -> u8 { (s as f32 * af + d as f32 * (1.0 - af)).round() as u8 };
    image.put_pixel(
        x,
        y,
        Rgba([
            mix(color.r(), dst[0]),
            mix(color.g(), dst[1]),
            mix(color.b(), dst[2]),
            255,
        ]),
    );
}

struct ImageCanvas {
    image: RgbaImage,
    font: FontArc,
    logical_size: Vec2,
    scale: f32,
}

impl ImageCanvas {
    fn map_pos(&self, p: Pos2) -> Pos2 {
        Pos2::new(p.x * self.scale, p.y * self.scale)
    }

    fn map_rect(&self, r: Rect) -> ImgRect {
        let min = self.map_pos(r.min);
        let max = self.map_pos(r.max);
        ImgRect::at(min.x.round() as i32, min.y.round() as i32).of_size(
            (max.x - min.x).max(1.0).round() as u32,
            (max.y - min.y).max(1.0).round() as u32,
        )
    }
}

impl Canvas for ImageCanvas {
    fn logical_size(&self) -> Vec2 {
        self.logical_size
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32) {
        let r = self.map_rect(rect);
        let (x0, y0, w, h) = (r.left(), r.top(), r.width() as i32, r.height() as i32);
        let radius_px = (radius * self.scale)
            .max(0.0)
            .min(w as f32 / 2.0)
            .min(h as f32 / 2.0);
        let (img_w, img_h) = self.image.dimensions();

        for dy in 0..h {
            let py = y0 + dy;
            if py < 0 || py as u32 >= img_h {
                continue;
            }
            for dx in 0..w {
                let px = x0 + dx;
                if px < 0 || px as u32 >= img_w {
                    continue;
                }
                if !inside_rounded_rect(dx as f32 + 0.5, dy as f32 + 0.5, w as f32, h as f32, radius_px)
                {
                    continue;
                }
                blend_pixel(&mut self.image, px as u32, py as u32, color);
            }
        }
    }

    fn line(&mut self, p1: Pos2, p2: Pos2, color: Color32, _stroke_width: f32) {
        let a = self.map_pos(p1);
        let b = self.map_pos(p2);
        draw_line_segment_mut(&mut self.image, (a.x, a.y), (b.x, b.y), to_rgba(color));
    }

    fn text(&mut self, pos: Pos2, text: &str, size: f32, color: Color32, anchor: TextAnchor) {
        let scale = PxScale::from(size * self.scale);
        let p = self.map_pos(pos);
        let (x, y) = match anchor {
            TextAnchor::TopLeft => (p.x, p.y),
            TextAnchor::TopRight => {
                let (w, _h) = text_size(scale, &self.font, text);
                (p.x - w as f32, p.y)
            }
            TextAnchor::CenterRight => {
                let (w, h) = text_size(scale, &self.font, text);
                (p.x - w as f32, p.y - h as f32 / 2.0)
            }
        };
        draw_text_mut(
            &mut self.image,
            to_rgba(color),
            x.round() as i32,
            y.round() as i32,
            scale,
            &self.font,
            text,
        );
    }

    fn text_width(&self, text: &str, size: f32) -> f32 {
        let scale = PxScale::from(size * self.scale);
        let (w, _h) = text_size(scale, &self.font, text);
        w as f32 / self.scale
    }
}

/// Renders `timeline` at a fixed pixels-per-logical-unit resolution and
/// writes it to `path` as a PNG, matching the live preview exactly (same
/// shared render function, just scaled up). The image grows taller for
/// longer timelines rather than being squeezed into a fixed height.
pub fn export_png(timeline: &Timeline, path: &Path) -> Result<(), String> {
    let font = load_font()?;
    let layout = Layout::default();
    let logical_size = compute_logical_size(timeline, &layout);
    let scale = EXPORT_SCALE;
    let pixel_size = logical_size * scale;

    let image = RgbaImage::from_pixel(
        pixel_size.x.round() as u32,
        pixel_size.y.round() as u32,
        Rgba([255, 255, 255, 255]),
    );
    let mut canvas = ImageCanvas {
        image,
        font,
        logical_size,
        scale,
    };

    render_timeline(&mut canvas, timeline, &layout);

    canvas
        .image
        .save(path)
        .map_err(|e| format!("Failed to save PNG: {e}"))
}
