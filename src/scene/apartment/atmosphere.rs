//! Day/night lighting and close-range textile and architectural details.
use super::*;
use details::tube;

pub(super) fn exterior(scene: &mut Scene, night: bool) {
    // The sky card stays above the view through the windows; the street has actual depth.
    let sky_light = scene.add_material(Material::light(if night {
        Vec3::new(0.05, 0.09, 0.22)
    } else {
        Vec3::new(18.0, 21.0, 26.0)
    }));
    scene.add_quad(
        Vec3::new(-8.0, 4.5, -6.0),
        Vec3::Z * 16.0,
        Vec3::Y * 8.0,
        sky_light,
    );
    if !night {
        let sun = scene.add_material(Material::light(Vec3::new(140.0, 112.0, 77.0)));
        scene.add_quad(Vec3::new(-8.0, 5.0, 3.0), Vec3::Y * 0.7, Vec3::Z * 0.9, sun);
    }
    let stone = scene.add_material(Material::matte(Vec3::new(0.51, 0.49, 0.43)).patterned(1, 3.0));
    let trim = scene.add_material(Material::matte(Vec3::new(0.65, 0.63, 0.57)));
    let slate = scene.add_material(Material::matte(Vec3::new(0.10, 0.14, 0.18)));
    scene.add_box(
        Vec3::new(-16.0, -8.0, -24.0),
        Vec3::new(-15.5, 8.0, 30.0),
        stone,
    );
    for level in -2..=1 {
        let y = level as f32 * 3.1;
        scene.add_box(
            Vec3::new(-15.5, y - 0.15, -24.0),
            Vec3::new(-15.3, y, 30.0),
            trim,
        );
        for bay in -6..=7 {
            let z = bay as f32 * 3.6;
            let glazing = if night && (bay + level) % 3 == 0 {
                scene.add_material(Material::light(Vec3::new(0.7, 0.36, 0.13)))
            } else {
                slate
            };
            scene.add_box(
                Vec3::new(-15.48, y + 0.35, z),
                Vec3::new(-15.40, y + 2.55, z + 1.25),
                trim,
            );
            scene.add_quad(
                Vec3::new(-15.39, y + 0.43, z + 0.08),
                Vec3::Y * 2.04,
                Vec3::Z * 1.09,
                glazing,
            );
            scene.add_box(
                Vec3::new(-15.38, y + 0.43, z + 0.60),
                Vec3::new(-15.33, y + 2.47, z + 0.65),
                trim,
            );
        }
    }
}

pub(super) fn balcony(scene: &mut Scene, start: f32, end: f32, iron: usize) {
    let x = -4.91;
    for y in [0.37, 1.22] {
        scene.rounded_box(
            Vec3::new(x - 0.025, y - 0.018, start - 0.04),
            Vec3::new(x + 0.025, y + 0.018, end + 0.04),
            0.008,
            iron,
        );
    }
    for i in 0..=10 {
        let z = start + (end - start) * i as f32 / 10.0;
        tube(
            scene,
            &[Vec3::new(x, 0.36, z), Vec3::new(x, 1.23, z)],
            0.009,
            iron,
            8,
        );
        for y in [0.51, 1.08] {
            scene.add_sphere(Vec3::new(x, y, z), 0.016, iron);
        }
        if i < 10 {
            // Paired scrolls have rolled ends, relief at intersections and slight forging variation.
            let center = z + (end - start) / 20.0;
            for side in [-1.0, 1.0] {
                let points: Vec<_> = (0..=40)
                    .map(|step| {
                        let t = step as f32 / 40.0;
                        let angle = t * TAU * 1.25;
                        let radius = 0.075 * (1.0 - 0.75 * t);
                        Vec3::new(
                            x + 0.006 * angle.sin(),
                            0.79 + side * radius * 2.4 * angle.sin(),
                            center + side * radius * angle.cos(),
                        )
                    })
                    .collect();
                tube(scene, &points, 0.0055, iron, 6);
            }
        }
    }
    for z in [start, end] {
        for y in [0.38, 1.2] {
            scene.rounded_box(
                Vec3::new(-4.67, y - 0.055, z - 0.035),
                Vec3::new(-4.63, y + 0.055, z + 0.035),
                0.005,
                iron,
            );
            tube(
                scene,
                &[Vec3::new(-4.65, y, z), Vec3::new(x, y, z)],
                0.014,
                iron,
                10,
            );
            for dy in [-0.035, 0.035] {
                scene.add_sphere(Vec3::new(-4.62, y + dy, z), 0.009, iron);
            }
        }
    }
}

