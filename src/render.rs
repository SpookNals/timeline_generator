use crate::model::{format_year_month, Timeline, TimelineBlock};
use egui::{Color32, Pos2, Rect, Vec2};

/// Logical height (in logical units) allotted per year of timeline span.
/// Fixed regardless of how many years the timeline covers, so the canvas
/// grows taller for longer timelines instead of squeezing everything into a
/// constant height - long timelines scroll instead of getting cramped.
pub const YEAR_HEIGHT: f32 = 40.0;

/// Floor on the plotted height, so a very short timeline (e.g. one year)
/// still gets a reasonable amount of room.
pub const MIN_PLOT_HEIGHT: f32 = 560.0;

/// Multiplier from logical units to PNG pixels on export. Fixed regardless
/// of timeline length, so exports stay crisp without an image that balloons
/// in resolution independent of content.
pub const EXPORT_SCALE: f32 = 2.0;

#[derive(Clone, Copy)]
pub enum TextAnchor {
    TopLeft,
    /// Right-aligned horizontally, top-aligned vertically - used for labels
    /// on the left side of the axis.
    TopRight,
    /// Right-aligned horizontally, vertically centered - used for the year
    /// labels next to the axis.
    CenterRight,
}

/// A drawing surface that the shared render function paints onto, in the
/// shared logical coordinate space (see [`YEAR_HEIGHT`]). Implemented once
/// for the live `egui` preview and once for PNG export, so `render_timeline`
/// itself never needs to know which one it is talking to.
pub trait Canvas {
    fn logical_size(&self) -> Vec2;
    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32);
    fn line(&mut self, p1: Pos2, p2: Pos2, color: Color32, stroke_width: f32);
    fn text(&mut self, pos: Pos2, text: &str, size: f32, color: Color32, anchor: TextAnchor);
    /// Width, in logical units, that `text` would take up at font `size`.
    fn text_width(&self, text: &str, size: f32) -> f32;
}

pub struct Layout {
    pub margin: f32,
    /// Width (and rounded-end radius x2) of a period bar.
    pub bar_width: f32,
    /// Horizontal spacing between adjacent lane centers, i.e. one column of
    /// bars on either side of the axis.
    pub lane_pitch: f32,
    /// Horizontal gap between the axis and the nearest lane on either side -
    /// also where the year tick labels live (to the left of the axis), so
    /// this needs to stay wide enough to clear their text.
    pub connector_gap: f32,
    /// Horizontal space reserved on the outer left/right edges for each
    /// block's title/subtitle label, connected back to its bar by a leader
    /// line.
    pub label_gutter: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            margin: 44.0,
            bar_width: 16.0,
            lane_pitch: 34.0,
            connector_gap: 56.0,
            label_gutter: 300.0,
        }
    }
}

/// Which side of the axis a block's bar is drawn on.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    Left,
    Right,
}

/// For each block: which side of the axis it's drawn on, and how many
/// lanes out from the axis (0 = closest).
///
/// Side alternates by chronological order, giving the classic zig-zag
/// timeline look. Depth is assigned independently per side, by greedily
/// packing blocks into the first lane where they don't overlap in time
/// another block already there - so two blocks only need separate lanes
/// when they actually overlap and landed on the same side.
fn assign_placement(timeline: &Timeline) -> Vec<(Side, usize)> {
    let mut order: Vec<usize> = (0..timeline.blocks.len()).collect();
    order.sort_by(|&a, &b| {
        timeline.blocks[a]
            .start_year
            .total_cmp(&timeline.blocks[b].start_year)
    });

    let mut placement = vec![(Side::Right, 0usize); timeline.blocks.len()];
    let mut right_lane_ends: Vec<f32> = Vec::new();
    let mut left_lane_ends: Vec<f32> = Vec::new();

    for (rank, idx) in order.into_iter().enumerate() {
        let side = if rank % 2 == 0 { Side::Right } else { Side::Left };
        let lane_ends = match side {
            Side::Right => &mut right_lane_ends,
            Side::Left => &mut left_lane_ends,
        };
        let block = &timeline.blocks[idx];
        let mut depth = None;
        for (d, end) in lane_ends.iter_mut().enumerate() {
            if block.start_year >= *end {
                *end = block.end_year;
                depth = Some(d);
                break;
            }
        }
        let depth = depth.unwrap_or_else(|| {
            lane_ends.push(block.end_year);
            lane_ends.len() - 1
        });
        placement[idx] = (side, depth);
    }
    placement
}

