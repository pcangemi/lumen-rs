//! Display transform and lossless PNG / linear OpenEXR output.
use crate::render::RenderOutput;
use anyhow::Result;
use glam::Vec3;
use std::path::Path;

fn srgb(x: f32) -> f32 {
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}
pub fn tonemap(c: Vec3, exposure: f32) -> Vec3 {
    let x = c.max(Vec3::ZERO) * 2.0f32.powf(exposure);
    let mapped = ((x * (2.51 * x + Vec3::splat(0.03)))
        / (x * (2.43 * x + Vec3::splat(0.59)) + Vec3::splat(0.14)))
    .clamp(Vec3::ZERO, Vec3::ONE);
    Vec3::new(srgb(mapped.x), srgb(mapped.y), srgb(mapped.z))
}
pub fn save(render: &RenderOutput, path: &Path, exposure: f32, exr: bool) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?
    }
    let data: Vec<u8> = render
        .pixels
        .iter()
        .flat_map(|c| {
            let c = tonemap(*c, exposure);
            [
                (c.x * 255.0 + 0.5) as u8,
                (c.y * 255.0 + 0.5) as u8,
                (c.z * 255.0 + 0.5) as u8,
            ]
        })
        .collect();
    image::save_buffer_with_format(
        path,
        &data,
        render.stats.width,
        render.stats.height,
        image::ColorType::Rgb8,
        image::ImageFormat::Png,
    )?;
    if exr {
        let data: Vec<f32> = render.pixels.iter().flat_map(|c| [c.x, c.y, c.z]).collect();
        let im =
            image::Rgb32FImage::from_raw(render.stats.width, render.stats.height, data).unwrap();
        im.save_with_format(path.with_extension("exr"), image::ImageFormat::OpenExr)?;
    }
    std::fs::write(
        path.with_extension("json"),
        serde_json::to_string_pretty(&render.stats)?,
    )?;
    Ok(())
}
