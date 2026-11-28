//! Full HD architectural checks from a street-facing viewpoint or beside a railing.
use anyhow::{Result, ensure};
use glam::Vec3;
use lumen_rs::{
    camera::Camera,
    denoise::{Denoiser, Guides},
    navigation::Viewpoint,
    output,
    render::RenderSettings,
    scene,
};
use std::{path::Path, time::Instant};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("night");
    ensure!(
        matches!(mode, "day" | "night" | "railing"),
        "Choose day, night or railing"
    );
    let samples = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(128);
    let settings = RenderSettings {
        samples,
        max_depth: 16,
        ..Default::default()
    };
    settings.validate()?;
    let name = if mode == "day" || mode == "railing" {
        "interior"
    } else {
        "interior-neon"
    };
    let mut scene = scene::preset(name, 16.0 / 9.0)?;
    let (from, target, fov) = if mode == "railing" {
        (Vec3::new(-3.5, 1.95, 1.1), Vec3::new(-4.8, 1.12, 0.1), 65.0)
    } else {
        (Vec3::new(-5.9, 2.2, 0.8), Vec3::new(-15.4, 6.2, 0.0), 82.0)
    };
    scene.camera = Camera::new(from, target, fov, 16.0 / 9.0, 0.0, from.distance(target));
    let base = format!("renders/haussmann-{mode}");
    Viewpoint {
        scene: name.into(),
        camera: scene.camera,
        exposure: 0.0,
    }
    .save(Path::new(&format!("{base}-view.json")))?;
    #[cfg(target_os = "macos")]
    let mut result = lumen_rs::metal_backend::render(&scene, settings)?;
    #[cfg(not(target_os = "macos"))]
    let mut result = lumen_rs::render::cpu(&scene, settings)?;
    ensure!(result.stats.invalid_samples == 0, "Non-finite samples");
    output::save(&result, Path::new(&format!("{base}-raw.png")), 0.0, true)?;
    let started = Instant::now();
    result.pixels = Denoiser::new()?.filter(
        &result.pixels,
        &Guides::new(&scene, settings.width, settings.height),
        settings.width,
        settings.height,
    )?;
    result.stats.denoiser = Some("Open Image Denoise 2.x / CPU / high".into());
    result.stats.denoise_seconds = started.elapsed().as_secs_f64();
    output::save(&result, Path::new(&format!("{base}.png")), 0.0, true)?;
    Ok(())
}