fn lane_counts(placement: &[(Side, usize)]) -> (f32, f32) {
    let right = placement
        .iter()
        .filter_map(|&(s, d)| (s == Side::Right).then_some(d + 1))
        .max()
        .unwrap_or(1);
    let left = placement
        .iter()
        .filter_map(|&(s, d)| (s == Side::Left).then_some(d + 1))
        .max()
        .unwrap_or(1);
    (left as f32, right as f32)
}

/// The logical canvas size needed to draw `timeline`: a height that grows
/// with the number of years covered (see [`YEAR_HEIGHT`]), and a width that
/// grows with the number of lanes needed on either side of the axis for
/// time-overlapping blocks.
pub fn compute_logical_size(timeline: &Timeline, layout: &Layout) -> Vec2 {
    let (left_lanes, right_lanes) = lane_counts(&assign_placement(timeline));
    let width = layout.margin * 2.0
        + layout.label_gutter * 2.0
        + layout.connector_gap * 2.0
        + (left_lanes + right_lanes) * layout.lane_pitch;

    let span_years = (timeline.end_year - timeline.start_year).max(0.0);
    let plot_height = (span_years * YEAR_HEIGHT).max(MIN_PLOT_HEIGHT);
    let height = plot_height + layout.margin * 2.0;

    Vec2::new(width, height)
}

/// Maps a year to a vertical position: the most recent year is at the top
/// of `plot_rect`, the oldest at the bottom.
fn year_to_y(timeline: &Timeline, year: f32, plot_rect: Rect) -> f32 {
    let span = (timeline.end_year - timeline.start_year).max(0.001);
    let t = (year - timeline.start_year) / span;
    plot_rect.bottom() - t * plot_rect.height()
}

/// `text` if it fits in `max_width`, otherwise a shortened version ending in
/// an ellipsis, or an empty string if even the ellipsis doesn't fit.
fn truncate_to_width(canvas: &dyn Canvas, text: &str, size: f32, max_width: f32) -> String {
    if canvas.text_width(text, size) <= max_width {
        return text.to_owned();
    }
    const ELLIPSIS: &str = "…";
    if canvas.text_width(ELLIPSIS, size) > max_width {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        let candidate: String = chars[..mid].iter().collect::<String>() + ELLIPSIS;
        if canvas.text_width(&candidate, size) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        ELLIPSIS.to_owned()
    } else {
        chars[..lo].iter().collect::<String>() + ELLIPSIS
    }
}

/// Linearly interpolates between two opaque colors.
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| -> u8 { (x as f32 + (y as f32 - x as f32) * t).round() as u8 };
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Minimum height (in logical units) a bar is drawn at, regardless of how
/// short the underlying period is - so a one-month block still reads as a
/// small rounded marker instead of vanishing to a hairline.
const MIN_BAR_HEIGHT: f32 = 16.0;

/// Line heights (in logical units) reserved per label line, and the gap kept
/// between two labels stacked in the same gutter column.
const TITLE_LINE: f32 = 22.0;
const DATE_LINE: f32 = 16.0;
const SUBTITLE_LINE: f32 = 20.0;
const LABEL_GAP: f32 = 10.0;
/// Length of the little straight "nub" from the gutter boundary to a
/// label's color bullet.
const NUB_LEN: f32 = 14.0;

const TEXT_DARK: Color32 = Color32::from_rgb(29, 29, 31);
const TEXT_GRAY: Color32 = Color32::from_rgb(96, 96, 102);
const TEXT_DATE: Color32 = Color32::from_rgb(150, 152, 160);

struct BlockGeom {
    rect: Rect,
    cy: f32,
    side: Side,
}

/// Vertical space (in logical units) a block's label needs: title, a subtle
/// date-range line, and (if present) the subtitle.
fn label_content_height(block: &TimelineBlock) -> f32 {
    TITLE_LINE + DATE_LINE + if block.subtitle.is_some() { SUBTITLE_LINE } else { 0.0 }
}

