use crate::export;
use crate::model::{
    format_year_month, fraction_from_year_month, year_month_from_fraction, Timeline,
    TimelineBlock,
};
use crate::render::{compute_logical_size, render_timeline, Canvas, Layout, TextAnchor};
use eframe::egui;
use egui::{Color32, Pos2, Rect, Vec2};
use std::path::PathBuf;

/// Draws the shared `render_timeline` output onto an `egui::Painter`, by
/// scaling the logical coordinates it receives to whatever rect the UI
/// allocated for the preview.
struct EguiCanvas<'a> {
    painter: &'a egui::Painter,
    origin: Pos2,
    scale: f32,
    logical_size: Vec2,
}

impl<'a> EguiCanvas<'a> {
    fn new(painter: &'a egui::Painter, origin: Pos2, scale: f32, logical_size: Vec2) -> Self {
        Self {
            painter,
            origin,
            scale,
            logical_size,
        }
    }

    fn map_pos(&self, p: Pos2) -> Pos2 {
        self.origin + Vec2::new(p.x * self.scale, p.y * self.scale)
    }

    fn map_rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.map_pos(r.min), self.map_pos(r.max))
    }
}

impl<'a> Canvas for EguiCanvas<'a> {
    fn logical_size(&self) -> Vec2 {
        self.logical_size
    }

    fn fill_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color32) {
        self.painter
            .rect_filled(self.map_rect(rect), radius * self.scale, color);
    }

    fn line(&mut self, p1: Pos2, p2: Pos2, color: Color32, stroke_width: f32) {
        self.painter.line_segment(
            [self.map_pos(p1), self.map_pos(p2)],
            egui::Stroke::new(stroke_width * self.scale, color),
        );
    }

    fn text(&mut self, pos: Pos2, text: &str, size: f32, color: Color32, anchor: TextAnchor) {
        let align = match anchor {
            TextAnchor::TopLeft => egui::Align2::LEFT_TOP,
            TextAnchor::TopRight => egui::Align2::RIGHT_TOP,
            TextAnchor::CenterRight => egui::Align2::RIGHT_CENTER,
        };
        self.painter.text(
            self.map_pos(pos),
            align,
            text,
            egui::FontId::proportional(size * self.scale),
            color,
        );
    }

    fn text_width(&self, text: &str, size: f32) -> f32 {
        let font_id = egui::FontId::proportional(size * self.scale);
        let galley = self
            .painter
            .layout_no_wrap(text.to_owned(), font_id, Color32::BLACK);
        galley.size().x / self.scale
    }
}

/// The editable fields for adding or updating a block. Kept separate from
/// `TimelineBlock` so the subtitle can be a plain (possibly empty) string in
/// the form, and so an in-progress edit doesn't mutate the real block until
/// confirmed. Start/end are edited as a (year, month) pair rather than a
/// raw fractional year, so a block spanning e.g. just one month can be
/// entered directly instead of having to guess a decimal fraction.
struct BlockForm {
    title: String,
    subtitle: String,
    start_year: i32,
    start_month: u32,
    end_year: i32,
    end_month: u32,
    color: Color32,
}

impl Default for BlockForm {
    fn default() -> Self {
        let block = TimelineBlock::default();
        let (start_year, start_month) = year_month_from_fraction(block.start_year);
        let (end_year, end_month) = year_month_from_fraction(block.end_year);
        Self {
            title: block.title,
            subtitle: block.subtitle.unwrap_or_default(),
            start_year,
            start_month,
            end_year,
            end_month,
            color: block.color,
        }
    }
}

impl BlockForm {
    fn from_block(block: &TimelineBlock) -> Self {
        let (start_year, start_month) = year_month_from_fraction(block.start_year);
        let (end_year, end_month) = year_month_from_fraction(block.end_year);
        Self {
            title: block.title.clone(),
            subtitle: block.subtitle.clone().unwrap_or_default(),
            start_year,
            start_month,
            end_year,
            end_month,
            color: block.color,
        }
    }

    fn to_block(&self) -> TimelineBlock {
        TimelineBlock {
            title: self.title.clone(),
            subtitle: if self.subtitle.trim().is_empty() {
                None
            } else {
                Some(self.subtitle.clone())
            },
            start_year: fraction_from_year_month(self.start_year, self.start_month),
            end_year: fraction_from_year_month(self.end_year, self.end_month),
            color: self.color,
        }
    }
}

pub struct App {
    timeline: Timeline,
    form: BlockForm,
    editing_index: Option<usize>,
    current_file: Option<PathBuf>,
    status: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            timeline: Timeline::default(),
            form: BlockForm::default(),
            editing_index: None,
            current_file: None,
            status: None,
        }
    }
}

impl App {
    fn start_edit(&mut self, index: usize) {
        self.form = BlockForm::from_block(&self.timeline.blocks[index]);
        self.editing_index = Some(index);
    }

    fn cancel_edit(&mut self) {
        self.form = BlockForm::default();
        self.editing_index = None;
    }

