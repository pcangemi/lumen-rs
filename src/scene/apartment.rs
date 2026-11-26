//! Detailed apartment interior with cut parquet boards, joinery, mouldings and soft furnishings.
use super::*;
use crate::geometry::{pack_vec3, unpack_vec3};
use crate::rng::Rng;
use std::f32::consts::TAU;

pub fn build(aspect: f32) -> Result<Scene> {
    let from = Vec3::new(3.45, 1.75, 6.1);
    let target = Vec3::new(-0.5, 1.35, -1.9);
    let mut scene = Scene::empty(Camera::new(
        from,
        target,
        54.0,
        aspect,
        0.0,
        from.distance(target),
    ));
    scene.sky = Vec3::new(0.48, 0.62, 0.85);
    scene.horizon = Vec3::new(0.75, 0.74, 0.69);
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let oak_texture = scene.textures.load(&asset_root.join("textures/oak.jpg"))?;
    let plaster_texture = scene
        .textures
        .load(&asset_root.join("textures/plaster.jpg"))?;
    let fabric_texture = scene
        .textures
        .load(&asset_root.join("textures/fabric.jpg"))?;
    let linen_texture = scene
        .textures
        .load(&asset_root.join("textures/linen.jpg"))?;
    let plaster = scene.add_material(
        Material::matte(Vec3::new(0.97, 0.94, 0.88))
            .textured(plaster_texture, -0.45)
            .texture_strength(0.12),
    );
    let trim = scene.add_material(Material {
        surface: [0.36, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.83, 0.82, 0.76))
    });
    let dark = scene.add_material(Material::metal(Vec3::splat(0.035), 0.35));
    let brass = scene.add_material(Material::metal(Vec3::new(0.74, 0.49, 0.22), 0.3));
    let walnut = scene.add_material(Material {
        surface: [0.32, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.46, 0.28, 0.15)).textured(oak_texture, -0.7)
    });
    let fabric = scene.add_material(
        Material::matte(Vec3::new(0.92, 0.85, 0.70))
            .textured(fabric_texture, -3.0)
            .texture_strength(0.25),
    );
    let cushion = scene.add_material(
        Material::matte(Vec3::new(0.39, 0.47, 0.35))
            .textured(fabric_texture, -3.0)
            .texture_strength(0.25),
    );
    let curtain = scene.add_material(
        Material::matte(Vec3::new(0.89, 0.87, 0.80))
            .textured(linen_texture, -2.0)
            .texture_strength(0.15),
    );
    let glass = scene.add_material(Material::glass(1.52, Vec3::new(0.008, 0.003, 0.005)));

    parquet(&mut scene, oak_texture);
    scene.add_box(
        Vec3::new(-4.6, -0.12, -4.7),
        Vec3::new(4.6, -0.01, 9.0),
        walnut,
    );
    scene.add_box(
        Vec3::new(-4.6, 0.0, -4.7),
        Vec3::new(4.6, 3.8, -4.5),
        plaster,
    );
    scene.add_box(Vec3::new(4.5, 0.0, -4.5), Vec3::new(4.7, 3.8, 9.0), plaster);
    scene.add_box(
        Vec3::new(-4.6, 3.8, -4.7),
        Vec3::new(4.7, 4.0, 9.0),
        plaster,
    );
    // The left wall is assembled around three actual openings.
    scene.add_box(
        Vec3::new(-4.65, 0.0, -4.5),
        Vec3::new(-4.4, 0.48, 9.0),
        plaster,
    );
    scene.add_box(
        Vec3::new(-4.65, 3.22, -4.5),
        Vec3::new(-4.4, 3.8, 9.0),
        plaster,
    );
    let windows = [(-3.7, -1.9), (-0.8, 1.0), (2.1, 3.9)];
    let mut previous = -4.5;
    for (start, end) in windows {
        scene.add_box(
            Vec3::new(-4.65, 0.48, previous),
            Vec3::new(-4.4, 3.22, start),
            plaster,
        );
        window(&mut scene, start, end, trim, brass, dark, glass);
        folded_curtain(&mut scene, start - 0.06, 0.40, curtain);
        folded_curtain(&mut scene, end - 0.34, 0.40, curtain);
        previous = end;
    }
    scene.add_box(
        Vec3::new(-4.65, 0.48, previous),
        Vec3::new(-4.4, 3.22, 9.0),
        plaster,
    );
    let daylight = scene.add_material(Material::light(Vec3::new(9.0, 10.0, 12.0)));
    scene.add_quad(
        Vec3::new(-6.4, 0.1, -5.5),
        Vec3::Z * 12.0,
        Vec3::Y * 6.0,
        daylight,
    );
    let sunlight = scene.add_material(Material::light(Vec3::new(13.0, 10.0, 6.7)));
    scene.add_quad(
        Vec3::new(-7.0, 4.5, 2.5),
        Vec3::Y * 1.3,
        Vec3::Z * 2.0,
        sunlight,
    );

    mouldings(&mut scene, trim);
    let rug = scene.add_material(
        Material::matte(Vec3::new(0.72, 0.70, 0.61))
            .textured(fabric_texture, -4.0)
            .texture_strength(0.2),
    );
    scene.rounded_box(
        Vec3::new(-2.0, 0.012, -2.2),
        Vec3::new(3.7, 0.035, 2.15),
        0.018,
        rug,
    );

    // Upholstered sofa: separate cushions, softened edges, dark reveal and slender feet.
    scene.rounded_box(
        Vec3::new(-0.8, 0.20, -3.63),
        Vec3::new(3.35, 0.43, -2.08),
        0.08,
        walnut,
    );
    scene.rounded_box(
        Vec3::new(-0.85, 0.43, -3.68),
        Vec3::new(3.4, 1.45, -3.24),
        0.17,
        fabric,
    );
    for x in [-0.67, 0.65, 1.97] {
        scene.rounded_box(
            Vec3::new(x, 0.43, -3.2),
            Vec3::new(x + 1.25, 0.74, -2.11),
            0.13,
            fabric,
        );
        // A raised seam below the cushion gives a fine, geometric shadow line.
        scene.rounded_box(
            Vec3::new(x + 0.02, 0.47, -2.12),
            Vec3::new(x + 1.23, 0.49, -2.10),
            0.008,
            fabric,
        );
    }
    for x in [-1.03, 3.2] {
        scene.rounded_box(
            Vec3::new(x, 0.36, -3.65),
            Vec3::new(x + 0.38, 1.03, -2.05),
            0.16,
            fabric,
        );
    }
    for x in [-0.65, 3.0] {
        for z in [-3.4, -2.25] {
            cylinder(&mut scene, Vec3::new(x, 0.025, z), 0.035, 0.21, brass);
        }
    }
    scene.rounded_box(
        Vec3::new(-0.48, 0.75, -3.21),
        Vec3::new(0.24, 1.32, -2.94),
        0.13,
        cushion,
    );
    scene.rounded_box(
        Vec3::new(2.23, 0.75, -3.21),
        Vec3::new(2.95, 1.32, -2.94),
        0.13,
        cushion,
    );

    // Low circular table, layered books, and original ceramic vessel.
    cylinder(&mut scene, Vec3::new(0.5, 0.46, -0.5), 1.12, 0.055, walnut);
    for x in [-0.13, 1.13] {
        cylinder(&mut scene, Vec3::new(x, 0.035, -0.5), 0.13, 0.43, walnut);
    }
    let paper = scene.add_material(Material::matte(Vec3::new(0.76, 0.73, 0.64)));
    let book = scene.add_material(Material::matte(Vec3::new(0.12, 0.18, 0.17)));
    scene.add_box(
        Vec3::new(0.35, 0.517, -0.8),
        Vec3::new(0.95, 0.535, -0.35),
        book,
    );
    scene.add_box(
        Vec3::new(0.36, 0.535, -0.79),
        Vec3::new(0.94, 0.58, -0.36),
        paper,
    );
    scene.add_box(
        Vec3::new(0.35, 0.58, -0.8),
        Vec3::new(0.95, 0.597, -0.35),
        book,
    );
    let ceramic = scene.add_material(Material {
        surface: [0.21, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.22, 0.27, 0.18))
    });
    scene.import_obj(
        &asset_root.join("vessel.obj"),
        Vec3::new(-0.05, 0.515, -0.55),
        0.38,
        0.0,
        Some(ceramic),
    )?;

    // A framed abstract canvas sits within the upper wall moulding.
    scene.add_box(
        Vec3::new(0.0, 1.65, -4.46),
        Vec3::new(2.45, 3.15, -4.36),
        walnut,
    );
    scene.add_quad(
        Vec3::new(0.055, 1.705, -4.345),
        Vec3::X * 2.34,
        Vec3::Y * 1.39,
        paper,
    );
    let ink = scene.add_material(Material::matte(Vec3::new(0.15, 0.19, 0.15)));
    let clay = scene.add_material(Material::matte(Vec3::new(0.40, 0.23, 0.13)));
    ellipse(&mut scene, Vec3::new(0.9, 2.3, -4.34), 0.57, 0.48, ink);
    ellipse(&mut scene, Vec3::new(1.68, 2.58, -4.33), 0.4, 0.3, clay);

    pendant(&mut scene, Vec3::new(0.4, 0.0, -0.7), trim, brass, dark);
    Ok(scene)
}

