//! Mitered wall trim and painted timber casements with traditional fittings.
use super::*;
use details::tube;

pub(super) fn frame(
    scene: &mut Scene,
    origin: Vec3,
    axes: [Vec3; 3],
    size: Vec2,
    band: f32,
    depth: f32,
    material: usize,
) {
    // One connected profile around all four sides avoids overlapping butt joints.
    let profile = [
        (0.0, -0.003),
        (0.0, depth * 0.45),
        (band * 0.15, depth * 0.9),
        (band * 0.35, depth),
        (band * 0.55, depth * 0.88),
        (band * 0.8, depth * 0.4),
        (band, -0.003),
    ];
    let ring = |(inset, projection): (f32, f32)| {
        [
            Vec2::splat(inset),
            Vec2::new(size.x - inset, inset),
            size - Vec2::splat(inset),
            Vec2::new(inset, size.y - inset),
        ]
        .map(|p| origin + axes[0] * p.x + axes[1] * p.y + axes[2] * projection)
    };
    for i in 0..profile.len() {
        let a = ring(profile[i]);
        let b = ring(profile[(i + 1) % profile.len()]);
        for side in 0..4 {
            let next = (side + 1) % 4;
            let p = [a[side], a[next], b[next], b[side]];
            for tri in [[0, 1, 2], [0, 2, 3]] {
                scene.primitives.push(Primitive::triangle(
                    tri.map(|j| p[j]),
                    None,
                    [Vec2::ZERO; 3],
                    material,
                ));
            }
        }
    }
}

#[allow(clippy::approx_constant, reason = "Dimensions are in meters")]
pub(super) fn window(
    scene: &mut Scene,
    start: f32,
    end: f32,
    paint: usize,
    brass: usize,
    iron: usize,
    glass: usize,
) {
    let x = -4.38;
    // Deep sill and timber lining, with eased exposed edges.
    for (lo, hi, radius) in [
        (
            Vec3::new(x - 0.2, 0.71, start - 0.06),
            Vec3::new(x + 0.2, 0.79, end + 0.06),
            0.008,
        ),
        (
            Vec3::new(x - 0.18, 3.14, start - 0.07),
            Vec3::new(x + 0.05, 3.25, end + 0.07),
            0.006,
        ),
        (
            Vec3::new(x - 0.18, 0.79, start - 0.07),
            Vec3::new(x + 0.05, 3.14, start + 0.04),
            0.005,
        ),
        (
            Vec3::new(x - 0.18, 0.79, end - 0.04),
            Vec3::new(x + 0.05, 3.14, end + 0.07),
            0.005,
        ),
    ] {
        scene.rounded_box(lo, hi, radius, paint);
    }
    let middle = (start + end) * 0.5;
    let variation = 6.0 * (start * 2.3).sin();
    for (left, right, angle, pivot_z) in [
        (start + 0.04, middle, 62.0 + variation, start + 0.04),
        (middle, end - 0.04, -70.0 + variation * 0.6, end - 0.04),
    ] {
        let before = scene.primitives.len();
        for (lo, hi) in [
            (Vec3::new(x, 0.81, left), Vec3::new(x + 0.065, 0.88, right)),
            (Vec3::new(x, 3.05, left), Vec3::new(x + 0.065, 3.13, right)),
            (
                Vec3::new(x, 0.88, left),
                Vec3::new(x + 0.065, 3.05, left + 0.06),
            ),
            (
                Vec3::new(x, 0.88, right - 0.06),
                Vec3::new(x + 0.065, 3.05, right),
            ),
        ] {
            scene.rounded_box(lo, hi, 0.005, paint);
        }
        for y in [1.59, 2.32] {
            scene.rounded_box(
                Vec3::new(x, y, left + 0.05),
                Vec3::new(x + 0.066, y + 0.035, right - 0.05),
                0.006,
                paint,
            );
        }
        for (bottom, top) in [(0.88, 1.59), (1.625, 2.32), (2.355, 3.05)] {
            // Ogee glazing beads and separate putty rebates surround each pane.
            frame(
                scene,
                Vec3::new(x + 0.058, bottom - 0.018, left + 0.042),
                [Vec3::Z, Vec3::Y, Vec3::X],
                Vec2::new(right - left - 0.084, top - bottom + 0.036),
                0.018,
                0.012,
                paint,
            );
            pane(
                scene,
                x + 0.027,
                bottom,
                top,
                left + 0.06,
                right - 0.06,
                glass,
            );
        }
        let closing = if angle > 0.0 {
            right - 0.03
        } else {
            left + 0.03
        };
        // Rounded meeting stile, surface-mounted crémone and rod guides.
        tube(
            scene,
            &[
                Vec3::new(x + 0.060, 0.85, closing),
                Vec3::new(x + 0.060, 3.09, closing),
            ],
            0.023,
            paint,
            16,
        );
        tube(
            scene,
            &[
                Vec3::new(x + 0.089, 0.88, closing),
                Vec3::new(x + 0.089, 3.06, closing),
            ],
            0.004,
            brass,
            10,
        );
        for y in [1.02, 1.57, 2.22, 2.94] {
            scene.rounded_box(
                Vec3::new(x + 0.071, y - 0.018, closing - 0.014),
                Vec3::new(x + 0.098, y + 0.018, closing + 0.014),
                0.006,
                brass,
            );
        }
        scene.rounded_box(
            Vec3::new(x + 0.076, 1.42, closing - 0.024),
            Vec3::new(x + 0.105, 1.57, closing + 0.024),
            0.012,
            brass,
        );
        let grip: Vec<_> = (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0;
                Vec3::new(
                    x + 0.105 + 0.028 * (t * std::f32::consts::PI).sin(),
                    1.46 + t * 0.10,
                    closing + 0.028 * (t * TAU).sin(),
                )
            })
            .collect();
        tube(scene, &grip, 0.008, brass, 10);
        for y in [1.02, 1.97, 2.92] {
            let hinge = Vec3::new(x + 0.01, y, pivot_z);
            lathe_segments(
                scene,
                hinge,
                &[
                    (0.0, -0.065),
                    (0.009, -0.06),
                    (0.013, -0.045),
                    (0.012, 0.045),
                    (0.009, 0.06),
                    (0.0, 0.065),
                ],
                brass,
                16,
            );
        }
        let rotation = Mat3::from_rotation_y(angle.to_radians());
        let pivot = Vec3::new(x, 0.0, pivot_z);
        for primitive in &mut scene.primitives[before..] {
            primitive.p0 = pack_vec3(
                pivot + rotation * (unpack_vec3(primitive.p0) - pivot),
                primitive.p0[3],
            );
            primitive.p1 = pack_vec3(rotation * unpack_vec3(primitive.p1), 0.0);
            primitive.p2 = pack_vec3(rotation * unpack_vec3(primitive.p2), 0.0);
            primitive.n0 = pack_vec3(rotation * unpack_vec3(primitive.n0), 0.0);
            primitive.n1 = pack_vec3(rotation * unpack_vec3(primitive.n1), 0.0);
            primitive.n2 = pack_vec3(rotation * unpack_vec3(primitive.n2), 0.0);
        }
    }
    atmosphere::balcony(scene, start, end, iron);
}

