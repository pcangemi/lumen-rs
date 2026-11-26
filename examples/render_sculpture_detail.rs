//! Full HD view of the glass sculpture in its actual apartment lighting.
use anyhow::{Result, ensure};
use glam::Vec3;
use lumen_rs::{
    camera::Camera,
    denoise::{Denoiser, Guides},
    output,
    render::RenderSettings,
    scene,
};
use std::{path::Path, time::Instant};

fn main() -> Result<()> {
    let samples = std::env::args()
        .nth(1)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(256);
    let settings = RenderSettings {
        samples,
        max_depth: 50,
        ..Default::default()
    };
    settings.validate()?;
    let mut scene = scene::preset("interior-neon", 16.0 / 9.0)?;
    let from = Vec3::new(-2.75, 1.43, 4.18);
    let to = Vec3::new(1.20, 1.0, 2.20);
    scene.camera = Camera::new(from, to, 38.0, 16.0 / 9.0, 0.0, from.distance(to));
    #[cfg(target_os = "macos")]
    let mut rendered = lumen_rs::metal_backend::render(&scene, settings)?;
    #[cfg(not(target_os = "macos"))]
    let mut rendered = lumen_rs::render::cpu(&scene, settings)?;
    ensure!(rendered.stats.invalid_samples == 0, "Non-finite samples");
    output::save(
        &rendered,
        Path::new("renders/sculpture-detail-raw.png"),
        0.0,
        true,
    )?;
    let started = Instant::now();
    let guides = Guides::new(&scene, settings.width, settings.height);
    rendered.pixels =
        Denoiser::new()?.filter(&rendered.pixels, &guides, settings.width, settings.height)?;
    rendered.stats.denoiser = Some("Open Image Denoise 2.x / CPU / high".into());
    rendered.stats.denoise_seconds = started.elapsed().as_secs_f64();
    output::save(
        &rendered,
        Path::new("renders/sculpture-detail.png"),
        0.0,
        true,
    )?;
    Ok(())
}
