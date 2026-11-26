//! Render a pixel region of the Full HD camera for focused scene diagnostics.
use anyhow::{Context, Result, ensure};
use lumen_rs::{
    denoise::{Denoiser, Guides},
    geometry::{pack_vec3, unpack_vec3},
    output,
    render::RenderSettings,
    scene,
};
use std::{env, path::Path, time::Instant};

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    ensure!(
        args.len() == 8,
        "Usage: render_region scene left top width height samples output.png"
    );
    let values: Vec<u32> = args[2..7]
        .iter()
        .map(|value| value.parse().context("Invalid region or sample count"))
        .collect::<Result<_>>()?;
    let (left, top, width, height, samples) =
        (values[0], values[1], values[2], values[3], values[4]);
    ensure!(
        u64::from(left) + u64::from(width) <= 1920 && u64::from(top) + u64::from(height) <= 1080,
        "Region must lie within the 1920 x 1080 film"
    );
    let settings = RenderSettings {
        width,
        height,
        samples,
        ..RenderSettings::default()
    };
    settings.validate()?;
    let mut scene = scene::preset(&args[1], 16.0 / 9.0)?;
    let camera = &mut scene.camera;
    let center_x = (2.0 * left as f32 + width as f32) / 1920.0 - 1.0;
    let center_y = 1.0 - (2.0 * top as f32 + height as f32) / 1080.0;
    // Shift and narrow the original frustum without changing its pixel footprint.
    // Keep its lens basis and focus plane; this also preserves thin-lens rays.
    camera.forward = pack_vec3(
        unpack_vec3(camera.forward)
            + center_x * camera.right[3] * unpack_vec3(camera.right)
            + center_y * camera.up[3] * unpack_vec3(camera.up),
        camera.forward[3],
    );
    camera.right[3] *= width as f32 / 1920.0;
    camera.up[3] *= height as f32 / 1080.0;

    #[cfg(target_os = "macos")]
    let mut rendered = lumen_rs::metal_backend::render(&scene, settings)?;
    #[cfg(not(target_os = "macos"))]
    let mut rendered = lumen_rs::render::cpu(&scene, settings)?;
    ensure!(rendered.stats.invalid_samples == 0, "Non-finite samples");
    let destination = Path::new(&args[7]);
    let stem = destination.file_stem().context("Missing output name")?;
    let raw = destination.with_file_name(format!("{}-raw.png", stem.to_string_lossy()));
    output::save(&rendered, &raw, 0.0, true)?;
    let started = Instant::now();
    let guides = Guides::new(&scene, width, height);
    rendered.pixels = Denoiser::new()?.filter(&rendered.pixels, &guides, width, height)?;
    rendered.stats.denoiser = Some("Open Image Denoise 2.x / CPU / high".into());
    rendered.stats.denoise_seconds = started.elapsed().as_secs_f64();
    output::save(&rendered, destination, 0.0, true)?;
    Ok(())
}
