//! Full HD close-up of the built-in neon sign, using the same materials and transport.
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
        .unwrap_or(128);
    let settings = RenderSettings {
        samples,
        max_depth: 24,
        ..Default::default()
    };
    settings.validate()?;
    let mut scene = scene::preset("interior-neon", 16.0 / 9.0)?;
    // Stay beyond the pendant's glass/cane envelope, without hiding any geometry.
    let from = Vec3::new(2.0, 2.345, -1.5);
    let to = Vec3::new(4.425, 2.345, -1.5);
    scene.camera = Camera::new(from, to, 35.0, 16.0 / 9.0, 0.0, from.distance(to));
    #[cfg(target_os = "macos")]
    let mut rendered = lumen_rs::metal_backend::render(&scene, settings)?;
    #[cfg(not(target_os = "macos"))]
    let mut rendered = lumen_rs::render::cpu(&scene, settings)?;
    ensure!(rendered.stats.invalid_samples == 0, "Non-finite samples");
    output::save(
        &rendered,
        Path::new("renders/neon-detail-raw.png"),
        0.0,
        true,
    )?;
    let started = Instant::now();
    let guides = Guides::new(&scene, settings.width, settings.height);
    rendered.pixels =
        Denoiser::new()?.filter(&rendered.pixels, &guides, settings.width, settings.height)?;
    rendered.stats.denoiser = Some("Open Image Denoise 2.x / CPU / high".into());
    rendered.stats.denoise_seconds = started.elapsed().as_secs_f64();
    output::save(&rendered, Path::new("renders/neon-detail.png"), 0.0, true)?;
    Ok(())
}
