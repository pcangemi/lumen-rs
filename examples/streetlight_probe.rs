//! Identify actual street-lamp reflection paths in the apartment camera.
use anyhow::{Result, ensure};
use glam::Vec3;
use lumen_rs::{
    geometry::{Ray, offset, unpack_vec3},
    material::thin_glass_transmittance,
    rng::Rng,
    scene::{self, Scene},
};
use std::collections::BTreeMap;

struct ReflectionPath {
    source: usize,
    vertices: Vec<Vec3>,
}

/// Search for one reflection plus straight transmission through thin glass.
/// Opaque and solid-refractive interactions terminate this restricted probe.
fn reflected_source(
    scene: &Scene,
    ray: Ray,
    reflected: bool,
    depth: u32,
) -> Option<ReflectionPath> {
    if depth == 0 {
        return None;
    }
    let hit = scene.intersect(ray, f32::INFINITY, false)?;
    let material = &scene.materials[hit.material];
    if material.is_light() {
        return (reflected
            && hit.p.x < -5.0
            && hit.p.y < 0.0
            && unpack_vec3(material.emission).max_element() > 100.0)
            .then(|| ReflectionPath {
                source: hit.primitive,
                vertices: vec![hit.p],
            });
    }
    if !material.is_thin_glass() {
        return None;
    }
    let cosine = -ray.direction.dot(hit.geometric);
    let transmission = thin_glass_transmittance(cosine, material.surface[3]);
    for (direction, has_reflected, probability) in [
        (
            ray.direction + 2.0 * cosine * hit.geometric,
            true,
            if reflected { 0.0 } else { 1.0 - transmission },
        ),
        (ray.direction, reflected, transmission),
    ] {
        if probability <= 0.0 {
            continue;
        }
        let next = Ray {
            origin: offset(hit.p, hit.geometric, direction),
            direction,
        };
        if let Some(mut path) = reflected_source(scene, next, has_reflected, depth - 1) {
            path.vertices.insert(0, hit.p);
            return Some(path);
        }
    }
    None
}

fn main() -> Result<()> {
    let scene = scene::preset("interior-night", 16.0 / 9.0)?;
    let mut sources = BTreeMap::new();
    let mut first_path = None;
    let mut camera_samples = 0u64;
    for (left, top, width, height) in [(150, 510, 50, 100), (640, 510, 50, 70)] {
        for y in top..top + height {
            for x in left..left + width {
                for sample in 0..8 {
                    let mut rng = Rng::new(y * 1920 + x, sample, 42);
                    let ray = scene.camera.ray(x, y, 1920, 1080, &mut rng);
                    camera_samples += 1;
                    if let Some(path) = reflected_source(&scene, ray, false, 8) {
                        *sources.entry(path.source).or_insert(0u64) += 1;
                        first_path.get_or_insert(serde_json::json!({
                            "pixel": [x, y],
                            "camera": unpack_vec3(scene.camera.origin).to_array(),
                            "surface_vertices": path.vertices.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
                        }));
                    }
                }
            }
        }
    }
    ensure!(
        !sources.is_empty(),
        "No street-lamp reflection path found in the diagnostic regions"
    );
    let sources: Vec<_> = sources.into_iter().map(|(index, count)| {
        let (lo, hi) = scene.primitives[index].bounds();
        serde_json::json!({ "source_center": ((lo + hi) * 0.5).to_array(), "camera_samples_with_path": count })
    }).collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "scope": "Geometric existence of single-reflection paths through thin glazing to public lamps; not an image-error or energy-convergence measurement.",
            "film": [1920, 1080], "camera_samples": camera_samples,
            "sources": sources, "example_path": first_path,
        }))?
    );
    Ok(())
}