fn parquet(scene: &mut Scene, texture: usize) {
    let mut rng = Rng::new(14, 0, 42);
    let materials: Vec<usize> = (0..48)
        .map(|_| {
            let tint = 0.72 + rng.uniform() * 0.24;
            scene.add_material(Material {
                surface: [0.34 + rng.uniform() * 0.12, 0.0, 0.0, 1.5],
                ..Material::matte(Vec3::new(tint, tint * 0.90, tint * 0.77)).textured(texture, 1.0)
            })
        })
        .collect();
    // Chevron boards are parallelograms with 45-degree cut ends, not rectangular herringbone.
    let half_span = 0.48;
    let course = 0.125;
    for column in -5..=5 {
        for row in -42..74 {
            for side in [-1.0, 1.0] {
                let x = column as f32 * half_span * 2.0;
                let z = row as f32 * course;
                let mut corners = [
                    Vec3::new(x, 0.008, z),
                    Vec3::new(x + side * half_span, 0.008, z - half_span),
                    Vec3::new(x + side * half_span, 0.008, z - half_span + course),
                    Vec3::new(x, 0.008, z + course),
                ];
                if side > 0.0 {
                    corners.reverse();
                }
                let center = corners.iter().copied().sum::<Vec3>() * 0.25;
                let outer = corners.map(|p| center + (p - center) * 0.991);
                let inner = corners.map(|p| center + (p - center) * 0.980 + Vec3::Y * 0.002);
                let direction = Vec3::new(side, 0.0, -1.0).normalize();
                let cross = Vec3::new(1.0, 0.0, side).normalize();
                let shift = Vec2::new(rng.uniform(), rng.uniform());
                let uv = |p: Vec3| {
                    Vec2::new(
                        (p - center).dot(cross) * 0.55,
                        (p - center).dot(direction) * 0.55,
                    ) + shift
                };
                let material = materials[(rng.next_u32() as usize) % materials.len()];
                for indices in [[0, 1, 2], [0, 2, 3]] {
                    scene.primitives.push(Primitive::triangle(
                        indices.map(|i| inner[i]),
                        Some([Vec3::Y; 3]),
                        indices.map(|i| uv(inner[i])),
                        material,
                    ));
                }
                for i in 0..4 {
                    let j = (i + 1) % 4;
                    for points in [
                        [outer[i], outer[j], inner[j]],
                        [outer[i], inner[j], inner[i]],
                    ] {
                        scene.primitives.push(Primitive::triangle(
                            points,
                            None,
                            points.map(uv),
                            material,
                        ));
                    }
                }
            }
        }
    }
}

