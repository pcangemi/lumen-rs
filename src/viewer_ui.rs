//! Software-drawn native viewer UI. Kept independent of the window for headless checks.
use crate::{output::tonemap, progressive::Tile};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use glam::Vec3;

const BACKGROUND: u32 = 0x101316;
const PANEL: u32 = 0x1a1e22;
const TEXT: u32 = 0xedece8;
const MUTED: u32 = 0x8e999f;
const ACCENT: u32 = 0xb5e4b6;

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}
impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x as f32
            && y >= self.y as f32
            && x < (self.x + self.width) as f32
            && y < (self.y + self.height) as f32
    }
}
#[derive(Clone, Copy)]
pub enum Action {
    Scene(usize),
    TogglePause,
    ToggleDenoise,
    Restart,
    Save,
}
pub struct Button {
    pub rect: Rect,
    pub action: Action,
}

/// UI state contains a copy of the linear film; exposure changes never restart tracing.
pub struct ViewerState {
    pub image_width: u32,
    pub image_height: u32,
    pub pixels: Vec<Vec3>,
    pub filtered_pixels: Option<Vec<Vec3>>,
    pub show_filtered: bool,
    pub filtered_samples: u32,
    pub scene: String,
    pub backend: String,
    pub device: String,
    pub status: String,
    pub exposure: f32,
    pub progress: f32,
    pub completed_samples: u64,
    pub target_samples: u32,
    pub pass_samples: u32,
    pub seconds: f64,
    pub primitives: usize,
    pub bvh_nodes: usize,
    pub bounces: u32,
    pub paused: bool,
    pub finished: bool,
    pub last_tile: Option<Tile>,
    pub notice: String,
}
impl ViewerState {
    pub fn display_pixels(&self) -> &[Vec3] {
        if self.show_filtered {
            self.filtered_pixels.as_deref().unwrap_or(&self.pixels)
        } else {
            &self.pixels
        }
    }

