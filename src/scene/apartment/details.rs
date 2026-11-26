//! Architectural fittings and soft furnishings, measured in meters.
use super::*;

/// A tube with smooth radial normals, used for piping, curtain rods and rug binding.
pub(super) fn tube(scene: &mut Scene, points: &[Vec3], radius: f32, material: usize, sides: usize) {
    if points.len() < 2 {
        return;
    }
    let mut rings = Vec::with_capacity(points.len());
    let mut previous = crate::rng::basis((points[1] - points[0]).normalize()).0;
    for (i, &point) in points.iter().enumerate() {
        let tangent =
            (points[(i + 1).min(points.len() - 1)] - points[i.saturating_sub(1)]).normalize();
        let projected = previous - tangent * previous.dot(tangent);
        let u = if projected.length_squared() > 1e-8 {
            projected.normalize()
        } else {
            crate::rng::basis(tangent).0
        };
        let v = tangent.cross(u);
        rings.push((point, u, v));
        previous = u;
    }
    for pair in rings.windows(2) {
        for i in 0..sides {
            let angles = [
                i as f32 * TAU / sides as f32,
                (i + 1) as f32 * TAU / sides as f32,
            ];
            let na = angles.map(|a| pair[0].1 * a.cos() + pair[0].2 * a.sin());
            let nb = angles.map(|a| pair[1].1 * a.cos() + pair[1].2 * a.sin());
            let normals = [na[0], na[1], nb[1], nb[0]];
            let positions = [
                pair[0].0 + na[0] * radius,
                pair[0].0 + na[1] * radius,
                pair[1].0 + nb[1] * radius,
                pair[1].0 + nb[0] * radius,
            ];
            for tri in [[0, 1, 2], [0, 2, 3]] {
                scene.primitives.push(Primitive::triangle(
                    tri.map(|i| positions[i]),
                    Some(tri.map(|i| normals[i])),
                    [Vec2::ZERO; 3],
                    material,
                ));
            }
        }
    }
}

pub(super) fn window_surround(scene: &mut Scene, start: f32, end: f32, trim: usize, brass: usize) {
    // Stepped architrave, projecting sill, apron and deep jambs catch separate highlights.
    for (offset, width, projection) in [
        (0.01, 0.11, 0.07),
        (0.13, 0.028, 0.045),
        (0.17, 0.018, 0.025),
    ] {
        for (z0, z1) in [
            (start - offset - width, start - offset),
            (end + offset, end + offset + width),
        ] {
            scene.rounded_box(
                Vec3::new(-4.42, 0.69, z0),
                Vec3::new(-4.4 + projection, 3.26 + offset, z1),
                0.005,
                trim,
            );
        }
        scene.rounded_box(
            Vec3::new(-4.42, 3.26 + offset, start - offset - width),
            Vec3::new(
                -4.4 + projection,
                3.26 + offset + width,
                end + offset + width,
            ),
            0.005,
            trim,
        );
    }
    scene.rounded_box(
        Vec3::new(-4.63, 0.69, start - 0.17),
        Vec3::new(-4.13, 0.76, end + 0.17),
        0.012,
        trim,
    );
    scene.rounded_box(
        Vec3::new(-4.42, 0.60, start - 0.09),
        Vec3::new(-4.32, 0.69, end + 0.09),
        0.01,
        trim,
    );
    for z in [start + 0.03, end - 0.03] {
        for y in [1.0, 2.0, 2.95] {
            cylinder(scene, Vec3::new(-4.31, y, z), 0.013, 0.075, brass);
        }
    }
    let rod_start = Vec3::new(-4.04, 3.49, start - 0.47);
    let rod_end = Vec3::new(-4.04, 3.49, end + 0.47);
    tube(scene, &[rod_start, rod_end], 0.012, brass, 12);
    for point in [rod_start, rod_end] {
        scene.add_sphere(point, 0.025, brass);
        tube(scene, &[point - Vec3::X * 0.33, point], 0.008, brass, 10);
    }
}

