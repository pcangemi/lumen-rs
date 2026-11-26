//! A full-resolution glass guardian lion on its own round wool display rug.
use super::*;

const CENTER: Vec3 = Vec3::new(1.20, 0.0, 2.20);
const HEIGHT: f32 = 1.90;
const BASE_Y: f32 = 0.024;
const YAW: f32 = -65.0;
const RUG_RADIUS: f32 = 0.88;
const MODEL: &str = "models/guardian_lion/guardian_lion.obj";

pub(super) fn build(scene: &mut Scene, root: &Path, linen: usize) -> Result<()> {
    round_rug(scene, linen);
    let glass = scene.add_material(Material::glass(1.52, Vec3::new(0.006, 0.003, 0.004)));
    scene.import_obj(
        &root.join(MODEL),
        CENTER + Vec3::Y * BASE_Y,
        HEIGHT,
        YAW,
        Some(glass),
    )?;
    Ok(())
}

fn round_rug(scene: &mut Scene, linen: usize) {
    let wool = scene.add_material(
        Material::matte(Vec3::new(0.31, 0.27, 0.22))
            .textured(linen, -18.0)
            .texture_strength(0.18),
    );
    lathe_segments(
        scene,
        CENTER,
        &[
            (0.0, 0.010),
            (RUG_RADIUS - 0.008, 0.010),
            (RUG_RADIUS - 0.003, 0.013),
            (RUG_RADIUS - 0.003, 0.019),
            (RUG_RADIUS - 0.008, 0.022),
            (0.0, 0.022),
        ],
        wool,
        192,
    );
    let binding: Vec<_> = (0..=256)
        .map(|i| {
            let angle = i as f32 * TAU / 256.0;
            CENTER
                + Vec3::new(
                    (RUG_RADIUS - 0.006) * angle.cos(),
                    0.020,
                    (RUG_RADIUS - 0.006) * angle.sin(),
                )
        })
        .collect();
    details::tube(scene, &binding, 0.006, wool, 8);

    let yarn: Vec<_> = (0..6)
        .map(|i| {
            scene.add_material(Material {
                surface: [0.96, 0.0, 0.0, 1.5],
                ..Material::matte(Vec3::new(0.34, 0.30, 0.25) * (0.93 + i as f32 * 0.025))
                    .textured(linen, -35.0)
                    .texture_strength(0.12)
            })
        })
        .collect();
    let inverse_rotation = Mat3::from_rotation_y(-YAW.to_radians());
    let mut rng = Rng::new(193, 0, 42);
    // Short, rounded yarn loops provide actual relief. The pile under the
    // pedestal is compressed to its support plane rather than piercing glass.
    let spacing = 0.012;
    let extent = (RUG_RADIUS / spacing).ceil() as i32;
    for column in -extent..=extent {
        for row in -extent..=extent {
            let p = Vec3::new(
                (column as f32 + rng.uniform() * 0.6) * spacing,
                0.0,
                (row as f32 + rng.uniform() * 0.6) * spacing,
            );
            if p.length() > RUG_RADIUS - 0.018 {
                continue;
            }
            let local = inverse_rotation * p;
            let under_base =
                local.x.abs() < HEIGHT * 0.24 + 0.006 && local.z.abs() < HEIGHT * 0.34 + 0.006;
            let top = if under_base {
                BASE_Y
            } else {
                0.027 + rng.uniform() * 0.003
            };
            let angle = rng.uniform() * TAU;
            let tangent = Vec3::new(angle.cos(), 0.0, angle.sin());
            let material = yarn[rng.next_u32() as usize % yarn.len()];
            let radius = 0.0018;
            for segment in 0..8 {
                let theta = segment as f32 * TAU / 8.0;
                let height = 0.019 + (top - radius - 0.019) * (0.5 + 0.5 * theta.sin());
                scene.primitives.push(Primitive::sphere(
                    CENTER + p + tangent * (0.0038 * theta.cos()) + Vec3::Y * height,
                    radius,
                    material,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Ray, offset};

    #[test]
    fn glass_sculpture_has_real_scale_closed_shell_and_separate_rug() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let mut scene = Scene::empty(Camera::new(Vec3::Z * 6.0, Vec3::ZERO, 54.0, 1.5, 0.0, 6.0));
        let linen = scene
            .textures
            .load(&root.join("textures/linen.jpg"))
            .unwrap();
        build(&mut scene, &root, linen).unwrap();
        let glass = scene
            .materials
            .iter()
            .position(|m| m.surface[2] == 1.0)
            .unwrap();
        let (lo, hi) = scene
            .primitives
            .iter()
            .filter(|p| p.info[1] == glass as u32)
            .fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(lo, hi), p| {
                    let bounds = p.bounds();
                    (lo.min(bounds.0), hi.max(bounds.1))
                },
            );
        assert!((hi.y - lo.y - HEIGHT).abs() < 0.0001);
        assert!((lo.y - BASE_Y).abs() < 0.0001);
        assert_eq!(scene.materials[glass].base[..3], [1.0; 3]);
        assert_eq!(scene.materials[glass].surface[3], 1.52);
        // Include the binding and every yarn loop in the actual clearance check.
        for p in &scene.primitives {
            let (lo, hi) = p.bounds();
            assert!(
                lo.z > 1.29,
                "Keep at least 29 cm beyond the main rug's front edge"
            );
            assert!(
                hi.x < 4.21,
                "Keep the artwork clear of the wall and skirting"
            );
            assert!(lo.y >= 0.0099);
        }
        scene.primitives.retain(|p| p.info[1] == glass as u32);
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("models/guardian_lion/conversion.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            scene.primitives.len() as u64,
            report["triangles"].as_u64().unwrap(),
            "Import must retain every repaired triangle, including fine carving"
        );
        scene.finish().unwrap();
        // Probe entry/exit orientation through the actual sculpture on three axes.
        // Every crossing must alternate air/glass and finish in air.
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for i in 0..9 {
                for j in 0..9 {
                    let mut origin = (lo + hi) * 0.5;
                    origin[axis] = lo[axis] - 0.1;
                    origin[u] = lo[u] + (hi[u] - lo[u]) * (i as f32 + 0.37) / 9.0;
                    origin[v] = lo[v] + (hi[v] - lo[v]) * (j as f32 + 0.63) / 9.0;
                    let mut direction = Vec3::ZERO;
                    direction[axis] = 1.0;
                    let mut ray = Ray { origin, direction };
                    let mut inside = false;
                    let mut crossings = 0;
                    while let Some(hit) = scene.intersect(ray, f32::INFINITY, false) {
                        assert_eq!(
                            hit.front, !inside,
                            "Invalid shell crossing on axis {axis}, probe {i},{j}"
                        );
                        inside = !inside;
                        crossings += 1;
                        assert!(crossings < 64);
                        ray.origin = offset(hit.p, hit.geometric, direction);
                    }
                    assert!(!inside, "Open glass shell");
                }
            }
        }
    }
}
