use crate::model::Timeline;
use egui::{Color32, Pos2, Rect, Vec2};

/// The length (in logical units) that the vertical time axis always spans,
/// regardless of how many years the timeline covers. Both the live preview
/// and PNG export render in this same logical space and then scale it to
/// their actual target size, so the two stay visually identical regardless
/// of window size or export resolution.
pub const LOGICAL_HEIGHT: f32 = 1400.0;

/// Fixed pixel height used for PNG export, independent of the on-screen
/// window size, so exports are always sharp enough for print use.
pub const EXPORT_HEIGHT: f32 = 2600.0;

#[derive(Clone, Copy)]
pub enum TextAnchor {
    TopLeft,
    /// Right-aligned horizontally, top-aligned vertically - used for labels
    /// that spill out of a block on the left side of the axis.
    TopRight,
    /// Right-aligned horizontally, vertically centered - used for the year
    /// labels next to the axis.
    CenterRight,
}

/// A drawing surface that the shared render function paints onto, in the
/// shared logical coordinate space (see [`LOGICAL_HEIGHT`]). Implemented
/// once for the live `egui` preview and once for PNG export, so
/// `render_timeline` itself never needs to know which one it is talking to.
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
    /// Horizontal thickness of one lane, i.e. one column of blocks on
    /// either side of the axis.
    pub lane_width: f32,
    /// Horizontal gap between the axis and the nearest block on either
    /// side, where the connector line and (on the left) the year labels
    /// live.
    pub connector_gap: f32,
    /// Horizontal space reserved on the outer left/right edges for labels
    /// that spill out of a block that's too short to hold them.
    pub label_gutter: f32,
    /// Corner radius of a block. Blocks shorter than twice this become a
    /// full "pill" shape rather than a rounded rectangle.
    pub block_radius: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            margin: 40.0,
            lane_width: 260.0,
            connector_gap: 50.0,
            label_gutter: 260.0,
            block_radius: 16.0,
        }
    }
}

/// Which side of the axis a block's card is drawn on.
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

/// The logical canvas size needed to draw `timeline`: a fixed height (see
/// [`LOGICAL_HEIGHT`]) and a width that grows with the number of lanes
/// needed on either side of the axis for time-overlapping blocks.
pub fn compute_logical_size(timeline: &Timeline, layout: &Layout) -> Vec2 {
    let (left_lanes, right_lanes) = lane_counts(&assign_placement(timeline));
    let width = layout.margin * 2.0
        + layout.label_gutter * 2.0
        + layout.connector_gap * 2.0
        + (left_lanes + right_lanes) * layout.lane_width;
    Vec2::new(width, LOGICAL_HEIGHT)
}

/// Maps a year to a vertical position: the most recent year is at the top
/// of `plot_rect`, the oldest at the bottom.
fn year_to_y(timeline: &Timeline, year: f32, plot_rect: Rect) -> f32 {
    let span = (timeline.end_year - timeline.start_year).max(0.001);
    let t = (year - timeline.start_year) / span;
    plot_rect.bottom() - t * plot_rect.height()
}