pub(super) fn radiator(scene: &mut Scene, center_z: f32, paint: usize, brass: usize, iron: usize) {
    // Three-column cast sections joined by top/bottom headers, with visible feet and valves.
    for section in 0..13 {
        let z = center_z + (section as f32 - 6.0) * 0.083;
        for x in [-4.21, -4.13, -4.05] {
            lathe_segments(
                scene,
                Vec3::new(x, 0.0, z),
                &[
                    (0.0, 0.13),
                    (0.026, 0.13),
                    (0.033, 0.145),
                    (0.033, 0.18),
                    (0.029, 0.21),
                    (0.024, 0.26),
                    (0.022, 0.45),
                    (0.028, 0.50),
                    (0.033, 0.53),
                    (0.034, 0.575),
                    (0.029, 0.615),
                    (0.0, 0.635),
                ],
                paint,
                20,
            );
        }
        // Raised cast decoration and a parting ridge on each front column.
        for side in [-1.0, 1.0] {
            let scroll: Vec<_> = (0..30)
                .map(|i| {
                    let t = i as f32 / 29.0;
                    Vec3::new(
                        -4.024 - 0.003 * (t * TAU).cos(),
                        0.255 + t * 0.24,
                        z + side * 0.011 * (t * std::f32::consts::PI).sin(),
                    )
                })
                .collect();
            tube(scene, &scroll, 0.0022, paint, 6);
        }
        for y in [0.175, 0.555] {
            tube(
                scene,
                &[Vec3::new(-4.23, y, z), Vec3::new(-4.03, y, z)],
                0.029,
                paint,
                16,
            );
        }
        if section == 1 || section == 11 {
            for x in [-4.22, -4.04] {
                lathe_segments(
                    scene,
                    Vec3::new(x, 0.0, z),
                    &[
                        (0.0, 0.015),
                        (0.038, 0.015),
                        (0.031, 0.04),
                        (0.02, 0.19),
                        (0.0, 0.19),
                    ],
                    paint,
                    16,
                );
            }
        }
    }
    for y in [0.175, 0.555] {
        tube(
            scene,
            &[
                Vec3::new(-4.13, y, center_z - 0.57),
                Vec3::new(-4.13, y, center_z + 0.57),
            ],
            0.018,
            paint,
            16,
        );
    }
    for side in [-1.0, 1.0] {
        let z = center_z + side * 0.655;
        let mut pipe = vec![Vec3::new(-4.13, 0.018, z), Vec3::new(-4.13, 0.12, z)];
        for i in 1..=16 {
            let a = i as f32 * std::f32::consts::FRAC_PI_2 / 16.0;
            pipe.push(Vec3::new(
                -4.13,
                0.12 + 0.055 * a.sin(),
                center_z + side * (0.60 + 0.055 * a.cos()),
            ));
        }
        pipe.push(Vec3::new(-4.13, 0.175, center_z + side * 0.55));
        tube(scene, &pipe, 0.0105, brass, 16);
        // Floor escutcheon, valve body, hexagonal compression nut and spindle.
        lathe_segments(
            scene,
            Vec3::new(-4.13, 0.012, z),
            &[
                (0.011, 0.0),
                (0.035, 0.0),
                (0.032, 0.006),
                (0.014, 0.012),
                (0.011, 0.012),
                (0.011, 0.0),
            ],
            brass,
            32,
        );
        lathe_segments(
            scene,
            Vec3::new(-4.13, 0.145, z),
            &[(0.0, 0.0), (0.025, 0.0), (0.025, 0.045), (0.0, 0.045)],
            brass,
            6,
        );
        cylinder(scene, Vec3::new(-4.13, 0.19, z), 0.012, 0.062, brass);
        let wheel: Vec<_> = (0..=32)
            .map(|i| {
                let a = i as f32 * TAU / 32.0;
                Vec3::new(-4.13 + 0.035 * a.cos(), 0.254, z + 0.035 * a.sin())
            })
            .collect();
        tube(scene, &wheel, 0.006, iron, 8);
        for i in 0..4 {
            let a = i as f32 * TAU / 4.0;
            tube(
                scene,
                &[
                    Vec3::new(-4.13, 0.254, z),
                    Vec3::new(-4.13 + 0.035 * a.cos(), 0.254, z + 0.035 * a.sin()),
                ],
                0.004,
                iron,
                6,
            );
        }
        cylinder(scene, Vec3::new(-4.13, 0.251, z), 0.009, 0.009, brass);
    }
}

