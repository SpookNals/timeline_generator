use crate::{
    export,
    model::Timeline,
    render::{Layout, compute_logical_size},
    svg,
};
use serde::Serialize;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Default)]
pub struct ProjectPath(Mutex<Option<PathBuf>>);
#[derive(Serialize)]
pub struct Opened {
    timeline: Timeline,
    name: String,
    svg: String,
}

pub fn validate(t: &Timeline) -> Result<(), String> {
    if !t.start_year.is_finite()
        || !t.end_year.is_finite()
        || t.start_year < 1.0
        || t.end_year > 9999.0
        || t.end_year <= t.start_year
        || t.end_year - t.start_year > 200.0
    {
        return Err("Choose a valid timeline range of up to 200 years.".into());
    }
    if t.blocks.len() > 300 {
        return Err("A project can contain up to 300 periods.".into());
    }
    for b in &t.blocks {
        if b.title.trim().is_empty()
            || b.title.len() > 500
            || b.subtitle.as_ref().is_some_and(|s| s.len() > 1000)
            || !b.start_year.is_finite()
            || !b.end_year.is_finite()
            || b.end_year < b.start_year
            || b.start_year < 1.0
            || b.end_year > 9999.0
            || b.description
                .as_ref()
                .is_some_and(|s| s.chars().count() > 1000)
            || b.label_position
                .is_some_and(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 100_000.0))
        {
            return Err("Check the title and dates. The end must be on or after the start.".into());
        }
    }
    let size = compute_logical_size(t, &Layout::default()) * crate::render::EXPORT_SCALE;
    if size.x * size.y > 80_000_000.0 {
        return Err(
            "This timeline is too large to export. Reduce the number of overlapping periods."
                .into(),
        );
    }
    Ok(())
}
#[derive(Serialize)]
pub struct Preview {
    timeline: Timeline,
    svg: String,
}
#[tauri::command]
pub fn preview(mut timeline: Timeline) -> Result<Preview, String> {
    timeline.include_periods();
    validate(&timeline)?;
    let svg = svg::render(&timeline)?;
    Ok(Preview { timeline, svg })
}
#[tauri::command]
pub async fn open_project(state: tauri::State<'_, ProjectPath>) -> Result<Option<Opened>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter("Timeline", &["json"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    load_project(file.path(), &state)
}