/// Draws `timeline` onto `canvas`. This is the single source of truth for
/// timeline rendering, used by both the live preview and PNG export.
///
/// Each block is drawn as a short, thick rounded "bar" marking its period on
/// the axis, with its title, a subtle exact date range, and its optional
/// subtitle always placed outside in the label gutter - so a block's label
/// is never constrained by how much text fits inside it. A label is
/// connected back to its bar by a leader line in the bar's own color (dimmed
/// toward gray) ending in a matching color bullet, so it's unambiguous which
/// label belongs to which bar even when several are stacked close together.
/// Labels are packed top-to-bottom to never overlap each other, which can
/// nudge a label away from its bar's exact time position (this happens when
/// bars overlap in time on the same side and land in different lanes); when
/// that happens the leader line jogs vertically to follow it. Leader lines
/// are drawn before the bars, so where a leader line happens to cross
/// another lane's bar, the (opaque) bar is painted on top and the line
/// appears to duck behind it rather than running through it.
pub fn render_timeline(canvas: &mut dyn Canvas, timeline: &Timeline, layout: &Layout) {
    let size = canvas.logical_size();

    // Soft vertical gradient background instead of flat white, for a more
    // modern, airy feel.
    let bg_top = Color32::from_rgb(239, 242, 247);
    let bg_bottom = Color32::from_rgb(255, 255, 255);
    let band_count = 28;
    let band_height = size.y / band_count as f32;
    for i in 0..band_count {
        let t = i as f32 / (band_count - 1).max(1) as f32;
        let color = lerp_color(bg_top, bg_bottom, t);
        canvas.fill_rounded_rect(
            Rect::from_min_max(
                Pos2::new(0.0, i as f32 * band_height),
                Pos2::new(size.x, (i as f32 + 1.0) * band_height + 1.0),
            ),
            0.0,
            color,
        );
    }

    let placement = assign_placement(timeline);
    let (left_lanes, right_lanes) = lane_counts(&placement);

    let plot_top = layout.margin;
    let plot_bottom = size.y - layout.margin;
    let axis_x = layout.margin + layout.label_gutter + left_lanes * layout.lane_pitch
        + layout.connector_gap;
    let plot_rect = Rect::from_min_max(
        Pos2::new(axis_x, plot_top),
        Pos2::new(axis_x, plot_bottom),
    );

    // The vertical axis, softened to a muted blue-gray rather than harsh
    // black/gray.
    let axis_color = Color32::from_rgb(150, 156, 173);
    canvas.line(
        Pos2::new(axis_x, plot_top),
        Pos2::new(axis_x, plot_bottom),
        axis_color,
        2.0,
    );

    // Adaptive tick spacing: long timelines get sparser gridlines so labels
    // stay legible instead of turning into an unreadable wall of numbers.
    let span_years = (timeline.end_year - timeline.start_year).max(1.0);
    let step: i32 = if span_years > 80.0 {
        10
    } else if span_years > 40.0 {
        5
    } else if span_years > 15.0 {
        2
    } else {
        1
    };

    let start_year = timeline.start_year.floor() as i32;
    let end_year = timeline.end_year.ceil() as i32;
    let first_tick = start_year + ((step - start_year.rem_euclid(step)) % step);
    let mut year = first_tick;
    while year <= end_year {
        let y = year_to_y(timeline, year as f32, plot_rect);
        canvas.fill_rounded_rect(
            Rect::from_center_size(Pos2::new(axis_x, y), Vec2::splat(6.0)),
            3.0,
            Color32::from_rgb(120, 128, 150),
        );
        canvas.text(
            Pos2::new(axis_x - 16.0, y),
            &year.to_string(),
            13.0,
            Color32::from_rgb(122, 128, 142),
            TextAnchor::CenterRight,
        );
        year += step;
    }

    let right_gutter_x = axis_x + layout.connector_gap + right_lanes * layout.lane_pitch;
    let left_gutter_x = axis_x - layout.connector_gap - left_lanes * layout.lane_pitch;

    // Precompute each block's bar geometry up front, since it's needed by
    // all three drawing passes below.
    let geoms: Vec<BlockGeom> = timeline
        .blocks
        .iter()
        .enumerate()
        .map(|(i, block)| {
            let (side, depth) = placement[i];
            let depth = depth as f32;
            let y_top = year_to_y(timeline, block.end_year, plot_rect);
            let y_bottom = year_to_y(timeline, block.start_year, plot_rect)
                .max(y_top + MIN_BAR_HEIGHT);
            let half = layout.bar_width / 2.0;
            let lane_center = match side {
                Side::Right => axis_x + layout.connector_gap + depth * layout.lane_pitch + half,
                Side::Left => axis_x - layout.connector_gap - depth * layout.lane_pitch - half,
            };
            let rect = Rect::from_min_max(
                Pos2::new(lane_center - half, y_top),
                Pos2::new(lane_center + half, y_bottom),
            );
            let cy = (y_top + y_bottom) / 2.0;
            BlockGeom { rect, cy, side }
        })
        .collect();

    // Each label's final vertical position, chosen by packing labels
    // top-to-bottom per side so they never overlap each other - regardless
    // of how their bars ended up overlapping across lanes (a plain "gap to
    // the previous item" rule breaks as soon as one block's span nests
    // inside another's, e.g. a one-month gig during a multi-year job).
    let mut label_top = vec![0.0f32; timeline.blocks.len()];
    for side in [Side::Right, Side::Left] {
        let mut items: Vec<(usize, f32, f32)> = geoms
            .iter()
            .enumerate()
            .filter(|(_, g)| g.side == side)
            .map(|(i, g)| {
                let desired = g.rect.top().min(g.cy - TITLE_LINE / 2.0);
                (i, desired, label_content_height(&timeline.blocks[i]))
            })
            .collect();
        items.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut next_free = f32::MIN;
        for (i, desired, height) in items {
            let top = desired.max(next_free);
            label_top[i] = top;
            next_free = top + height + LABEL_GAP;
        }
    }

    // Pass 1: leader lines, from the bar's own outer edge (not the axis) out
    // to the gutter, jogging vertically if the label was nudged away from
    // the bar's time position to avoid overlapping another label - drawn
    // before the bars (pass 2) so bars paint over any further-out lane a
    // line happens to cross.
    let leader_base = Color32::from_rgb(186, 191, 201);
    for (i, (block, geom)) in timeline.blocks.iter().zip(&geoms).enumerate() {
        let sign = match geom.side {
            Side::Right => 1.0,
            Side::Left => -1.0,
        };
        let bar_edge_x = match geom.side {
            Side::Right => geom.rect.right(),
            Side::Left => geom.rect.left(),
        };
        let gutter_x = match geom.side {
            Side::Right => right_gutter_x,
            Side::Left => left_gutter_x,
        };
        let bullet_y = label_top[i] + TITLE_LINE / 2.0;
        let bullet_x = gutter_x + NUB_LEN * sign;
        let line_color = lerp_color(leader_base, block.color, 0.35);

        canvas.line(
            Pos2::new(bar_edge_x, geom.cy),
            Pos2::new(gutter_x, geom.cy),
            line_color,
            1.5,
        );
        if (bullet_y - geom.cy).abs() > 0.5 {
            canvas.line(
                Pos2::new(gutter_x, geom.cy),
                Pos2::new(gutter_x, bullet_y),
                line_color,
                1.5,
            );
        }
        canvas.line(
            Pos2::new(gutter_x, bullet_y),
            Pos2::new(bullet_x, bullet_y),
            line_color,
            1.5,
        );
    }

    // Pass 2: the period bars themselves, with a soft drop shadow.
    for (block, geom) in timeline.blocks.iter().zip(&geoms) {
        let radius = layout.bar_width / 2.0;
        canvas.fill_rounded_rect(
            geom.rect.translate(Vec2::new(0.0, 3.0)),
            radius + 1.0,
            Color32::from_rgba_unmultiplied(20, 24, 40, 45),
        );
        canvas.fill_rounded_rect(geom.rect, radius, block.color);
    }

    // Pass 3: labels in the gutter - a color bullet (matching the bar, so
    // it's unambiguous which label goes with which bar), the title, a
    // subtle exact date range (so the reader never has to eyeball a bar's
    // position on the axis to know its span), and the optional subtitle.
    for (i, (block, geom)) in timeline.blocks.iter().zip(&geoms).enumerate() {
        let sign = match geom.side {
            Side::Right => 1.0,
            Side::Left => -1.0,
        };
        let (gutter_x, anchor) = match geom.side {
            Side::Right => (right_gutter_x, TextAnchor::TopLeft),
            Side::Left => (left_gutter_x, TextAnchor::TopRight),
        };
        let bullet_x = gutter_x + NUB_LEN * sign;
        let text_x = bullet_x + 8.0 * sign;
        let top = label_top[i];
        let bullet_y = top + TITLE_LINE / 2.0;

        let max_label_width = match geom.side {
            Side::Right => (size.x - layout.margin - text_x).max(0.0),
            Side::Left => (text_x - layout.margin).max(0.0),
        };

        canvas.fill_rounded_rect(
            Rect::from_center_size(Pos2::new(bullet_x, bullet_y), Vec2::splat(8.0)),
            4.0,
            block.color,
        );

        let title = truncate_to_width(canvas, &block.title, 18.0, max_label_width);
        canvas.text(Pos2::new(text_x, top), &title, 18.0, TEXT_DARK, anchor);

        let date_range = format!(
            "{} – {}",
            format_year_month(block.start_year),
            format_year_month(block.end_year)
        );
        let date_text = truncate_to_width(canvas, &date_range, 12.0, max_label_width);
        canvas.text(
            Pos2::new(text_x, top + TITLE_LINE),
            &date_text,
            12.0,
            TEXT_DATE,
            anchor,
        );

        if let Some(sub) = &block.subtitle {
            let sub_text = truncate_to_width(canvas, sub, 14.0, max_label_width);
            canvas.text(
                Pos2::new(text_x, top + TITLE_LINE + DATE_LINE),
                &sub_text,
                14.0,
                TEXT_GRAY,
                anchor,
            );
        }
    }
}