pub(super) fn curtain(scene: &mut Scene, start: f32, width: f32, material: usize) {
    let phase = start * 7.13;
    let point = |u: f32, v: f32| {
        let fold = u * TAU * 4.5 + 0.42 * (v * 5.0 + phase).sin() * v;
        let amplitude = 0.046 + 0.023 * v + 0.008 * (u * 13.0 + phase).sin();
        let pooling = v.powi(12);
        Vec3::new(
            -4.08
                + amplitude * fold.sin()
                + v * 0.045 * (v * 4.2 + phase).sin()
                + pooling * (0.06 + 0.04 * (u * 7.0 + phase).sin()),
            3.47 * (1.0 - v) + v * (0.024 + 0.012 * (u * 9.0 + phase).sin().powi(2))
                - 0.012 * (u * TAU * 4.5).sin().powi(2) * (1.0 - v),
            start + width * u + 0.018 * v * (u * 5.0 + v * 4.0 + phase).sin(),
        )
    };
    let normal = |u: f32, v: f32| {
        (point(u + 0.001, v) - point(u - 0.001, v))
            .cross(point(u, v + 0.001) - point(u, v - 0.001))
            .normalize()
    };
    const COLUMNS: usize = 72;
    const ROWS: usize = 96;
    for column in 0..COLUMNS {
        for row in 0..ROWS {
            let u = column as f32 / COLUMNS as f32;
            let v = row as f32 / ROWS as f32;
            let uv = [
                Vec2::new(u, v),
                Vec2::new(u + 1.0 / COLUMNS as f32, v),
                Vec2::new(u + 1.0 / COLUMNS as f32, v + 1.0 / ROWS as f32),
                Vec2::new(u, v + 1.0 / ROWS as f32),
            ];
            for tri in [[0, 1, 2], [0, 2, 3]] {
                scene.primitives.push(Primitive::triangle(
                    tri.map(|i| point(uv[i].x, uv[i].y)),
                    Some(tri.map(|i| normal(uv[i].x, uv[i].y))),
                    tri.map(|i| Vec2::new(uv[i].x * 0.7, (1.0 - uv[i].y) * 3.47)),
                    material,
                ));
            }
        }
    }
    for u in [0.0, 1.0] {
        let seam: Vec<_> = (0..=96).map(|i| point(u, i as f32 / 96.0)).collect();
        tube(scene, &seam, 0.002, material, 5);
    }
    let hem: Vec<_> = (0..=72).map(|i| point(i as f32 / 72.0, 1.0)).collect();
    tube(scene, &hem, 0.003, material, 6);
}

pub(super) fn lamps(scene: &mut Scene, root: &Path, night: bool) -> Result<()> {
    let first = scene.materials.len();
    scene.import_obj(
        &root.join("models/rattan_pendant/rattan_pendant.obj"),
        Vec3::new(0.2, 2.174, -1.35),
        1.8,
        0.0,
        None,
    )?;
    // Retain the source silhouette and fittings, replacing its flat masked shell
    // with actual interlaced cane so nearby strands have thickness and cast shadows.
    scene
        .primitives
        .retain(|primitive| primitive.info[1] as usize != first + 4);
    rattan_weave(scene, Vec3::new(0.2, 2.174, -1.35), 1.8);
    // Preserve the full 80 mm source bulb. Its thin glass envelope surrounds a
    // separate compact emitting element, which projects the cane's real shadows.
    const BULB_SCALE: f32 = 0.55;
    let bulb_anchor = Vec3::new(0.2, 2.174 + 0.2214641 * 1.8, -1.35);
    for primitive in &mut scene.primitives {
        if primitive.info[1] as usize == first + 3 {
            primitive.p0 = pack_vec3(
                bulb_anchor + (unpack_vec3(primitive.p0) - bulb_anchor) * BULB_SCALE,
                0.0,
            );
            primitive.p1 = pack_vec3(unpack_vec3(primitive.p1) * BULB_SCALE, 0.0);
            primitive.p2 = pack_vec3(unpack_vec3(primitive.p2) * BULB_SCALE, 0.0);
        }
    }
    scene.materials[first + 3] = Material::thin_glass(1.52);
    let emitter_radius: f32 = 0.0054;
    let emitter = scene.add_material(if night {
        Material::light(Vec3::new(360.0, 235.0, 118.0) * (0.0396 / emitter_radius).powi(2))
    } else {
        Material::matte(Vec3::new(0.93, 0.86, 0.65))
    });
    scene.add_sphere(bulb_anchor - Vec3::Y * 0.0396, emitter_radius, emitter);
    let first = scene.materials.len();
    scene.import_obj(
        &root.join("models/arc_floor_lamp/arc_floor_lamp.obj"),
        Vec3::new(3.1, 0.012, -3.05),
        1.5,
        180.0,
        None,
    )?;
    if night {
        scene.materials[first] = Material::light(Vec3::new(350.0, 240.0, 145.0));
    }
    let linen = scene.add_material(scene.materials[first + 1].translucent(0.45));
    // The source uses one texture atlas for marble, metal and cloth. Restrict diffuse
    // transmission to its tall cylindrical shade faces, preserving the other parts.
    for primitive in &mut scene.primitives {
        if primitive.info[1] as usize != first + 1 {
            continue;
        }
        let (lo, hi) = primitive.bounds();
        let center = (lo + hi) * 0.5;
        let radius = Vec2::new(center.x - 2.423, center.z + 3.503).length();
        if lo.y > 1.54 && hi.y < 1.92 && hi.y - lo.y > 0.1 && (0.28..0.31).contains(&radius) {
            primitive.info[1] = linen as u32;
        }
    }
    power_cords(scene);
    Ok(())
}