/// For each block, the start year of the block immediately before it (in
/// time) on the same side and depth, if any - used to keep a block's
/// spilled-out label from running into the block below it.
fn prev_end_year(timeline: &Timeline, placement: &[(Side, usize)]) -> Vec<Option<f32>> {
    use std::collections::HashMap;
    let mut groups: HashMap<(Side, usize), Vec<usize>> = HashMap::new();
    for (i, &key) in placement.iter().enumerate() {
        groups.entry(key).or_default().push(i);
    }
    for group in groups.values_mut() {
        group.sort_by(|&a, &b| {
            timeline.blocks[a]
                .start_year
                .total_cmp(&timeline.blocks[b].start_year)
        });
    }

    let mut prev_end = vec![None; timeline.blocks.len()];
    for group in groups.values() {
        for pair in group.windows(2) {
            prev_end[pair[1]] = Some(timeline.blocks[pair[0]].end_year);
        }
    }
    prev_end
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

const PADDING: f32 = 10.0;
/// Minimum block height (in logical units) needed to draw title + subtitle
/// stacked inside it.
const TWO_LINE_HEIGHT: f32 = 46.0;
/// Minimum block height needed to draw just the title inside it.
const ONE_LINE_HEIGHT: f32 = 22.0;

/// Draws `timeline` onto `canvas`. This is the single source of truth for
/// timeline rendering, used by both the live preview and PNG export.
pub fn render_timeline(canvas: &mut dyn Canvas, timeline: &Timeline, layout: &Layout) {
    let size = canvas.logical_size();
    canvas.fill_rounded_rect(Rect::from_min_size(Pos2::ZERO, size), 0.0, Color32::WHITE);

    let placement = assign_placement(timeline);
    let (left_lanes, right_lanes) = lane_counts(&placement);
    let prev_end = prev_end_year(timeline, &placement);

    let plot_top = layout.margin;
    let plot_bottom = size.y - layout.margin;
    let axis_x = layout.margin + layout.label_gutter + left_lanes * layout.lane_width
        + layout.connector_gap;
    let plot_rect = Rect::from_min_max(
        Pos2::new(axis_x, plot_top),
        Pos2::new(axis_x, plot_bottom),
    );

    // The vertical axis.
    canvas.line(
        Pos2::new(axis_x, plot_top),
        Pos2::new(axis_x, plot_bottom),
        Color32::from_gray(120),
        2.0,
    );

    let start_year = timeline.start_year.floor() as i32;
    let end_year = timeline.end_year.ceil() as i32;
    for year in start_year..=end_year {
        let y = year_to_y(timeline, year as f32, plot_rect);
        canvas.line(
            Pos2::new(axis_x - 5.0, y),
            Pos2::new(axis_x + 5.0, y),
            Color32::from_gray(120),
            1.5,
        );
        canvas.text(
            Pos2::new(axis_x - 10.0, y),
            &year.to_string(),
            14.0,
            Color32::from_gray(80),
            TextAnchor::CenterRight,
        );
    }

    let right_spill_x = axis_x + layout.connector_gap + right_lanes * layout.lane_width + PADDING;
    let left_spill_x = axis_x - layout.connector_gap - left_lanes * layout.lane_width - PADDING;

    for (i, block) in timeline.blocks.iter().enumerate() {
        let (side, depth) = placement[i];
        let depth = depth as f32;
        let y_top = year_to_y(timeline, block.end_year, plot_rect);
        let y_bottom = year_to_y(timeline, block.start_year, plot_rect);
        let y_bottom = y_bottom.max(y_top + 2.0);

        let rect = match side {
            Side::Right => {
                let x0 = axis_x + layout.connector_gap + depth * layout.lane_width;
                Rect::from_min_max(
                    Pos2::new(x0 + 6.0, y_top),
                    Pos2::new(x0 + layout.lane_width - 6.0, y_bottom),
                )
            }
            Side::Left => {
                let x1 = axis_x - layout.connector_gap - depth * layout.lane_width;
                Rect::from_min_max(
                    Pos2::new(x1 - layout.lane_width + 6.0, y_top),
                    Pos2::new(x1 - 6.0, y_bottom),
                )
            }
        };

        canvas.fill_rounded_rect(rect, layout.block_radius, block.color);

        // A short connector from the axis to the block, at its vertical
        // midpoint, capped with a small tick at each end: |---[block]
        let cy = (y_top + y_bottom) / 2.0;
        let near_edge_x = match side {
            Side::Right => rect.left(),
            Side::Left => rect.right(),
        };
        canvas.line(
            Pos2::new(axis_x, cy),
            Pos2::new(near_edge_x, cy),
            Color32::from_gray(150),
            1.5,
        );
        canvas.line(
            Pos2::new(axis_x, cy - 4.0),
            Pos2::new(axis_x, cy + 4.0),
            Color32::from_gray(150),
            1.5,
        );
        canvas.line(
            Pos2::new(near_edge_x, cy - 4.0),
            Pos2::new(near_edge_x, cy + 4.0),
            Color32::from_gray(150),
            1.5,
        );

        let available_width = (rect.width() - 2.0 * PADDING).max(0.0);
        let available_height = rect.height();
        let title_width = canvas.text_width(&block.title, 18.0);
        let subtitle_width = block
            .subtitle
            .as_deref()
            .map(|s| canvas.text_width(s, 14.0))
            .unwrap_or(0.0);

        if title_width <= available_width
            && subtitle_width <= available_width
            && available_height >= TWO_LINE_HEIGHT
        {
            canvas.text(
                Pos2::new(rect.left() + PADDING, rect.top() + 6.0),
                &block.title,
                18.0,
                Color32::BLACK,
                TextAnchor::TopLeft,
            );
            if let Some(sub) = &block.subtitle {
                canvas.text(
                    Pos2::new(rect.left() + PADDING, rect.top() + 30.0),
                    sub,
                    14.0,
                    Color32::from_gray(70),
                    TextAnchor::TopLeft,
                );
            }
        } else if title_width <= available_width && available_height >= ONE_LINE_HEIGHT {
            canvas.text(
                Pos2::new(rect.left() + PADDING, rect.top() + 6.0),
                &block.title,
                18.0,
                Color32::BLACK,
                TextAnchor::TopLeft,
            );
        } else {
            // Spill the label out past all lanes on this side, away from
            // the axis, so it never overlaps another lane's blocks.
            let (spill_x, anchor) = match side {
                Side::Right => (right_spill_x, TextAnchor::TopLeft),
                Side::Left => (left_spill_x, TextAnchor::TopRight),
            };
            let max_label_width = match side {
                Side::Right => (size.x - layout.margin - spill_x).max(0.0),
                Side::Left => (spill_x - layout.margin).max(0.0),
            };
            let limit_y = prev_end[i]
                .map(|year| year_to_y(timeline, year, plot_rect))
                .unwrap_or(plot_bottom);
            let max_label_height = (limit_y - rect.top()).max(0.0);

            let title = truncate_to_width(canvas, &block.title, 18.0, max_label_width);
            canvas.text(
                Pos2::new(spill_x, rect.top() + 6.0),
                &title,
                18.0,
                Color32::from_gray(20),
                anchor,
            );
            if let Some(sub) = &block.subtitle {
                if max_label_height >= TWO_LINE_HEIGHT {
                    let sub_text = truncate_to_width(canvas, sub, 14.0, max_label_width);
                    canvas.text(
                        Pos2::new(spill_x, rect.top() + 30.0),
                        &sub_text,
                        14.0,
                        Color32::from_gray(70),
                        anchor,
                    );
                }
            }
        }
    }
}