fn mouldings(scene: &mut Scene, material: usize) {
    // Layered cornice and skirting profiles run along the room's actual boundaries.
    for (height, thickness, projection) in [
        (0.025, 0.13, 0.045),
        (0.155, 0.025, 0.065),
        (0.18, 0.018, 0.04),
        (1.12, 0.035, 0.045),
        (1.16, 0.018, 0.065),
        (3.53, 0.035, 0.04),
        (3.57, 0.055, 0.075),
        (3.625, 0.055, 0.11),
        (3.68, 0.08, 0.16),
        (3.76, 0.04, 0.21),
    ] {
        scene.add_box(
            Vec3::new(-4.4, height, -4.5),
            Vec3::new(4.5, height + thickness, -4.5 + projection),
            material,
        );
        scene.add_box(
            Vec3::new(4.5 - projection, height, -4.5),
            Vec3::new(4.5, height + thickness, 9.0),
            material,
        );
        if height > 3.3 || height < 0.3 {
            scene.add_box(
                Vec3::new(-4.4, height, -4.5),
                Vec3::new(-4.4 + projection, height + thickness, 9.0),
                material,
            );
        }
    }
    for (left, right) in [(-4.05, -1.65), (-1.3, 1.1), (1.45, 4.12)] {
        panel(scene, left, right, 0.32, 0.94, material);
    }
    panel(scene, -4.05, -1.65, 1.4, 3.29, material);
    panel(scene, -1.3, 4.12, 1.4, 3.29, material);
}
fn panel(scene: &mut Scene, left: f32, right: f32, bottom: f32, top: f32, material: usize) {
    for (inset, width, depth) in [(0.0, 0.035, 0.026), (0.044, 0.014, 0.018)] {
        let (l, r, b, t) = (left + inset, right - inset, bottom + inset, top - inset);
        for (min, max) in [
            (
                Vec3::new(l, b, -4.49),
                Vec3::new(r, b + width, -4.49 + depth),
            ),
            (
                Vec3::new(l, t - width, -4.49),
                Vec3::new(r, t, -4.49 + depth),
            ),
            (
                Vec3::new(l, b, -4.49),
                Vec3::new(l + width, t, -4.49 + depth),
            ),
            (
                Vec3::new(r - width, b, -4.49),
                Vec3::new(r, t, -4.49 + depth),
            ),
        ] {
            scene.add_box(min, max, material);
        }
    }
}

