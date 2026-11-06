# CV Timeline

Desktop timeline editor built with Tauri 2 and Rust. Graphite and amber shell,
subtle optional scanlines, and a short timeline-drawing startup animation.
The light editor and PNG exports have no scanline effects. Reduced-motion
preferences skip the startup animation.

## Run

Requires Rust, the Windows C++ build tools and WebView2 on Windows.

```sh
cargo run
```

Build a standalone executable with embedded frontend assets:

```sh
cargo build --release
```

The executable is `target/release/cv_timeline.exe`. No Node.js, frontend
server or internet connection is needed to run the app.

For a Windows NSIS installer, install the Tauri 2 CLI and run:

```sh
cargo install tauri-cli --version '^2' --locked
cargo tauri build
```

Tauri configuration: https://v2.tauri.app/reference/config/

## Use

- Set the visible year range, then add periods with a title, optional subtitle,
  month dates and color. The timeline automatically expands to include new or
  edited periods, even when their dates are outside the current year range.
- Add a company and optional multiline description. Duration is calculated in
  years and months from the entered dates (same-month periods show 0 months).
- Drag a description anywhere on the worksheet; its connector follows without
  changing the dates. Arrow keys nudge a focused description (Shift for larger
  steps). Double-click to edit. Reset label position restores automatic placement.
  Manually placed descriptions can overlap; their positions are saved and exported.
- Click a period in the sidebar to edit or delete it.
- Open/save existing JSON projects using the native file dialogs. Failed opens retain
  the current project; saves and PNG exports replace files only after a complete write.
- Ctrl+S saves, Ctrl+Shift+S saves as, Ctrl+O opens a project.
- Export PNG uses the existing Rust renderer at 2x logical resolution.
- Zoom with the toolbar; clicking the percentage restores fit-to-width.
- Toggle scanlines at the bottom of the sidebar. This preference is remembered.
- Unsaved changes trigger confirmation when opening another file or closing.

Existing JSON projects remain supported. RGBA arrays and fractional years are unchanged; optional description and label_position fields store the new details.
Projects are validated before rendering or saving: the range automatically expands to fit periods, ranges span at most 200 years, and projects have at most 300 periods.

## Implementation

- `frontend/`: local HTML, CSS and JavaScript; no framework or bundler.
- `src/desktop.rs`: Tauri commands, file dialogs and validation.
- `src/model.rs`: existing serializable project model.
- `src/render.rs`: shared layout, overlap handling and drawing commands.
- `src/svg.rs`: SVG preview adapter for the same renderer; text is escaped.
- `src/export.rs`: existing high-resolution PNG adapter and system font lookup.
- `tauri.conf.json`, `capabilities/`: window, asset and permission configuration.

Preview and export share layout calculations; SVG text is drawn by WebView2,
while PNG text is rasterized using the system font, so glyph appearance can differ
slightly. The original egui interface remains in `src/ui.rs` as reference and is
not compiled.

## Verify

```sh
cargo test
cargo check
node --check frontend/app.js
node tests/close.test.cjs
node tests/editor.test.cjs
```

MIT; see LICENSE.
