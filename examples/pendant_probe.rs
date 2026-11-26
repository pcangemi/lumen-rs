//! Measure pendant visibility and local path-depth convergence before tone mapping.
use anyhow::{Context, Result, ensure};
use glam::Vec3;
use lumen_rs::{
    geometry::{Ray, offset, unpack_vec3},
    integrator::trace,
    rng::{Rng, luminance},
    scene,
};

fn main() -> Result<()> {
    let scene = scene::preset("interior-night", 16.0 / 9.0)?;
    // The compact pendant core is the only high-radiance source in this preset.
    // Fail if the scene changes rather than silently measuring a different lamp.
    let mut pendant_sources = scene.lights.iter().filter(|light| {
        let primitive = &scene.primitives[light.primitive as usize];
        unpack_vec3(scene.materials[primitive.info[1] as usize].emission).max_element() > 1000.0
    });
    let light = pendant_sources.next().context("Missing pendant emitter")?;
    ensure!(
        pendant_sources.next().is_none(),
        "Ambiguous pendant emitter"
    );
    let primitive = &scene.primitives[light.primitive as usize];
    let emission = unpack_vec3(scene.materials[primitive.info[1] as usize].emission);
    let mut reports = Vec::new();
    for (name, center, normal) in [
        ("Rug below pendant", Vec3::new(0.2, 0.11, -1.35), Vec3::Y),
        ("Rug in front of table", Vec3::new(0.5, 0.11, -0.8), Vec3::Y),
        ("Rug foreground", Vec3::new(1.0, 0.11, 0.6), Vec3::Y),
        ("Under coffee table", Vec3::new(0.5, 0.11, -1.8), Vec3::Y),
        ("Left parquet", Vec3::new(-3.0, 0.03, -1.0), Vec3::Y),
        ("Foreground parquet", Vec3::new(0.2, 0.03, 1.7), Vec3::Y),
        (
            "Ceiling beside pendant",
            Vec3::new(1.2, 3.79, -0.85),
            -Vec3::Y,
        ),
    ] {
        let mut incoming = Vec3::ZERO;
        let mut unobstructed = Vec3::ZERO;
        for sample in 0..4096 {
            let mut rng = Rng::new(0, sample, 42);
            let (u, v) = rng.disk();
            let point = center + Vec3::new(u, 0.0, v) * 0.08;
            let (target, light_normal) = primitive.sample(&mut rng);
            let difference = target - point;
            let distance_squared = difference.length_squared();
            let direction = difference.normalize();
            let cosine = normal.dot(direction).max(0.0);
            let cosine_light = light_normal.dot(-direction).max(0.0);
            let value = emission * (cosine * cosine_light * light.area / distance_squared);
            unobstructed += value;
            let end = offset(target, light_normal, -direction);
            let shadow = end - point;
            let visibility = scene.shadow_transmittance(
                Ray {
                    origin: point,
                    direction: shadow.normalize(),
                },
                shadow.length() - 1e-5,
            );
            incoming += value * visibility;
        }
        incoming /= 4096.0;
        unobstructed /= 4096.0;
        let mut radiance_12 = Vec3::ZERO;
        let mut radiance_50 = Vec3::ZERO;
        let mut difference_mean = 0.0f64;
        let mut difference_m2 = 0.0f64;
        for sample in 0..4096 {
            let mut rng = Rng::new(1, sample, 42);
            let (u, v) = rng.disk();
            let ray = Ray {
                origin: center + Vec3::new(u, 0.0, v) * 0.08,
                direction: -normal,
            };
            // Paired paths share random numbers until the shorter depth limit.
            // The measured view is normal to the patch, not the scene camera.
            let short = trace(&scene, ray, &mut rng.clone(), 12);
            let long = trace(&scene, ray, &mut rng, 50);
            radiance_12 += short;
            radiance_50 += long;
            let difference = f64::from(luminance(long - short));
            let delta = difference - difference_mean;
            difference_mean += delta / f64::from(sample + 1);
            difference_m2 += delta * (difference - difference_mean);
        }
        radiance_12 /= 4096.0;
        radiance_50 /= 4096.0;
        reports.push(serde_json::json!({
            "location": name, "position": center.to_array(),
            "direct_irradiance_scene_units": incoming.to_array(),
            "unobstructed_irradiance_scene_units": unobstructed.to_array(),
            "fraction_after_glass_and_occlusion": luminance(incoming) / luminance(unobstructed),
            "all_lights_normal_view_radiance": {
                "depth_12": radiance_12.to_array(),
                "depth_50": radiance_50.to_array(),
                "paired_luminance_difference": difference_mean,
                "paired_difference_standard_error": (difference_m2 / 4095.0 / 4096.0).sqrt(),
            },
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "scope": "Direct pendant irradiance over 8 cm radius patches above surfaces; excludes indirect light and surface BRDF. Values are scene units, not calibrated lux.",
            "path_depth_scope": "Normal-view surface radiance from all lights and indirect transport, paired at 12 and 50 interactions; independent of camera exposure and denoising. Neither depth is an infinite-depth reference.",
            "samples_per_patch": 4096, "measurements": reports,
        }))?
    );
    Ok(())
}
