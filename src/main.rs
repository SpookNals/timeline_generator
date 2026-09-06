mod export;
mod model;
mod render;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 750.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CV Timeline Generator",
        options,
        Box::new(|_cc| Ok(Box::new(ui::App::default()))),
    )
}
