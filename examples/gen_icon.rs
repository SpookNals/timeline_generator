//! One-off generator for the app icon: a small pixel-art grid (drawn as a
//! handful of rectangles on a rounded dark backdrop) upscaled with
//! nearest-neighbor sampling so the pixels stay crisp and blocky. Run with
//! `cargo run --example gen_icon` and it writes `assets/icon.png` (used as
//! the live window icon) and `assets/icon.ico` (embedded into the .exe
//! itself, at several sizes, so File Explorer/the taskbar/a desktop shortcut
//! show it before the app is even running).

use image::{imageops::FilterType, Rgba, RgbaImage};

const GRID: i32 = 16;
const SCALE: u32 = 16;

type Color = [u8; 4];
const TRANSPARENT: Color = [0, 0, 0, 0];
const BG: Color = [30, 33, 40, 255];
const AXIS: Color = [205, 209, 218, 255];
const BLUE: Color = [66, 133, 244, 255];
const GREEN: Color = [52, 199, 89, 255];
const ORANGE: Color = [255, 149, 0, 255];

struct Grid {
    pixels: Vec<Color>,
}

impl Grid {
    fn new() -> Self {
        Self {
            pixels: vec![TRANSPARENT; (GRID * GRID) as usize],
        }
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        for gy in y..y + h {
            for gx in x..x + w {
                if gx < 0 || gy < 0 || gx >= GRID || gy >= GRID {
                    continue;
                }
                self.pixels[(gy * GRID + gx) as usize] = color;
            }
        }
    }

    /// A square backdrop with its four corners chamfered by clipping the
    /// corner cell and its two orthogonal neighbors - a cheap but
    /// recognizable "rounded corner" at icon resolution.
    fn rounded_square(&mut self, color: Color) {
        let corners = [(0, 0), (GRID - 1, 0), (0, GRID - 1), (GRID - 1, GRID - 1)];
        for y in 0..GRID {
            for x in 0..GRID {
                let clipped = corners
                    .iter()
                    .any(|&(cx, cy)| (x - cx).abs() + (y - cy).abs() <= 1);
                if !clipped {
                    self.pixels[(y * GRID + x) as usize] = color;
                }
            }
        }
    }
}

fn main() {
    let mut grid = Grid::new();

    grid.rounded_square(BG);
    // Vertical axis, matching the app's own timeline visual.
    grid.fill_rect(7, 2, 2, 12, AXIS);
    // Bars alternating left/right off the axis, like real timeline entries.
    grid.fill_rect(9, 3, 5, 3, BLUE);
    grid.fill_rect(2, 7, 5, 3, GREEN);
    grid.fill_rect(9, 11, 6, 3, ORANGE);

    let mut image = RgbaImage::new(GRID as u32 * SCALE, GRID as u32 * SCALE);
    for y in 0..GRID as u32 * SCALE {
        for x in 0..GRID as u32 * SCALE {
            let c = grid.pixels[((y / SCALE) as i32 * GRID + (x / SCALE) as i32) as usize];
            image.put_pixel(x, y, Rgba(c));
        }
    }

    std::fs::create_dir_all("assets").expect("create assets dir");
    image.save("assets/icon.png").expect("save icon.png");
    println!(
        "wrote assets/icon.png ({}x{})",
        GRID as u32 * SCALE,
        GRID as u32 * SCALE
    );

    write_ico(&image, "assets/icon.ico", &[16, 32, 48, 64, 128, 256]);
    println!("wrote assets/icon.ico");
}

/// Writes a multi-resolution `.ico` by downsampling `master` (with nearest-
/// neighbor, to keep the pixel-art blocks crisp instead of blurring them)
/// to each requested size and packing the results as PNG-in-ICO frames,
/// which every Windows version since Vista accepts.
fn write_ico(master: &RgbaImage, path: &str, sizes: &[u32]) {
    let frames: Vec<(u32, Vec<u8>)> = sizes
        .iter()
        .map(|&size| {
            let resized = image::imageops::resize(master, size, size, FilterType::Nearest);
            let mut png_bytes = Vec::new();
            resized
                .write_to(
                    &mut std::io::Cursor::new(&mut png_bytes),
                    image::ImageFormat::Png,
                )
                .expect("encode ico frame as png");
            (size, png_bytes)
        })
        .collect();

    let mut out = Vec::new();
    // ICONDIR header: reserved, type=1 (icon), image count.
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());

    let header_len = 6 + 16 * frames.len();
    let mut offset = header_len as u32;
    for (size, png_bytes) in &frames {
        let dim_byte = if *size >= 256 { 0u8 } else { *size as u8 };
        out.push(dim_byte); // width
        out.push(dim_byte); // height
        out.push(0); // color count (0 = not palette-based)
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // color planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png_bytes.len() as u32;
    }
    for (_, png_bytes) in &frames {
        out.extend_from_slice(png_bytes);
    }

    std::fs::write(path, out).expect("write icon.ico");
}
