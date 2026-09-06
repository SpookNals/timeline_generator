// Suppresses the console window a plain Rust binary otherwise gets on
// Windows. Kept for debug builds so `println!`/`eprintln!` still show up
// somewhere while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod export;
mod model;
mod render;
mod ui;

fn load_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(bytes)
        .expect("bundled icon.png should decode")
        .into_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_icon(load_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "CV Timeline Generator",
        options,
        Box::new(|_cc| Ok(Box::new(ui::App::default()))),
    )
}
