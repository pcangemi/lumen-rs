//! Detailed apartment interior with cut parquet boards, joinery, mouldings and soft furnishings.
mod atmosphere;
mod details;
mod exterior;
mod joinery;
use super::*;
use crate::geometry::{pack_vec3, unpack_vec3};
use crate::rng::Rng;
use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Lighting {
    Day,
    Night,
}

pub fn build(aspect: f32, lighting: Lighting) -> Result<Scene> {
    let from = Vec3::new(3.05, 1.65, 4.1);
    let target = Vec3::new(-0.2, 1.3, -2.0);
    let mut scene = Scene::empty(Camera::new(
        from,
        target,
        58.0,
        aspect,
        0.0,
        from.distance(target),
    ));
    let night = lighting == Lighting::Night;
    scene.sky = if night {
        Vec3::new(0.002, 0.004, 0.012)
    } else {
        Vec3::new(0.48, 0.62, 0.85)
    };
    scene.horizon = if night {
        Vec3::new(0.008, 0.012, 0.025)
    } else {
        Vec3::new(0.75, 0.74, 0.69)
    };
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let oak_texture = scene.textures.load(&asset_root.join("textures/oak.jpg"))?;
    let plaster_texture = scene
        .textures
        .load(&asset_root.join("textures/plaster.jpg"))?;
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
    let window_paint = scene.add_material(Material {
        surface: [0.43, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.84, 0.83, 0.78))
            .textured(plaster_texture, -24.0)
            .texture_strength(0.06)
    });
    let radiator_paint = scene.add_material(Material {
        surface: [0.52, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.61, 0.62, 0.57))
            .textured(plaster_texture, -8.0)
            .texture_strength(0.22)
    });
    let dark = scene.add_material(Material::metal(Vec3::splat(0.035), 0.35));
    let brass = scene.add_material(Material::metal(Vec3::new(0.74, 0.49, 0.22), 0.3));
    let walnut = scene.add_material(Material {
        surface: [0.32, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.46, 0.28, 0.15)).textured(oak_texture, -0.7)
    });
    let curtain = scene.add_material(
        Material::matte(Vec3::new(0.89, 0.87, 0.80))
            .textured(linen_texture, 3.0)
            .texture_strength(0.3),
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
        Vec3::new(-4.4, 0.72, 9.0),
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
            Vec3::new(-4.65, 0.72, previous),
            Vec3::new(-4.4, 3.22, start),
            plaster,
        );
        window(&mut scene, start, end, window_paint, brass, dark, glass);
        details::window_surround(&mut scene, start, end, trim, brass);
        details::radiator(&mut scene, (start + end) * 0.5, radiator_paint, brass, dark);
        // The bunched cloth lies entirely outside the swept casement opening.
        folded_curtain(&mut scene, start - 0.48, 0.36, curtain);
        folded_curtain(&mut scene, end + 0.12, 0.36, curtain);
        previous = end;
    }
    scene.add_box(
        Vec3::new(-4.65, 0.72, previous),
        Vec3::new(-4.4, 3.22, 9.0),
        plaster,
    );
    exterior::build(&mut scene, &asset_root, night)?;

    mouldings(&mut scene, trim);
    details::wool_rug(&mut scene, &asset_root)?;
    details::imported_furniture(&mut scene, &asset_root)?;

    details::apartment_decor(&mut scene, &asset_root, brass, night)?;

    // Low circular table, layered books, and original ceramic vessel.
    let paper = scene.add_material(Material::matte(Vec3::new(0.76, 0.73, 0.64)));
    let book = scene.add_material(Material::matte(Vec3::new(0.12, 0.18, 0.17)));
    scene.add_box(
        Vec3::new(0.68, 0.554, -2.0),
        Vec3::new(1.18, 0.572, -1.65),
        book,
    );
    scene.add_box(
        Vec3::new(0.69, 0.572, -1.99),
        Vec3::new(1.17, 0.617, -1.66),
        paper,
    );
    scene.add_box(
        Vec3::new(0.68, 0.617, -2.0),
        Vec3::new(1.18, 0.634, -1.65),
        book,
    );
    let ceramic = scene.add_material(Material {
        surface: [0.21, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.22, 0.27, 0.18))
    });
    scene.import_obj(
        &asset_root.join("vessel.obj"),
        Vec3::new(4.02, 0.693, -1.9),
        0.38,
        0.0,
        Some(ceramic),
    )?;

    atmosphere::canvas(&mut scene, &asset_root, walnut)?;
    atmosphere::lamps(&mut scene, &asset_root, night)?;
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
    for (left, right) in [(-4.05, -1.65), (-1.3, 4.12)] {
        panel(scene, left, right, 0.32, 0.94, material);
    }
    panel(scene, -4.05, -1.65, 1.4, 3.29, material);
    panel(scene, -1.3, 4.12, 1.4, 3.29, material);
}
fn panel(scene: &mut Scene, left: f32, right: f32, bottom: f32, top: f32, material: usize) {
    for (inset, width, depth) in [(0.0, 0.035, 0.026), (0.044, 0.014, 0.018)] {
        joinery::frame(
            scene,
            Vec3::new(left + inset, bottom + inset, -4.499),
            [Vec3::X, Vec3::Y, Vec3::Z],
            Vec2::new(right - left - 2.0 * inset, top - bottom - 2.0 * inset),
            width,
            depth,
            material,
        );
    }
}