/// Compress yarn only beneath the imported table's two floor-level support rails.
fn rug_pile_height(x: f32, z: f32, free_height: f32) -> f32 {
    // Bounds measured from the placed model at y < 0.045 m. The tabletop spans
    // the gap between these rails but does not press on the rug underneath it.
    let distance = [(-0.299_217, -0.170_528), (1.170_528, 1.299_217)]
        .into_iter()
        .map(|(left, right)| {
            let dx = (left - x).max(x - right).max(0.0);
            let dz = (-2.196_839 - z).max(z + 1.403_161).max(0.0);
            Vec2::new(dx, dz).length()
        })
        .fold(f32::INFINITY, f32::min);
    // A loop extends up to 12 mm sideways from its root. Keep that margin
    // compressed, then ease back to the natural pile over the next 35 mm.
    let t = ((distance - 0.012) / 0.035).clamp(0.0, 1.0);
    let blend = t * t * (3.0 - 2.0 * t);
    0.003 + (free_height - 0.003) * blend
}

pub(super) fn wool_rug(scene: &mut Scene, asset_root: &Path) -> Result<()> {
    let color = scene
        .textures
        .load(&asset_root.join("textures/rug-color.jpg"))?;
    let normal = scene
        .textures
        .load_data(&asset_root.join("textures/rug-normal.jpg"))?;
    let roughness = scene
        .textures
        .load_data(&asset_root.join("textures/rug-roughness.jpg"))?;
    let mut wool = Material::matte(Vec3::splat(0.82))
        .textured(color, -0.9)
        .texture_strength(1.0);
    wool.surface[0] = 1.0;
    wool.maps = [normal as u32 + 1, roughness as u32 + 1, 0, 0];
    let wool = scene.add_material(wool);
    let edge = scene.add_material(Material::matte(Vec3::new(0.47, 0.41, 0.32)));
    let mut displacement = crate::texture::TextureAtlas::default();
    let height_map = displacement.load_data(&asset_root.join("textures/rug-height.png"))?;
    let height =
        |x: f32, z: f32| 0.022 + 0.009 * displacement.sample(height_map, Vec2::new(x, z) * 0.9).x;
    let normal = |x: f32, z: f32| {
        let step = 0.006;
        Vec3::new(
            height(x - step, z) - height(x + step, z),
            2.0 * step,
            height(x, z - step) - height(x, z + step),
        )
        .normalize()
    };
    let point = |u: f32, v: f32| {
        let x = -2.0 + 5.4 * u;
        let z = -2.5 + 3.5 * v;
        Vec3::new(x, height(x, z), z)
    };
    // A coarse backing supports individually curved yarn loops above its surface.
    const COLUMNS: usize = 270;
    const ROWS: usize = 175;
    for x in 0..COLUMNS {
        for z in 0..ROWS {
            let u = x as f32 / COLUMNS as f32;
            let v = z as f32 / ROWS as f32;
            let points = [
                point(u, v),
                point(u, v + 1.0 / ROWS as f32),
                point(u + 1.0 / COLUMNS as f32, v + 1.0 / ROWS as f32),
                point(u + 1.0 / COLUMNS as f32, v),
            ];
            for tri in [[0, 1, 2], [0, 2, 3]] {
                scene.primitives.push(Primitive::triangle(
                    tri.map(|j| points[j]),
                    Some(tri.map(|j| normal(points[j].x, points[j].z))),
                    [Vec2::ZERO; 3],
                    wool,
                ));
            }
        }
    }
    let yarn: Vec<_> = (0..8)
        .map(|i| {
            scene.add_material(Material {
                surface: [0.98, 0.0, 0.0, 1.5],
                ..Material::matte(Vec3::new(0.73, 0.67, 0.56) * (0.88 + i as f32 * 0.025))
            })
        })
        .collect();
    let mut rng = Rng::new(83, 0, 42);
    for column in 0..300 {
        for row in 0..194 {
            let x = -1.994 + (column as f32 + rng.uniform() * 0.7) * 0.01795;
            let z = -2.494 + (row as f32 + rng.uniform() * 0.7) * 0.01795;
            let pile = rug_pile_height(x, z, 0.022 + rng.uniform() * 0.028);
            let angle = rng.uniform() * TAU;
            let direction = Vec3::new(angle.cos(), 0.0, angle.sin());
            let curl = Vec3::new(-direction.z, 0.0, direction.x);
            let root = Vec3::new(x, height(x, z) - 0.003, z);
            let points: Vec<_> = (0..=4)
                .map(|i| {
                    let t = i as f32 / 4.0;
                    root + direction * ((t - 0.5) * 0.009)
                        + Vec3::Y * (pile * (t * std::f32::consts::PI).sin())
                        + curl * (0.005 * (t * TAU).sin())
                })
                .collect();
            let material = yarn[(rng.next_u32() as usize) % yarn.len()];
            tube(scene, &points, 0.0014 + rng.uniform() * 0.0007, material, 4);
        }
    }
    scene.add_box(
        Vec3::new(-2.0, 0.012, -2.5),
        Vec3::new(3.4, 0.023, 1.0),
        edge,
    );
    tube(
        scene,
        &[
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 1.0),
            point(0.0, 1.0),
            point(0.0, 0.0),
        ],
        0.009,
        wool,
        12,
    );
    // Loose end tassels continue the yarn silhouette beyond the woven backing.
    for side in [-1.0, 1.0] {
        for i in 0..270 {
            let x = -1.98 + i as f32 * 0.0199;
            let points: Vec<_> = (0..5)
                .map(|j| {
                    let t = j as f32 / 4.0;
                    Vec3::new(
                        x + 0.003 * (t * 12.0 + i as f32).sin(),
                        0.025 - 0.009 * t,
                        (if side < 0.0 { -2.5 } else { 1.0 }) + side * 0.055 * t,
                    )
                })
                .collect();
            tube(scene, &points, 0.0018, wool, 6);
        }
    }
    Ok(())
}

