#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod desktop;
mod export;
mod model;
mod render;
mod svg;
mod storage;

fn main() {
    tauri::Builder::default()
        .manage(desktop::ProjectPath::default())
        .invoke_handler(tauri::generate_handler![
            desktop::preview,
            desktop::open_project,
            desktop::save_project,
            desktop::export_image,
            desktop::confirm_discard,
        ])
        .run(tauri::generate_context!())
        .expect("Could not start CV Timeline");
}