fn window(
    scene: &mut Scene,
    start: f32,
    end: f32,
    trim: usize,
    brass: usize,
    iron: usize,
    glass: usize,
) {
    joinery::window(scene, start, end, trim, brass, iron, glass);
}

fn folded_curtain(scene: &mut Scene, start: f32, width: f32, material: usize) {
    atmosphere::curtain(scene, start, width, material);
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
    lathe_segments(scene, center, profile, material, 96);
}
fn lathe_segments(
    scene: &mut Scene,
    center: Vec3,
    profile: &[(f32, f32)],
    material: usize,
    steps: usize,
) {
    for segment in 0..profile.len() - 1 {
        let (r0, y0) = profile[segment];
        let (r1, y1) = profile[segment + 1];
        for i in 0..steps {
            let a = i as f32 * TAU / steps as f32;
            let b = (i + 1) as f32 * TAU / steps as f32;
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_is_centered_within_the_large_wall_panel() {
        let mut scene = Scene::empty(Camera::new(Vec3::Z * 6.0, Vec3::ZERO, 54.0, 1.5, 0.0, 6.0));
        let frame = scene.add_material(Material::matte(Vec3::splat(0.2)));
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        atmosphere::canvas(&mut scene, &root, frame).unwrap();
        let (lo, hi) = scene.primitives.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), primitive| {
                let bounds = primitive.bounds();
                (lo.min(bounds.0), hi.max(bounds.1))
            },
        );
        let center = (lo + hi) * 0.5;
        assert!((center.x - 1.41).abs() < 0.001);
        assert!((center.y - 2.345).abs() < 0.001);
        assert!(lo.y > 1.4 && hi.y < 3.29);
    }

    #[test]
    fn curtains_clear_open_casements_and_radiators_fit_below_sills() {
        let mut scene = Scene::empty(Camera::new(Vec3::Z * 6.0, Vec3::ZERO, 54.0, 1.5, 0.0, 6.0));
        for (start, end) in [(-3.7, -1.9), (-0.8, 1.0), (2.1, 3.9)] {
            window(&mut scene, start, end, 0, 1, 2, 3);
            let window_end = scene.primitives.len();
            folded_curtain(&mut scene, start - 0.48, 0.36, 4);
            folded_curtain(&mut scene, end + 0.12, 0.36, 4);
            // Every cloth triangle is outside the opening, including the rotated frame thickness.
            for primitive in &scene.primitives[window_end..] {
                let (lo, hi) = primitive.bounds();
                assert!(hi.z < start - 0.07 || lo.z > end + 0.07);
                assert!(lo.y > 0.01);
            }
            let before = scene.primitives.len();
            details::radiator(&mut scene, (start + end) * 0.5, 0, 1, 2);
            for primitive in &scene.primitives[before..] {
                assert!(primitive.bounds().1.y < 0.69);
            }
            scene.primitives.clear();
        }
    }
}
