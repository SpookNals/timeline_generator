use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineBlock {
    pub title: String,
    pub subtitle: Option<String>,
    pub start_year: f32,
    pub end_year: f32,
    pub color: Color32,
}

impl Default for TimelineBlock {
    fn default() -> Self {
        Self {
            title: "New block".to_owned(),
            subtitle: None,
            start_year: 2020.0,
            end_year: 2021.0,
            color: Color32::from_rgb(66, 133, 244),
        }
    }
}

pub const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Splits a fractional year (as stored on `TimelineBlock`) into a calendar
/// (year, month) pair, month being 1-12.
pub fn year_month_from_fraction(value: f32) -> (i32, u32) {
    let year = value.floor() as i32;
    let month = ((value - year as f32) * 12.0).round() as i32 + 1;
    if month > 12 {
        (year + 1, 1)
    } else if month < 1 {
        (year - 1, 12)
    } else {
        (year, month as u32)
    }
}

/// Combines a calendar (year, month) pair back into the fractional year
/// `TimelineBlock` stores.
pub fn fraction_from_year_month(year: i32, month: u32) -> f32 {
    year as f32 + (month.clamp(1, 12) - 1) as f32 / 12.0
}

/// Formats a fractional year as e.g. "Mar 2021".
pub fn format_year_month(value: f32) -> String {
    let (year, month) = year_month_from_fraction(value);
    format!("{} {}", MONTH_ABBR[(month - 1) as usize], year)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub start_year: f32,
    pub end_year: f32,
    pub blocks: Vec<TimelineBlock>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            start_year: 2015.0,
            end_year: 2026.0,
            blocks: Vec::new(),
        }
    }
}
