use crate::{
    export::load_font,
    model::Timeline,
    render::{Canvas, Layout, TextAnchor, compute_logical_size, render_timeline},
};
use ab_glyph::{Font, FontArc};
use egui::{Color32, Pos2, Rect, Vec2};
use std::fmt::Write;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn color(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    format!("rgba({r},{g},{b},{})", a as f32 / 255.0)
}
struct SvgCanvas {
    size: Vec2,
    output: String,
    font: FontArc,
}
impl Canvas for SvgCanvas {
    fn begin_label(&mut self, index: usize, r: Rect) {
        let _ = write!(
            self.output,
            "<g class='timeline-label' data-index='{index}' data-x='{}' data-y='{}' data-width='{}' data-height='{}' tabindex='0' role='button' aria-label='Move period description'><title>Drag to move · Double-click to edit · Arrow keys to nudge</title><rect x='{}' y='{}' width='{}' height='{}' fill='transparent'/>",
            r.min.x,
            r.min.y,
            r.width(),
            r.height(),
            r.min.x,
            r.min.y,
            r.width(),
            r.height()
        );
    }
    fn end_label(&mut self) {
        self.output.push_str("</g>");
    }
    fn connector(
        &mut self,
        index: usize,
        a: Pos2,
        b: Pos2,
        c: Pos2,
        d: Pos2,
        color_value: Color32,
    ) {
        let _ = write!(
            self.output,
            "<polyline data-connector='{index}' points='{},{} {},{} {},{} {},{}' fill='none' stroke='{}' stroke-width='1.5'/>",
            a.x,
            a.y,
            b.x,
            b.y,
            c.x,
            c.y,
            d.x,
            d.y,
            color(color_value)
        );
    }

    fn logical_size(&self) -> Vec2 {
        self.size
    }
    fn fill_rounded_rect(&mut self, r: Rect, radius: f32, c: Color32) {
        let _ = write!(
            self.output,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='{radius}' fill='{}'/>",
            r.min.x,
            r.min.y,
            r.width(),
            r.height(),
            color(c)
        );
    }
    fn line(&mut self, a: Pos2, b: Pos2, c: Color32, width: f32) {
        let _ = write!(
            self.output,
            "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='{}' stroke-width='{width}'/>",
            a.x,
            a.y,
            b.x,
            b.y,
            color(c)
        );
    }
    fn text(&mut self, p: Pos2, text: &str, size: f32, c: Color32, anchor: TextAnchor) {
        // ab_glyph scales by ascender-to-descender height; CSS scales by em size.
        // Match the export font metrics so wrapped lines fit the same label width.
        let svg_size = size
            * self
                .font
                .units_per_em()
                .unwrap_or(self.font.height_unscaled())
            / self.font.height_unscaled();
        let width = self.text_width(text, size);
        let (align, baseline) = match anchor {
            TextAnchor::TopLeft => ("start", "hanging"),
            TextAnchor::TopRight => ("end", "hanging"),
            TextAnchor::CenterRight => ("end", "central"),
        };
        let _ = write!(
            self.output,
            "<text x='{}' y='{}' font-family='Segoe UI,Arial,sans-serif' font-size='{svg_size}' textLength='{width}' lengthAdjust='spacingAndGlyphs' text-anchor='{align}' dominant-baseline='{baseline}' fill='{}'>{}</text>",
            p.x,
            p.y,
            color(c),
            escape(text)
        );
    }
    fn text_width(&self, text: &str, size: f32) -> f32 {
        imageproc::drawing::text_size(size, &self.font, text).0 as f32
    }
}
pub fn render(t: &Timeline) -> Result<String, String> {
    let layout = Layout::default();
    let size = compute_logical_size(t, &layout);
    let mut canvas = SvgCanvas {
        size,
        font: load_font()?,
        output: format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {} {}' role='img' aria-label='Timeline'>",
            size.x, size.y
        ),
    };
    render_timeline(&mut canvas, t, &layout);
    canvas.output.push_str("</svg>");
    Ok(canvas.output)
}
#[cfg(test)]
mod tests {
    #[test]
    fn escapes_project_text() {
        assert_eq!(
            super::escape("<script>&\"'"),
            "&lt;script&gt;&amp;&quot;&apos;"
        );
    }
}