pub(super) fn imported_furniture(scene: &mut Scene, asset_root: &Path) -> Result<()> {
    for (name, position, scale, rotation) in [
        (
            "modern_arm_chair_01",
            Vec3::new(-2.45, 0.012, -1.3),
            1.0,
            32.0,
        ),
        (
            "modern_coffee_table_01",
            Vec3::new(0.5, 0.036, -1.8),
            1.33,
            90.0,
        ),
        (
            "modern_wooden_cabinet",
            Vec3::new(4.05, 0.012, -1.5),
            1.0,
            -90.0,
        ),
        ("tea_set_01", Vec3::new(0.15, 0.554, -1.92), 0.72, -15.0),
        ("glam_velvet_sofa", Vec3::new(1.05, 0.012, -3.42), 1.25, 0.0),
    ] {
        scene.import_obj(
            &asset_root
                .join("models")
                .join(name)
                .join(format!("{name}.obj")),
            position,
            scale,
            rotation,
            None,
        )?;
    }
    Ok(())
}

pub(super) fn apartment_decor(
    scene: &mut Scene,
    root: &Path,
    brass: usize,
    night: bool,
) -> Result<()> {
    // A pleated table lamp sits on the sideboard, alongside the ceramic vessel.
    lathe_segments(
        scene,
        Vec3::new(4.03, 0.694, -0.70),
        &[
            (0.0, 0.0),
            (0.13, 0.0),
            (0.14, 0.03),
            (0.10, 0.05),
            (0.035, 0.25),
            (0.0, 0.25),
        ],
        brass,
        48,
    );
    let shade = scene.add_material(Material::matte(Vec3::new(0.80, 0.73, 0.60)).translucent(0.62));
    let center = Vec3::new(4.03, 0.694, -0.70);
    let bulb = scene.add_material(if night {
        Material::light(Vec3::new(180.0, 110.0, 52.0))
    } else {
        Material::matte(Vec3::new(0.95, 0.93, 0.86))
    });
    cylinder(scene, center + Vec3::Y * 0.24, 0.015, 0.065, brass);
    scene.add_sphere(center + Vec3::Y * 0.365, 0.028, bulb);
    for i in 0..128 {
        let point = |i: usize, y: f32| {
            let a = i as f32 * TAU / 128.0;
            let radius = if y < 0.3 { 0.235 } else { 0.14 };
            let radius = radius + if i.is_multiple_of(2) { 0.007 } else { 0.0 };
            center + Vec3::new(radius * a.cos(), y, radius * a.sin())
        };
        let p = [
            point(i, 0.23),
            point(i, 0.54),
            point(i + 1, 0.54),
            point(i + 1, 0.23),
        ];
        for tri in [[0, 1, 2], [0, 2, 3]] {
            scene.primitives.push(Primitive::triangle(
                tri.map(|j| p[j]),
                None,
                [Vec2::ZERO; 3],
                shade,
            ));
        }
    }
    scene.import_obj(
        &root.join("models/potted_plant_02/potted_plant_02.obj"),
        Vec3::new(-2.80, 0.012, -3.60),
        1.55,
        20.0,
        None,
    )?;
    Ok(())
}