fn pane(scene: &mut Scene, x: f32, bottom: f32, top: f32, left: f32, right: f32, glass: usize) {
    const COLUMNS: usize = 12;
    const ROWS: usize = 20;
    let point = |u: f32, v: f32, side: f32| {
        let y = bottom + v * (top - bottom);
        let z = left + u * (right - left);
        Vec3::new(
            x + side * 0.003 + 0.00022 * (z * 11.0).sin() * (y * 4.7).sin(),
            y,
            z,
        )
    };
    let normal = |p: Vec3, side: f32| {
        Vec3::new(
            1.0,
            -0.00022 * 4.7 * (p.z * 11.0).sin() * (p.y * 4.7).cos(),
            -0.00022 * 11.0 * (p.z * 11.0).cos() * (p.y * 4.7).sin(),
        )
        .normalize()
            * side
    };
    for side in [-1.0, 1.0] {
        for column in 0..COLUMNS {
            for row in 0..ROWS {
                let u = column as f32 / COLUMNS as f32;
                let v = row as f32 / ROWS as f32;
                let p = [
                    point(u, v, side),
                    point(u + 1.0 / COLUMNS as f32, v, side),
                    point(u + 1.0 / COLUMNS as f32, v + 1.0 / ROWS as f32, side),
                    point(u, v + 1.0 / ROWS as f32, side),
                ];
                let indices = if side > 0.0 {
                    [[0, 2, 1], [0, 3, 2]]
                } else {
                    [[0, 1, 2], [0, 2, 3]]
                };
                for tri in indices {
                    scene.primitives.push(Primitive::triangle(
                        tri.map(|i| p[i]),
                        Some(tri.map(|i| normal(p[i], side))),
                        [Vec2::ZERO; 3],
                        glass,
                    ));
                }
            }
        }
    }
    // Seal every subdivided boundary: waviness must not make an open dielectric.
    for (axis, steps) in [(0, COLUMNS), (1, ROWS)] {
        for edge in [0.0, 1.0] {
            for i in 0..steps {
                let uv = |t: f32| {
                    if axis == 0 {
                        Vec2::new(t, edge)
                    } else {
                        Vec2::new(edge, t)
                    }
                };
                let a = uv(i as f32 / steps as f32);
                let b = uv((i + 1) as f32 / steps as f32);
                let mut p = [
                    point(a.x, a.y, 1.0),
                    point(b.x, b.y, 1.0),
                    point(b.x, b.y, -1.0),
                    point(a.x, a.y, -1.0),
                ];
                let outward = if axis == 0 { Vec3::Y } else { Vec3::Z } * (edge * 2.0 - 1.0);
                if (p[1] - p[0]).cross(p[2] - p[0]).dot(outward) < 0.0 {
                    p.reverse();
                }
                for tri in [[0, 1, 2], [0, 2, 3]] {
                    scene.primitives.push(Primitive::triangle(
                        tri.map(|j| p[j]),
                        Some([outward; 3]),
                        [Vec2::ZERO; 3],
                        glass,
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Ray;

    #[test]
    fn wavy_glass_has_outward_entry_and_exit_boundaries() {
        let mut scene = Scene::empty(Camera::new(Vec3::X * 2.0, Vec3::ZERO, 45.0, 1.0, 0.0, 2.0));
        let glass = scene.add_material(Material::glass(1.52, Vec3::ZERO));
        pane(&mut scene, 0.0, 0.0, 1.0, 0.0, 1.0, glass);
        scene.finish().unwrap();
        for direction in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z] {
            let center = Vec3::new(0.0, 0.5, 0.5);
            let entry = scene
                .intersect(
                    Ray {
                        origin: center + direction * 2.0,
                        direction: -direction,
                    },
                    4.0,
                    false,
                )
                .unwrap();
            let exit = scene
                .intersect(
                    Ray {
                        origin: center,
                        direction,
                    },
                    4.0,
                    false,
                )
                .unwrap();
            assert!(entry.front);
            assert!(!exit.front);
        }
    }
}
