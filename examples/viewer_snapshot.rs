//! Reuse a finished linear render to document the viewer without tracing it again.
use anyhow::{Context, Result, ensure};
use glam::Vec3;
use lumen_rs::viewer_ui::{self, ViewerState};
use std::{env, fs::File, path::Path};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    ensure!(
        args.len() == 4,
        "Usage: viewer_snapshot image.exr stats.json output.png"
    );
    let image = image::open(&args[1])?.to_rgb32f();
    let stats: serde_json::Value = serde_json::from_reader(File::open(&args[2])?)?;
    let width = image.width();
    let height = image.height();
    let samples = stats["requested_spp"]
        .as_u64()
        .context("Missing sample count")? as u32;
    let scene = Path::new(&args[1])
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("interior");
    let mut state = ViewerState::new(
        width,
        height,
        samples,
        stats["max_depth"].as_u64().unwrap_or(12) as u32,
        scene,
        0.0,
    );
    state.pixels = image.pixels().map(|p| Vec3::from_array(p.0)).collect();
    state.backend = stats["backend"].as_str().unwrap_or("Image").to_uppercase();
    state.device = stats["device"].as_str().unwrap_or("").into();
    state.seconds = stats["render_seconds"].as_f64().unwrap_or(0.0)
        + stats["denoise_seconds"].as_f64().unwrap_or(0.0);
    state.completed_samples = (stats["average_spp"].as_f64().unwrap_or(samples as f64)
        * f64::from(width)
        * f64::from(height))
    .round() as u64;
    state.primitives = stats["primitives"].as_u64().unwrap_or(0) as usize;
    state.bvh_nodes = stats["bvh_nodes"].as_u64().unwrap_or(0) as usize;
    state.pass_samples = samples;
    if stats["denoiser"].is_string() {
        state.filtered_pixels = Some(state.pixels.clone());
        state.filtered_samples = samples;
    }
    state.progress = 1.0;
    state.finished = true;
    state.status = "COMPLETE".into();
    let (pixels, _) = viewer_ui::draw(&state, 1920, 1080);
    let bytes: Vec<_> = pixels
        .into_iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, p as u8])
        .collect();
    image::save_buffer(&args[3], &bytes, 1920, 1080, image::ColorType::Rgb8)?;
    Ok(())
}