#[allow(
    clippy::approx_constant,
    reason = "Window dimensions are measured in meters, not angles"
)]
fn window(
    scene: &mut Scene,
    start: f32,
    end: f32,
    trim: usize,
    brass: usize,
    iron: usize,
    glass: usize,
) {
    let x = -4.38;
    for (min, max) in [
        (
            Vec3::new(x - 0.2, 0.47, start - 0.06),
            Vec3::new(x + 0.2, 0.55, end + 0.06),
        ),
        (
            Vec3::new(x - 0.18, 3.14, start - 0.07),
            Vec3::new(x + 0.05, 3.25, end + 0.07),
        ),
        (
            Vec3::new(x - 0.18, 0.55, start - 0.07),
            Vec3::new(x + 0.05, 3.14, start + 0.04),
        ),
        (
            Vec3::new(x - 0.18, 0.55, end - 0.04),
            Vec3::new(x + 0.05, 3.14, end + 0.07),
        ),
    ] {
        scene.add_box(min, max, trim);
    }
    let middle = (start + end) * 0.5;
    for (left, right, angle, pivot_z) in [
        (start + 0.04, middle, 65.0f32, start + 0.04),
        (middle, end - 0.04, -65.0f32, end - 0.04),
    ] {
        let before = scene.primitives.len();
        for (min, max) in [
            (Vec3::new(x, 0.57, left), Vec3::new(x + 0.065, 0.64, right)),
            (Vec3::new(x, 3.05, left), Vec3::new(x + 0.065, 3.13, right)),
            (
                Vec3::new(x, 0.64, left),
                Vec3::new(x + 0.065, 3.05, left + 0.06),
            ),
            (
                Vec3::new(x, 0.64, right - 0.06),
                Vec3::new(x + 0.065, 3.05, right),
            ),
            (
                Vec3::new(x, 1.67, left + 0.06),
                Vec3::new(x + 0.065, 1.72, right - 0.06),
            ),
        ] {
            scene.add_box(min, max, trim);
        }
        for (bottom, top) in [(0.64, 1.67), (1.72, 3.05)] {
            scene.add_box(
                Vec3::new(x + 0.025, bottom, left + 0.06),
                Vec3::new(x + 0.031, top, right - 0.06),
                glass,
            );
        }
        scene.add_box(
            Vec3::new(x + 0.065, 1.38, right - 0.12),
            Vec3::new(x + 0.09, 1.53, right - 0.09),
            brass,
        );
        let rotation = Mat3::from_rotation_y(angle.to_radians());
        let pivot = Vec3::new(x, 0.0, pivot_z);
        for primitive in &mut scene.primitives[before..] {
            primitive.p0 = pack_vec3(pivot + rotation * (unpack_vec3(primitive.p0) - pivot), 0.0);
            primitive.p1 = pack_vec3(rotation * unpack_vec3(primitive.p1), 0.0);
            primitive.p2 = pack_vec3(rotation * unpack_vec3(primitive.p2), 0.0);
            primitive.n0 = pack_vec3(rotation * unpack_vec3(primitive.n0), 0.0);
            primitive.n1 = pack_vec3(rotation * unpack_vec3(primitive.n1), 0.0);
            primitive.n2 = pack_vec3(rotation * unpack_vec3(primitive.n2), 0.0);
        }
    }
    scene.add_box(
        Vec3::new(-4.83, 1.03, start),
        Vec3::new(-4.79, 1.07, end),
        iron,
    );
    for i in 0..12 {
        let z = start + (end - start) * i as f32 / 11.0;
        scene.add_box(
            Vec3::new(-4.82, 0.55, z - 0.009),
            Vec3::new(-4.80, 1.03, z + 0.009),
            iron,
        );
    }
}
fn folded_curtain(scene: &mut Scene, start: f32, width: f32, material: usize) {
    for column in 0..32 {
        for row in 0..14 {
            let point = |u: f32, v: f32| {
                Vec3::new(
                    -4.08 + 0.06 * (u * TAU * 5.0).sin() + 0.025 * (v * 3.0).sin(),
                    3.47 - v * 3.12,
                    start + u * width,
                )
            };
            let u = column as f32 / 32.0;
            let v = row as f32 / 14.0;
            let points = [
                point(u, v),
                point(u + 1.0 / 32.0, v),
                point(u + 1.0 / 32.0, v + 1.0 / 14.0),
                point(u, v + 1.0 / 14.0),
            ];
            for indices in [[0, 1, 2], [0, 2, 3]] {
                scene.primitives.push(Primitive::triangle(
                    indices.map(|i| points[i]),
                    None,
                    [Vec2::ZERO; 3],
                    material,
                ));
            }
        }
    }
}
fn cylinder(scene: &mut Scene, base: Vec3, radius: f32, height: f32, material: usize) {
    lathe(
        scene,
        base,
        &[(0.0, 0.0), (radius, 0.0), (radius, height), (0.0, height)],
        material,
    );
}
fn lathe(scene: &mut Scene, center: Vec3, profile: &[(f32, f32)], material: usize) {
    for segment in 0..profile.len() - 1 {
        let (r0, y0) = profile[segment];
        let (r1, y1) = profile[segment + 1];
        for i in 0..96 {
            let a = i as f32 * TAU / 96.0;
            let b = (i + 1) as f32 * TAU / 96.0;
            let points = [
                center + Vec3::new(r0 * a.cos(), y0, r0 * a.sin()),
                center + Vec3::new(r1 * a.cos(), y1, r1 * a.sin()),
                center + Vec3::new(r1 * b.cos(), y1, r1 * b.sin()),
                center + Vec3::new(r0 * b.cos(), y0, r0 * b.sin()),
            ];
            let normal = |angle: f32| {
                Vec3::new((y1 - y0) * angle.cos(), r0 - r1, (y1 - y0) * angle.sin()).normalize()
            };
            let normals = [normal(a), normal(a), normal(b), normal(b)];
            for indices in [[0, 1, 2], [0, 2, 3]] {
                let p = indices.map(|j| points[j]);
                if (p[1] - p[0]).cross(p[2] - p[0]).length_squared() > 1e-14 {
                    scene.primitives.push(Primitive::triangle(
                        p,
                        Some(indices.map(|j| normals[j])),
                        [Vec2::ZERO; 3],
                        material,
                    ));
                }
            }
        }
    }
}
fn pendant(scene: &mut Scene, center: Vec3, trim: usize, brass: usize, dark: usize) {
    cylinder(scene, center + Vec3::Y * 3.76, 0.22, 0.04, trim);
    cylinder(scene, center + Vec3::Y * 3.72, 0.15, 0.04, trim);
    cylinder(scene, center + Vec3::Y * 2.72, 0.008, 1.0, dark);
    lathe(
        scene,
        center,
        &[
            (0.0, 2.88),
            (0.10, 2.88),
            (0.20, 2.82),
            (0.34, 2.69),
            (0.49, 2.52),
            (0.59, 2.43),
            (0.59, 2.41),
            (0.56, 2.42),
            (0.46, 2.51),
            (0.31, 2.68),
            (0.17, 2.80),
            (0.08, 2.84),
            (0.0, 2.84),
        ],
        brass,
    );
    let diffuser = scene.add_material(Material::light(Vec3::new(3.0, 2.2, 1.35)));
    cylinder(scene, center + Vec3::Y * 2.435, 0.43, 0.006, diffuser);
}
fn ellipse(scene: &mut Scene, center: Vec3, width: f32, height: f32, material: usize) {
    for i in 0..64 {
        let a = i as f32 * TAU / 64.0;
        let b = (i + 1) as f32 * TAU / 64.0;
        scene.primitives.push(Primitive::triangle(
            [
                center,
                center + Vec3::new(width * a.cos(), height * a.sin(), 0.0),
                center + Vec3::new(width * b.cos(), height * b.sin(), 0.0),
            ],
            None,
            [Vec2::ZERO; 3],
            material,
        ));
    }
}