    pub fn new(
        width: u32,
        height: u32,
        samples: u32,
        bounces: u32,
        scene: &str,
        exposure: f32,
    ) -> Self {
        Self {
            image_width: width,
            image_height: height,
            pixels: vec![Vec3::ZERO; width as usize * height as usize],
            filtered_pixels: None,
            show_filtered: true,
            filtered_samples: 0,
            scene: scene.into(),
            backend: "INITIALIZING".into(),
            device: String::new(),
            status: "BUILDING SCENE".into(),
            exposure,
            progress: 0.0,
            completed_samples: 0,
            target_samples: samples,
            pass_samples: 0,
            seconds: 0.0,
            primitives: 0,
            bvh_nodes: 0,
            bounces,
            paused: false,
            finished: false,
            last_tile: None,
            notice: String::new(),
        }
    }
}

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}
impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![BACKGROUND; width * height],
        }
    }
    fn pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color;
        }
    }
    fn rect(&mut self, rect: Rect, color: u32) {
        for y in rect.y..(rect.y + rect.height).min(self.height) {
            for x in rect.x..(rect.x + rect.width).min(self.width) {
                self.pixels[y * self.width + x] = color;
            }
        }
    }
    fn line(&mut self, x: usize, y: usize, width: usize, color: u32) {
        self.rect(
            Rect {
                x,
                y,
                width,
                height: 1,
            },
            color,
        );
    }
    fn text(&mut self, x: usize, y: usize, text: &str, color: u32, scale: usize) {
        for (index, character) in text.chars().enumerate() {
            let Some(glyph) = BASIC_FONTS.get(character) else {
                continue;
            };
            for (row, bits) in glyph.iter().enumerate() {
                for column in 0..8 {
                    if bits & (1 << column) != 0 {
                        self.rect(
                            Rect {
                                x: x + (index * 8 + column) * scale,
                                y: y + row * scale,
                                width: scale,
                                height: scale,
                            },
                            color,
                        );
                    }
                }
            }
        }
    }
}
fn color(rgb: Vec3, exposure: f32) -> u32 {
    let rgb = tonemap(rgb, exposure);
    ((rgb.x * 255.0 + 0.5) as u32) << 16
        | ((rgb.y * 255.0 + 0.5) as u32) << 8
        | (rgb.z * 255.0 + 0.5) as u32
}
fn time(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// Draw the same frame for a native window or an exported UI preview.
pub fn draw(state: &ViewerState, width: usize, height: usize) -> (Vec<u32>, Vec<Button>) {
    let mut canvas = Canvas::new(width, height);
    let sidebar_width = 272.min(width / 2);
    let sidebar_x = width.saturating_sub(sidebar_width);
    canvas.rect(
        Rect {
            x: 0,
            y: 0,
            width,
            height: 76,
        },
        PANEL,
    );
    canvas.text(24, 22, "LUMEN", TEXT, 3);
    canvas.text(168, 29, "PROGRESSIVE RENDER", MUTED, 1);
    canvas.text(sidebar_x + 20, 24, &state.backend.to_uppercase(), ACCENT, 2);
    canvas.text(
        sidebar_x + 20,
        50,
        &state.device.chars().take(29).collect::<String>(),
        MUTED,
        1,
    );
    canvas.line(0, 74, width, 0x30363b);
    canvas.rect(
        Rect {
            x: 0,
            y: 74,
            width: (sidebar_x as f32 * state.progress) as usize,
            height: 2,
        },
        ACCENT,
    );
    canvas.rect(
        Rect {
            x: sidebar_x,
            y: 76,
            width: sidebar_width,
            height: height.saturating_sub(76),
        },
        PANEL,
    );

    let available_width = sidebar_x.saturating_sub(48).max(1);
    let available_height = height.saturating_sub(164).max(1);
    let scale = (available_width as f32 / state.image_width as f32)
        .min(available_height as f32 / state.image_height as f32);
    let display_width = (state.image_width as f32 * scale).max(1.0) as usize;
    let display_height = (state.image_height as f32 * scale).max(1.0) as usize;
    let origin_x = sidebar_x.saturating_sub(display_width) / 2;
    let origin_y = 100 + available_height.saturating_sub(display_height) / 2;
    for y in 0..display_height {
        for x in 0..display_width {
            let source_x = ((x as f32 / scale) as u32).min(state.image_width - 1);
            let source_y = ((y as f32 / scale) as u32).min(state.image_height - 1);
            let index = (source_y * state.image_width + source_x) as usize;
            canvas.pixel(
                origin_x + x,
                origin_y + y,
                color(state.display_pixels()[index], state.exposure),
            );
        }
    }
    if let Some(tile) = state
        .last_tile
        .filter(|_| !state.finished && !state.paused && state.status == "RENDERING")
    {
        let x = origin_x + (tile.x as f32 * scale) as usize;
        let y = origin_y + (tile.y as f32 * scale) as usize;
        let w = (tile.width as f32 * scale).max(1.0) as usize;
        let h = (tile.height as f32 * scale).max(1.0) as usize;
        canvas.line(x, y, w, ACCENT);
        canvas.line(x, y + h.saturating_sub(1), w, ACCENT);
        canvas.rect(
            Rect {
                x,
                y,
                width: 1,
                height: h,
            },
            ACCENT,
        );
        canvas.rect(
            Rect {
                x: x + w.saturating_sub(1),
                y,
                width: 1,
                height: h,
            },
            ACCENT,
        );
    }
    canvas.text(
        24,
        height.saturating_sub(45),
        &format!(
            "{} x {}  /  PASS {} SPP  /  CLEAN {} SPP  /  {} BOUNCES",
            state.image_width,
            state.image_height,
            state.pass_samples,
            state.filtered_samples,
            state.bounces
        ),
        MUTED,
        1,
    );
    canvas.text(
        24,
        height.saturating_sub(25),
        "SPACE pause  R restart  S save  D raw/clean  +/- exposure  ESC stop",
        MUTED,
        1,
    );

    let left = sidebar_x + 20;
    let row_width = sidebar_width.saturating_sub(40);
    canvas.text(left, 94, "SCENE", MUTED, 1);
    let mut buttons = Vec::new();
    for (i, name) in ["studio", "interior", "outdoor", "clouds"]
        .iter()
        .enumerate()
    {
        let rect = Rect {
            x: left,
            y: 110 + i * 30,
            width: row_width,
            height: 24,
        };
        let selected = state.scene == *name;
        canvas.rect(rect, if selected { 0x334337 } else { 0x252b30 });
        canvas.text(
            left + 12,
            rect.y + 10,
            &name.to_uppercase(),
            if selected { ACCENT } else { TEXT },
            1,
        );
        buttons.push(Button {
            rect,
            action: Action::Scene(i),
        });
    }
    canvas.line(left, 230, row_width, 0x353a3e);
    canvas.text(left, 250, &state.status, ACCENT, 1);
    if !state.finished {
        let activity_x = left + ((state.seconds * 35.0) as usize % row_width.max(1));
        canvas.line(
            activity_x,
            265,
            6.min(left + row_width - activity_x),
            ACCENT,
        );
    }
    canvas.text(
        left,
        277,
        &format!("{:5.1}%", state.progress * 100.0),
        TEXT,
        3,
    );
    canvas.rect(
        Rect {
            x: left,
            y: 312,
            width: row_width,
            height: 4,
        },
        0x353a3e,
    );
    canvas.rect(
        Rect {
            x: left,
            y: 312,
            width: (row_width as f32 * state.progress) as usize,
            height: 4,
        },
        ACCENT,
    );
    let average =
        state.completed_samples as f64 / f64::from(state.image_width * state.image_height);
    let rate = state.completed_samples as f64 / state.seconds.max(0.001) / 1e6;
    let eta = if state.progress > 0.001 {
        time(state.seconds * (1.0 - f64::from(state.progress)) / f64::from(state.progress))
    } else {
        "--:--".into()
    };
    let rows = [
        (
            "SAMPLES / PIXEL",
            format!("{average:.0} / {}", state.target_samples),
        ),
        ("ELAPSED", time(state.seconds)),
        (
            "REMAINING ~",
            if state.finished { "00:00".into() } else { eta },
        ),
        ("CAMERA SAMPLES/S", format!("{rate:.2} M")),
        ("EXPOSURE", format!("{:+.1} EV", state.exposure)),
        ("PRIMITIVES", state.primitives.to_string()),
        ("BVH NODES", state.bvh_nodes.to_string()),
    ];
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = 338 + i * 39;
        canvas.text(left, y, label, MUTED, 1);
        canvas.text(left, y + 15, value, TEXT, 2);
    }
    let y = 626;
    for (i, (label, action)) in [
        (
            if state.paused {
                "RESUME  [SPACE]"
            } else {
                "PAUSE   [SPACE]"
            },
            Action::TogglePause,
        ),
        ("RESTART [R]", Action::Restart),
        ("SAVE PNG [S]", Action::Save),
        (
            if state.show_filtered {
                "VIEW RAW [D]"
            } else {
                "VIEW CLEAN [D]"
            },
            Action::ToggleDenoise,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect {
            x: left,
            y: y + i * 36,
            width: row_width,
            height: 28,
        };
        canvas.rect(rect, 0x30383d);
        canvas.text(left + 10, rect.y + 10, label, TEXT, 1);
        buttons.push(Button { rect, action });
    }
    if !state.notice.is_empty() {
        canvas.text(
            left,
            784,
            &state.notice.chars().take(28).collect::<String>(),
            ACCENT,
            1,
        );
    }
    (canvas.pixels, buttons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_preview_keeps_tile_activity_visible() {
        let mut state = ViewerState::new(320, 200, 512, 12, "interior", 0.0);
        state.filtered_pixels = Some(vec![Vec3::splat(0.4); 320 * 200]);
        state.filtered_samples = 128;
        state.status = "RENDERING".into();
        state.last_tile = Some(Tile {
            x: 64,
            y: 64,
            width: 64,
            height: 64,
        });
        let (active, _) = draw(&state, 1280, 820);
        state.last_tile = None;
        let (inactive, _) = draw(&state, 1280, 820);
        let outline = active
            .iter()
            .zip(inactive)
            .filter(|(a, b)| **a != *b && **a == ACCENT)
            .count();
        assert!(
            outline > 300,
            "Clean display must retain a visible tile border"
        );
        assert_eq!(state.display_pixels()[0], Vec3::splat(0.4));
    }
}
