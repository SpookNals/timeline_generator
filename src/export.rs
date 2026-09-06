use crate::model::Timeline;
use crate::render::{
    compute_logical_size, render_timeline, Canvas, Layout, TextAnchor, EXPORT_HEIGHT,
    LOGICAL_HEIGHT,
};
use ab_glyph::{FontArc, PxScale};
use egui::{Color32, Pos2, Rect, Vec2};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{
    draw_filled_circle_mut, draw_filled_rect_mut, draw_line_segment_mut, draw_text_mut, text_size,
};
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
        let pixel = to_rgba(color);
        let radius_px = ((radius * self.scale).max(0.0) as i32)
            .min(r.width() as i32 / 2)
            .min(r.height() as i32 / 2);

        if radius_px <= 0 {
            draw_filled_rect_mut(&mut self.image, r, pixel);
            return;
        }

        let (x, y, w, h) = (r.left(), r.top(), r.width() as i32, r.height() as i32);

        if h > 2 * radius_px {
            let band = ImgRect::at(x, y + radius_px).of_size(w as u32, (h - 2 * radius_px) as u32);
            draw_filled_rect_mut(&mut self.image, band, pixel);
        }
        if w > 2 * radius_px {
            let band = ImgRect::at(x + radius_px, y).of_size((w - 2 * radius_px) as u32, h as u32);
            draw_filled_rect_mut(&mut self.image, band, pixel);
        }
        for (cx, cy) in [
            (x + radius_px, y + radius_px),
            (x + w - radius_px - 1, y + radius_px),
            (x + radius_px, y + h - radius_px - 1),
            (x + w - radius_px - 1, y + h - radius_px - 1),
        ] {
            draw_filled_circle_mut(&mut self.image, (cx, cy), radius_px, pixel);
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

/// Renders `timeline` at a fixed high resolution and writes it to `path` as
/// a PNG, matching the live preview exactly (same shared render function,
/// just scaled up).
pub fn export_png(timeline: &Timeline, path: &Path) -> Result<(), String> {
    let font = load_font()?;
    let layout = Layout::default();
    let logical_size = compute_logical_size(timeline, &layout);
    let scale = EXPORT_HEIGHT / LOGICAL_HEIGHT;
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