fn power_cords(scene: &mut Scene) {
    let cable = scene.add_material(Material::matte(Vec3::splat(0.018)));
    let ivory = scene.add_material(Material::matte(Vec3::new(0.80, 0.78, 0.72)));
    // A double wall outlet supplies both lamps. Its plugs sit against the socket faces.
    scene.rounded_box(
        Vec3::new(4.461, 0.26, -3.145),
        Vec3::new(4.501, 0.38, -2.875),
        0.008,
        ivory,
    );
    for z in [-3.08, -2.94] {
        scene.rounded_box(
            Vec3::new(4.450, 0.277, z - 0.042),
            Vec3::new(4.464, 0.363, z + 0.042),
            0.012,
            ivory,
        );
        scene.rounded_box(
            Vec3::new(4.413, 0.296, z - 0.023),
            Vec3::new(4.456, 0.344, z + 0.023),
            0.012,
            cable,
        );
    }
    // Join the measured end of the source arc-lamp cord, then route under the cabinet.
    smooth_cord(
        scene,
        &[
            Vec3::new(3.718, 0.017, -2.296),
            Vec3::new(3.72, 0.017, -2.24),
            Vec3::new(3.82, 0.017, -2.20),
            Vec3::new(4.22, 0.017, -2.36),
            Vec3::new(4.44, 0.017, -2.67),
            Vec3::new(4.445, 0.03, -3.00),
            Vec3::new(4.43, 0.14, -3.08),
            Vec3::new(4.414, 0.32, -3.08),
        ],
        cable,
    );
    // The sideboard lamp's lead runs across the rear edge and down to the second plug.
    smooth_cord(
        scene,
        &[
            Vec3::new(4.14, 0.711, -0.70),
            Vec3::new(4.29, 0.711, -0.76),
            Vec3::new(4.435, 0.711, -1.05),
            Vec3::new(4.44, 0.711, -2.48),
            Vec3::new(4.44, 0.66, -2.85),
            Vec3::new(4.42, 0.45, -2.94),
            Vec3::new(4.414, 0.32, -2.94),
        ],
        cable,
    );
}

fn smooth_cord(scene: &mut Scene, controls: &[Vec3], material: usize) {
    let mut points = Vec::new();
    for i in 0..controls.len() - 1 {
        let a = controls[i.saturating_sub(1)];
        let b = controls[i];
        let c = controls[i + 1];
        let d = controls[(i + 2).min(controls.len() - 1)];
        for step in 0..12 {
            let t = step as f32 / 12.0;
            let mut p = (2.0 * b
                + (-a + c) * t
                + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                + (-a + 3.0 * b - 3.0 * c + d) * t * t * t)
                * 0.5;
            p.y = p.y.max(0.017);
            points.push(p);
        }
    }
    points.push(*controls.last().unwrap());
    tube(scene, &points, 0.0045, material, 8);
}