    fn save_project(&mut self, path: PathBuf) {
        match serde_json::to_string_pretty(&self.timeline) {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(()) => {
                    self.status = Some(format!("Project saved: {}", path.display()));
                    self.current_file = Some(path);
                }
                Err(e) => self.status = Some(format!("Save failed: {e}")),
            },
            Err(e) => self.status = Some(format!("Save failed: {e}")),
        }
    }

    fn open_project(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<Timeline>(&json) {
                Ok(timeline) => {
                    self.timeline = timeline;
                    self.status = Some(format!("Project opened: {}", path.display()));
                    self.current_file = Some(path);
                    self.cancel_edit();
                }
                Err(e) => self.status = Some(format!("Open failed: invalid JSON file ({e})")),
            },
            Err(e) => self.status = Some(format!("Open failed: {e}")),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Save project").clicked() {
                if let Some(path) = &self.current_file {
                    self.save_project(path.clone());
                } else if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON project", &["json"])
                    .set_file_name("cv_timeline.json")
                    .save_file()
                {
                    self.save_project(path);
                }
            }
            if ui.button("Save as...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON project", &["json"])
                    .set_file_name("cv_timeline.json")
                    .save_file()
                {
                    self.save_project(path);
                }
            }
            if ui.button("Open project").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON project", &["json"])
                    .pick_file()
                {
                    self.open_project(path);
                }
            }
            if ui.button("Export as PNG").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG image", &["png"])
                    .set_file_name("cv_timeline.png")
                    .save_file()
                {
                    match export::export_png(&self.timeline, &path) {
                        Ok(()) => self.status = Some(format!("Exported to: {}", path.display())),
                        Err(e) => self.status = Some(format!("Export failed: {e}")),
                    }
                }
            }
        });
        if let Some(status) = &self.status {
            ui.label(status);
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Timeline settings");
        ui.horizontal(|ui| {
            ui.label("Start year:");
            ui.add(egui::DragValue::new(&mut self.timeline.start_year).range(1900.0..=2200.0));
            ui.label("End year:");
            ui.add(egui::DragValue::new(&mut self.timeline.end_year).range(1900.0..=2200.0));
        });

        ui.separator();
        ui.heading("Blocks");
        let mut to_delete: Option<usize> = None;
        let mut to_edit: Option<usize> = None;
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for i in 0..self.timeline.blocks.len() {
                    let block = &self.timeline.blocks[i];
                    let (r, g, b, _a) = block.color.to_tuple();
                    let label = format!(
                        "{} ({} - {})",
                        block.title,
                        format_year_month(block.start_year),
                        format_year_month(block.end_year)
                    );
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(r, g, b), "⬤");
                        if ui.button(label).clicked() {
                            to_edit = Some(i);
                        }
                        if ui.small_button("Delete").clicked() {
                            to_delete = Some(i);
                        }
                    });
                }
            });
        if let Some(i) = to_edit {
            self.start_edit(i);
        }
        if let Some(i) = to_delete {
            self.timeline.blocks.remove(i);
            if self.editing_index == Some(i) {
                self.cancel_edit();
            }
        }

        ui.separator();
        ui.heading(if self.editing_index.is_some() {
            "Edit block"
        } else {
            "Add block"
        });
        ui.horizontal(|ui| {
            ui.label("Title:");
            ui.text_edit_singleline(&mut self.form.title);
        });
        ui.horizontal(|ui| {
            ui.label("Subtitle:");
            ui.text_edit_singleline(&mut self.form.subtitle);
        });
        ui.horizontal(|ui| {
            ui.label("Start:");
            ui.add(egui::DragValue::new(&mut self.form.start_year).range(1900..=2200));
            ui.label("month:");
            ui.add(egui::DragValue::new(&mut self.form.start_month).range(1..=12));
        });
        ui.horizontal(|ui| {
            ui.label("End:");
            ui.add(egui::DragValue::new(&mut self.form.end_year).range(1900..=2200));
            ui.label("month:");
            ui.add(egui::DragValue::new(&mut self.form.end_month).range(1..=12));
        });
        ui.horizontal(|ui| {
            ui.label("Color:");
            egui::color_picker::color_edit_button_srgba(
                ui,
                &mut self.form.color,
                egui::color_picker::Alpha::Opaque,
            );
        });

        ui.horizontal(|ui| {
            let button_label = if self.editing_index.is_some() {
                "Save changes"
            } else {
                "Add block"
            };
            if ui.button(button_label).clicked() {
                let block = self.form.to_block();
                match self.editing_index {
                    Some(i) => self.timeline.blocks[i] = block,
                    None => self.timeline.blocks.push(block),
                }
                self.cancel_edit();
            }
            if self.editing_index.is_some() && ui.button("Cancel").clicked() {
                self.cancel_edit();
            }
        });
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let layout = Layout::default();
        let logical_size = compute_logical_size(&self.timeline, &layout);
        let available_width = ui.available_width().max(1.0);
        let scale = available_width / logical_size.x;
        let display_size = logical_size * scale;

        // The canvas height grows with the number of years the timeline
        // covers, so a long timeline scrolls vertically instead of being
        // squeezed to fit a fixed page height.
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(display_size, egui::Sense::hover());
                let mut canvas = EguiCanvas::new(&painter, response.rect.min, scale, logical_size);
                render_timeline(&mut canvas, &self.timeline, &layout);
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            self.toolbar(ui);
        });
        egui::Panel::left("side_panel")
            .default_size(320.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.side_panel(ui);
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            self.preview(ui);
        });
    }
}
