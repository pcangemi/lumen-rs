//! Mansard brisis, zinc terrasson, dormers, gutters and terracotta chimney pots.
use super::*;

pub(super) fn build(scene: &mut Scene, m: &FacadeMaterials) {
    let zinc = scene.add_material(Material::metal(Vec3::new(0.36, 0.40, 0.43), 0.47));
    let slate: Vec<_> = (0..7)
        .map(|i| {
            scene.add_material(Material {
                surface: [0.53, 0.12, 0.0, 1.5],
                ..Material::matte(Vec3::new(0.085, 0.105, 0.13) * (0.87 + i as f32 * 0.045))
            })
        })
        .collect();
    let clay = scene.add_material(Material::matte(Vec3::new(0.32, 0.12, 0.065)));
    let soot = scene.add_material(Material::matte(Vec3::splat(0.018)));
    let eave = Vec3::new(FRONT + 0.30, 12.62, -23.4);
    let knee = Vec3::new(FRONT - 1.55, 15.60, -23.4);
    let ridge = Vec3::new(FRONT - 5.3, 16.35, -23.4);
    // Separate lapped slate courses; dormer openings are actual holes in the slope.
    for bay in -6..=7 {
        let center = bay as f32 * BAY_WIDTH;
        for row in 0..12 {
            for column in 0..12 {
                let z0 = center - 1.8 + column as f32 * 0.30;
                let z1 = z0 + 0.298;
                if (4..8).contains(&column) && row < 10 {
                    continue;
                }
                let a = eave.lerp(knee, row as f32 / 12.0);
                let b = eave.lerp(knee, (row + 1) as f32 / 12.0);
                let p = Vec3::new(a.x + 0.008, a.y, z0);
                scene.add_quad(
                    p,
                    Vec3::Z * (z1 - z0),
                    Vec3::new(b.x - a.x, b.y - a.y + 0.012, 0.0),
                    slate[((bay + 6) * 13 + row * 7 + column) as usize % slate.len()],
                );
            }
        }
        dormer(scene, center, m, zinc);
    }
    scene.add_quad(knee, Vec3::Z * 50.4, ridge - knee, zinc);
    scene.add_quad(ridge, Vec3::Z * 50.4, Vec3::new(-4.0, -0.8, 0.0), zinc);
    for i in 0..=91 {
        let z = -23.4 + i as f32 * (50.4 / 91.0);
        tube(
            scene,
            &[
                Vec3::new(knee.x, knee.y + 0.018, z),
                Vec3::new(ridge.x, ridge.y + 0.018, z),
                Vec3::new(ridge.x - 4.0, ridge.y - 0.78, z),
            ],
            0.012,
            zinc,
            6,
        );
    }
    for end in [-23.4, 27.0] {
        let a = Vec3::new(eave.x, eave.y, end);
        let b = Vec3::new(knee.x, knee.y, end);
        let c = Vec3::new(ridge.x, ridge.y, end);
        let base = Vec3::new(ridge.x, eave.y, end);
        for p in [[a, b, c], [a, c, base]] {
            scene
                .primitives
                .push(Primitive::triangle(p, None, [Vec2::ZERO; 3], m.stone[3]));
        }
    }
    tube(
        scene,
        &[
            eave + Vec3::X * 0.14,
            Vec3::new(eave.x + 0.14, eave.y, 27.0),
        ],
        0.085,
        zinc,
        12,
    );
    for z in [-19.8, -9.0, 1.8, 12.6, 23.4] {
        let base = Vec3::new(FRONT - 2.45, 15.2, z);
        scene.add_box(
            base + Vec3::new(-0.38, 0.0, -0.80),
            base + Vec3::new(0.38, 1.75, 0.80),
            m.stone[1],
        );
        for h in [1.35, 1.62, 1.72] {
            scene.add_box(
                base + Vec3::new(-0.44, h, -0.86),
                base + Vec3::new(0.44, h + 0.08, 0.86),
                m.trim,
            );
        }
        for dz in [-0.52, 0.0, 0.52] {
            let pot = base + Vec3::new(0.0, 1.79, dz);
            lathe_segments(
                scene,
                pot,
                &[
                    (0.16, 0.0),
                    (0.16, 0.1),
                    (0.11, 0.13),
                    (0.11, 0.58),
                    (0.15, 0.60),
                    (0.15, 0.68),
                    (0.10, 0.68),
                    (0.10, 0.45),
                ],
                clay,
                24,
            );
            scene.add_quad(
                pot + Vec3::new(-0.095, 0.46, -0.095),
                Vec3::X * 0.19,
                Vec3::Z * 0.19,
                soot,
            );
        }
    }
}

fn dormer(scene: &mut Scene, z: f32, m: &FacadeMaterials, zinc: usize) {
    let x = FRONT + 0.36;
    let y = 12.90;
    let width = 1.02;
    joinery::frame(
        scene,
        Vec3::new(x, y, z - width * 0.5),
        [Vec3::Z, Vec3::Y, Vec3::X],
        Vec2::new(width, 1.84),
        0.09,
        0.12,
        m.trim,
    );
    joinery::frame(
        scene,
        Vec3::new(x - 0.035, y + 0.09, z - 0.42),
        [Vec3::Z, Vec3::Y, Vec3::X],
        Vec2::new(0.84, 1.65),
        0.04,
        0.035,
        m.paint,
    );
    glass(
        scene,
        x - 0.05,
        y + 0.14,
        y + 1.69,
        z - 0.37,
        z + 0.37,
        m.glass,
    );
    scene.add_box(
        Vec3::new(x - 0.08, y + 0.12, z - 0.025),
        Vec3::new(x + 0.03, y + 1.72, z + 0.025),
        m.paint,
    );
    scene.add_box(
        Vec3::new(x - 0.08, y + 0.86, z - 0.39),
        Vec3::new(x + 0.03, y + 0.91, z + 0.39),
        m.paint,
    );
    // Recessed attic volume behind the glazing, closed at the back and sides.
    let back = FRONT - 1.25;
    scene.add_box(
        Vec3::new(back, y, z - 0.60),
        Vec3::new(back + 0.06, y + 1.95, z + 0.60),
        m.rooms[((z + 24.0) as usize) % m.rooms.len()],
    );
    for side in [-1.0, 1.0] {
        let edge = z + side * 0.59;
        scene.add_box(
            Vec3::new(back, y, edge - 0.035),
            Vec3::new(x, y + 1.85, edge + 0.035),
            zinc,
        );
        let a = Vec3::new(x + 0.13, y + 1.83, z + side * 0.66);
        let b = Vec3::new(x + 0.13, y + 2.19, z);
        scene.add_quad(a, b - a, Vec3::X * -1.75, zinc);
        tube(scene, &[a, b], 0.035, zinc, 10);
    }
    scene.primitives.push(Primitive::triangle(
        [
            Vec3::new(x + 0.09, y + 1.83, z - 0.62),
            Vec3::new(x + 0.09, y + 1.83, z + 0.62),
            Vec3::new(x + 0.09, y + 2.15, z),
        ],
        None,
        [Vec2::ZERO; 3],
        m.trim,
    ));
    scene.rounded_box(
        Vec3::new(x - 0.12, y - 0.09, z - 0.62),
        Vec3::new(x + 0.20, y + 0.025, z + 0.62),
        0.01,
        m.trim,
    );
}