fn rattan_weave(scene: &mut Scene, origin: Vec3, scale: f32) {
    // Radial profile sampled from the attributed Vinish source shade, in source meters.
    const PROFILE: [(f32, f32); 23] = [
        (0.0, 0.1302),
        (0.00346, 0.1302),
        (0.00693, 0.1302),
        (0.01846, 0.14223),
        (0.03346, 0.15989),
        (0.04846, 0.17498),
        (0.06346, 0.18745),
        (0.07846, 0.19726),
        (0.09346, 0.20432),
        (0.10846, 0.20858),
        (0.12346, 0.21),
        (0.13846, 0.20858),
        (0.15346, 0.20432),
        (0.16846, 0.19726),
        (0.18346, 0.18745),
        (0.19846, 0.17498),
        (0.21346, 0.15989),
        (0.22846, 0.14223),
        (0.24346, 0.12193),
        (0.25846, 0.09865),
        (0.27346, 0.07089),
        (0.28846, 0.05),
        (0.30693, 0.05),
    ];
    let radius = |height: f32| {
        let index = PROFILE
            .partition_point(|&(y, _)| y < height)
            .clamp(1, PROFILE.len() - 1);
        let (ya, ra) = PROFILE[index - 1];
        let (yb, rb) = PROFILE[index];
        ra + (rb - ra) * ((height - ya) / (yb - ya)).clamp(0.0, 1.0)
    };
    let materials: Vec<_> = (0..7)
        .map(|i| {
            scene.add_material(Material {
                surface: [0.72, 0.0, 0.0, 1.5],
                ..Material::matte(Vec3::new(0.43, 0.28, 0.13) * (0.87 + i as f32 * 0.035))
                    .patterned(2, 90.0)
            })
        })
        .collect();
    const RIBS: usize = 40;
    const ROWS: usize = 22;
    for rib in 0..RIBS {
        let angle = rib as f32 * TAU / RIBS as f32;
        let points: Vec<_> = (0..=ROWS * 4)
            .map(|i| {
                let v = i as f32 / (ROWS * 4) as f32;
                let y = v * 0.30693;
                let r = radius(y)
                    + 0.0032
                        * (v * ROWS as f32 * std::f32::consts::PI
                            + rib as f32 * std::f32::consts::PI)
                            .cos();
                origin + Vec3::new(r * angle.cos(), y, r * angle.sin()) * scale
            })
            .collect();
        tube(
            scene,
            &points,
            0.0030 * scale,
            materials[rib % materials.len()],
            5,
        );
    }
    for row in 0..=ROWS {
        let y = row as f32 / ROWS as f32 * 0.30693;
        let points: Vec<_> = (0..=RIBS * 4)
            .map(|i| {
                let angle = i as f32 * TAU / (RIBS * 4) as f32;
                let r = radius(y)
                    - 0.0032
                        * (angle * RIBS as f32 * 0.5 + row as f32 * std::f32::consts::PI).cos();
                origin + Vec3::new(r * angle.cos(), y, r * angle.sin()) * scale
            })
            .collect();
        tube(
            scene,
            &points,
            0.0026 * scale,
            materials[(row * 3) % materials.len()],
            5,
        );
    }
}

pub(super) fn canvas(scene: &mut Scene, root: &Path, frame: usize) -> Result<()> {
    // Use the same bounds as the large wall panel, centered in both directions.
    let center = Vec3::new((-1.3 + 4.12) * 0.5, (1.4 + 3.29) * 0.5, -4.37);
    let height = 1.38;
    let width = height * 1920.0 / 1848.0;
    let canvas_texture = scene
        .textures
        .load(&root.join("textures/monet-water-lilies.jpg"))?;
    let linen = scene.add_material(Material::matte(Vec3::new(0.68, 0.61, 0.49)));
    let paint = scene.add_material(Material {
        surface: [0.78, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::splat(0.88)).textured(canvas_texture, 1.0)
    });
    let half = Vec3::new(width * 0.5, height * 0.5, 0.0);
    scene.add_box(center - half - Vec3::Z * 0.038, center + half, linen);
    // Fine, shallow weave changes the real surface normals without distorting the reproduction.
    let point = |u: f32, v: f32| {
        center - half
            + Vec3::new(
                width * u,
                height * v,
                0.001 + 0.0002 * (u * 700.0).sin() * (v * 670.0).sin(),
            )
    };
    for x in 0..128 {
        for y in 0..128 {
            let u = x as f32 / 128.0;
            let v = y as f32 / 128.0;
            let uv = [
                Vec2::new(u, v),
                Vec2::new(u + 1.0 / 128.0, v),
                Vec2::new(u + 1.0 / 128.0, v + 1.0 / 128.0),
                Vec2::new(u, v + 1.0 / 128.0),
            ];
            for tri in [[0, 1, 2], [0, 2, 3]] {
                scene.primitives.push(Primitive::triangle(
                    tri.map(|i| point(uv[i].x, uv[i].y)),
                    None,
                    tri.map(|i| uv[i]),
                    paint,
                ));
            }
        }
    }
    let gap = 0.012;
    let border = 0.025;
    for side in [-1.0, 1.0] {
        let x = center.x + side * (width * 0.5 + gap + border * 0.5);
        scene.rounded_box(
            Vec3::new(
                x - border * 0.5,
                center.y - height * 0.5 - gap - border,
                center.z - 0.05,
            ),
            Vec3::new(
                x + border * 0.5,
                center.y + height * 0.5 + gap + border,
                center.z + 0.014,
            ),
            0.003,
            frame,
        );
        let y = center.y + side * (height * 0.5 + gap + border * 0.5);
        scene.rounded_box(
            Vec3::new(
                center.x - width * 0.5 - gap,
                y - border * 0.5,
                center.z - 0.05,
            ),
            Vec3::new(
                center.x + width * 0.5 + gap,
                y + border * 0.5,
                center.z + 0.014,
            ),
            0.003,
            frame,
        );
    }
    Ok(())
}
