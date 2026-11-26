//! Portable scene snapshots for reproducible comparisons with other renderers.
use crate::{geometry::unpack_vec3, render::RenderSettings, scene::Scene};
use anyhow::{Context, Result, ensure};
use glam::{Vec2, Vec3};
use serde_json::json;
use std::{fs::File, io::BufWriter, path::Path};

/// Export the actual assembled geometry, camera and linear texture atlas.
/// Shader models must still be reconciled by the receiving renderer.
pub fn export(scene: &Scene, settings: RenderSettings, path: &Path) -> Result<()> {
    ensure!(
        scene.medium.is_none(),
        "Cycles scene export does not yet support participating media"
    );
    std::fs::create_dir_all(path)?;
    let mut textures = Vec::new();
    for (index, &[offset, width, height, _]) in scene.textures.descriptors.iter().enumerate() {
        let pixels: Vec<f32> = scene.textures.pixels
            [offset as usize..offset as usize + width as usize * height as usize]
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        let filename = format!("texture-{index}.exr");
        image::Rgb32FImage::from_raw(width, height, pixels)
            .context("Texture dimensions do not match")?
            .save_with_format(path.join(&filename), image::ImageFormat::OpenExr)?;
        textures.push(filename);
    }
    let environment = image::Rgb32FImage::from_fn(512, 256, |_, y| {
        let elevation = ((y as f32 + 0.5) / 256.0 * std::f32::consts::PI).cos();
        image::Rgb(scene.environment(Vec3::new(0.0, elevation, 0.0)).to_array())
    });
    environment.save_with_format(path.join("environment.exr"), image::ImageFormat::OpenExr)?;
    let triangles: Vec<_> = scene
        .primitives
        .iter()
        .filter(|p| p.info[0] == 1)
        .map(|p| {
            let origin = unpack_vec3(p.p0);
            let positions = [
                origin,
                origin + unpack_vec3(p.p1),
                origin + unpack_vec3(p.p2),
            ];
            let normals = [unpack_vec3(p.n0), unpack_vec3(p.n1), unpack_vec3(p.n2)];
            let original_uvs = [
                Vec2::new(p.uv01[0], p.uv01[1]),
                Vec2::new(p.uv01[2], p.uv01[3]),
                Vec2::new(p.uv2[0], p.uv2[1]),
            ];
            let material = &scene.materials[p.info[1] as usize];
            let uvs = [0, 1, 2].map(|i| {
                if material.detail[3] >= 0.0 {
                    return original_uvs[i];
                }
                let n = normals[i].abs();
                let position = positions[i];
                if n.y >= n.x && n.y >= n.z {
                    Vec2::new(position.x, position.z)
                } else if n.x > n.z {
                    Vec2::new(position.z, position.y)
                } else {
                    Vec2::new(position.x, position.y)
                }
            });
            let colors = positions.map(|position| material.color(position).to_array());
            json!({
                "colors": colors,
                "positions": positions.map(|position| position.to_array()),
                "normals": normals.map(|normal| normal.to_array()),
                "uvs": uvs.map(|uv| uv.to_array()),
                "material": p.info[1],
            })
        })
        .collect();
    let spheres: Vec<_> = scene.primitives.iter().filter(|p| p.info[0] == 0).map(|p| {
        json!({"center": unpack_vec3(p.p0).to_array(), "radius": p.p0[3], "material": p.info[1]})
    }).collect();
    let materials: Vec<_> = scene
        .materials
        .iter()
        .map(|m| {
            let mut detail = m.detail;
            detail[3] = detail[3].abs();
            json!({
                "base": m.base, "surface": m.surface, "emission": m.emission,
                "absorption": m.absorption, "detail": detail, "maps": m.maps,
            })
        })
        .collect();
    let data = json!({
        "schema": 1,
        "camera": {"origin": scene.camera.origin, "forward": scene.camera.forward,
                   "right": scene.camera.right, "up": scene.camera.up},
        "sky": scene.sky.to_array(), "horizon": scene.horizon.to_array(),
        "width": settings.width, "height": settings.height, "samples": settings.samples,
        "max_depth": settings.max_depth, "seed": settings.seed,
        "triangles": triangles, "spheres": spheres, "materials": materials, "textures": textures,
        "differences": [
            "Cycles Principled BSDF approximates Lumen's Lambert plus single-scattering GGX mixture.",
            "Analytic spheres become tessellated spheres in Cycles.",
            "Procedural colors are baked at vertices; world projections become UVs and may differ on curved surfaces.",
            "Glass in Cycles uses its glass shader and absorption; shading-normal behavior may differ.",
            "Compare each renderer to its own high-sample reference before comparing time to quality."
        ]
    });
    serde_json::to_writer(
        BufWriter::new(File::create(path.join("scene.json"))?),
        &data,
    )?;
    Ok(())
}