fn load_project(path: &Path, state: &ProjectPath) -> Result<Option<Opened>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(2_000_001)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 2_000_000 {
        return Err("The project file is too large.".into());
    }
    let timeline: Timeline =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid project: {e}"))?;
    // Prepare the complete preview before changing the active save destination.
    let prepared = preview(timeline)?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(path.to_owned());
    Ok(Some(Opened {
        timeline: prepared.timeline,
        svg: prepared.svg,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    }))
}
#[tauri::command]
pub async fn save_project(
    mut timeline: Timeline,
    save_as: bool,
    state: tauri::State<'_, ProjectPath>,
) -> Result<Option<String>, String> {
    timeline.include_periods();
    validate(&timeline)?;
    let current = state.0.lock().map_err(|e| e.to_string())?.clone();
    let path = if !save_as && current.is_some() {
        current.unwrap()
    } else {
        let Some(file) = rfd::AsyncFileDialog::new()
            .add_filter("Timeline", &["json"])
            .set_file_name("timeline.json")
            .save_file()
            .await
        else {
            return Ok(None);
        };
        file.path().to_owned()
    };
    let bytes = serde_json::to_vec_pretty(&timeline).map_err(|e| e.to_string())?;
    crate::storage::replace_file(&path, |file| {
        file.write_all(&bytes).map_err(|e| e.to_string())
    })?;
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    *state.0.lock().map_err(|e| e.to_string())? = Some(path);
    Ok(Some(name))
}
#[tauri::command]
pub async fn export_image(mut timeline: Timeline) -> Result<bool, String> {
    timeline.include_periods();
    validate(&timeline)?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter("PNG", &["png"])
        .set_file_name("timeline.png")
        .save_file()
        .await
    else {
        return Ok(false);
    };
    let path = file.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || export::export_png(&timeline, &path))
        .await
        .map_err(|e| e.to_string())??;
    Ok(true)
}
#[tauri::command]
pub async fn confirm_discard() -> bool {
    rfd::AsyncMessageDialog::new()
        .set_title("Unsaved changes")
        .set_description("Discard unsaved changes?")
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        .await
        == rfd::MessageDialogResult::Yes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_open_preserves_save_destination() {
        let directory = tempfile::tempdir().unwrap();
        let previous = directory.path().join("previous.json");
        let state = ProjectPath(Mutex::new(Some(previous.clone())));
        let invalid = directory.path().join("invalid.json");
        std::fs::write(&invalid, b"invalid json").unwrap();
        assert!(load_project(&invalid, &state).is_err());
        assert_eq!(*state.0.lock().unwrap(), Some(previous));
        let valid = directory.path().join("valid.json");
        std::fs::write(&valid, serde_json::to_vec(&Timeline::default()).unwrap()).unwrap();
        let loaded = load_project(&valid, &state).unwrap().unwrap();
        assert!(loaded.svg.contains("<svg"));
        assert_eq!(*state.0.lock().unwrap(), Some(valid));
    }
    #[test]
    fn png_encoding_does_not_depend_on_filename_extension() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("custom-name.data");
        export::export_png(&Timeline::default(), &path).unwrap();
        assert!(
            std::fs::read(path)
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
    }
    #[test]
    fn preview_expands_both_ends_and_keeps_period_dates() {
        let mut t = Timeline::default();
        t.blocks.push(crate::model::TimelineBlock {
            start_year: 2010.5,
            end_year: 2030.5,
            ..Default::default()
        });
        let result = preview(t).unwrap();
        assert_eq!(
            (result.timeline.start_year, result.timeline.end_year),
            (2010.0, 2031.0)
        );
        assert_eq!(result.timeline.blocks[0].start_year, 2010.5);
        assert!(result.svg.contains("20 years"));
    }
    #[test]
    fn descriptions_and_positions_survive_save_and_render() {
        let mut t = Timeline::default();
        t.blocks.push(crate::model::TimelineBlock {
            subtitle: Some("Example company".into()),
            description: Some("Built a product.\nLed the team & mentored others.".into()),
            label_position: Some([100.0, 120.0]),
            ..Default::default()
        });
        let restored: Timeline = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(restored.blocks[0].label_position, Some([100.0, 120.0]));
        let result = preview(restored).unwrap();
        assert!(result.svg.contains("data-x='100' data-y='120'"));
        assert!(result.svg.contains("Example company"));
        assert!(result.svg.contains("Built a product."));
        assert!(result.svg.contains("Led the team &amp; mentored others."));
        assert!(result.svg.contains("1 year"));
        assert_eq!(crate::model::duration(2020.0, 2021.5), "1 year 6 months");
    }
    #[test]
    fn legacy_json_renders_and_exports() {
        let json = r#"{"start_year":2015.0,"end_year":2026.0,"blocks":[{"title":"Werk & <studie>","subtitle":"Test","start_year":2020.0,"end_year":2023.5,"color":[66,133,244,255]}]}"#;
        let timeline: Timeline = serde_json::from_str(json).unwrap();
        validate(&timeline).unwrap();
        let output = svg::render(&timeline).unwrap();
        assert!(output.contains("Werk &amp; &lt;studie&gt;"));
        let roundtrip: Timeline =
            serde_json::from_slice(&serde_json::to_vec(&timeline).unwrap()).unwrap();
        assert_eq!(roundtrip.blocks[0].color.to_array(), [66, 133, 244, 255]);
        let path =
            std::env::temp_dir().join(format!("cv-timeline-test-{}.png", std::process::id()));
        export::export_png(&timeline, &path).unwrap();
        let dimensions = image::image_dimensions(&path).unwrap();
        let expected =
            compute_logical_size(&timeline, &Layout::default()) * crate::render::EXPORT_SCALE;
        assert_eq!(dimensions, (expected.x as u32, expected.y as u32));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn rejects_invalid_ranges_and_expands_for_periods() {
        let mut t = Timeline::default();
        assert!(validate(&t).is_ok());
        t.end_year = t.start_year;
        assert!(validate(&t).is_err());
        t.end_year = 2026.0;
        t.blocks.push(crate::model::TimelineBlock {
            start_year: 2010.0,
            ..Default::default()
        });
        t.include_periods();
        assert_eq!(t.start_year, 2010.0);
        assert!(validate(&t).is_ok());
    }
}
