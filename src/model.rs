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
