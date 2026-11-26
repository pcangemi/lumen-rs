//! Recessed limestone facade and street lighting below the apartment's sightline.
use super::*;
use details::tube;

const FRONT: f32 = -15.4;
const STREET: f32 = -7.3;
const BAY_WIDTH: f32 = 3.6;
const FLOOR_HEIGHT: f32 = 3.1;

struct FacadeMaterials {
    stone: Vec<usize>,
    mortar: usize,
    trim: usize,
    paint: usize,
    iron: usize,
    glass: usize,
    curtain: usize,
    shutter: usize,
    rooms: Vec<usize>,
    room_lights: [usize; 2],
}

pub(super) fn build(scene: &mut Scene, root: &Path, night: bool) -> Result<()> {
    let sky = scene.add_material(Material::light(if night {
        Vec3::new(0.05, 0.09, 0.22)
    } else {
        Vec3::new(18.0, 21.0, 26.0)
    }));
    // The sky card is above the view; visible windows contain actual recessed rooms.
    scene.add_quad(
        Vec3::new(-8.0, 4.5, -6.0),
        Vec3::Z * 16.0,
        Vec3::Y * 8.0,
        sky,
    );
    if !night {
        let sun = scene.add_material(Material::light(Vec3::new(140.0, 112.0, 77.0)));
        scene.add_quad(Vec3::new(-8.0, 5.0, 3.0), Vec3::Y * 0.7, Vec3::Z * 0.9, sun);
    }
    let plaster = scene.textures.load(&root.join("textures/plaster.jpg"))?;
    let linen = scene.textures.load(&root.join("textures/linen.jpg"))?;
    let materials = FacadeMaterials {
        stone: (0..7)
            .map(|i| {
                scene.add_material(
                    Material::matte(Vec3::new(0.57, 0.53, 0.45) * (0.93 + i as f32 * 0.018))
                        .textured(plaster, -2.1)
                        .texture_strength(0.2),
                )
            })
            .collect(),
        mortar: scene.add_material(Material::matte(Vec3::new(0.34, 0.32, 0.28))),
        trim: scene.add_material(
            Material::matte(Vec3::new(0.67, 0.63, 0.54))
                .textured(plaster, -2.1)
                .texture_strength(0.14),
        ),
        paint: scene.add_material(Material {
            surface: [0.38, 0.0, 0.0, 1.5],
            ..Material::matte(Vec3::new(0.76, 0.75, 0.69))
        }),
        iron: scene.add_material(Material::metal(Vec3::new(0.055, 0.063, 0.058), 0.34)),
        glass: scene.add_material(Material::thin_glass(1.52)),
        curtain: scene.add_material(
            Material::matte(Vec3::new(0.66, 0.61, 0.5))
                .textured(linen, -1.2)
                .texture_strength(0.25)
                .translucent(0.3),
        ),
        shutter: scene.add_material(Material::matte(Vec3::new(0.19, 0.24, 0.23))),
        rooms: [
            Vec3::new(0.36, 0.3, 0.23),
            Vec3::new(0.31, 0.35, 0.34),
            Vec3::new(0.48, 0.43, 0.35),
            Vec3::new(0.36, 0.31, 0.34),
        ]
        .into_iter()
        .map(|color| scene.add_material(Material::matte(color)))
        .collect(),
        room_lights: [Vec3::new(16.0, 9.0, 3.8), Vec3::new(11.0, 10.0, 7.5)].map(|emission| {
            scene.add_material(if night {
                Material::light(emission)
            } else {
                Material::matte(Vec3::splat(0.75))
            })
        }),
    };
    for level in -2i32..=2 {
        let y = level as f32 * FLOOR_HEIGHT;
        for bay in -6i32..=7 {
            let z = bay as f32 * BAY_WIDTH;
            // Four masonry patches leave a genuine opening, including its deep reveal.
            for (bottom, top, left, right) in [
                (y, y + FLOOR_HEIGHT, z - 1.8, z - 0.69),
                (y, y + FLOOR_HEIGHT, z + 0.69, z + 1.8),
                (y, y + 0.35, z - 0.69, z + 0.69),
                (y + 2.65, y + FLOOR_HEIGHT, z - 0.69, z + 0.69),
            ] {
                masonry(scene, bottom, top, left, right, &materials);
            }
            window(
                scene,
                y,
                z,
                (bay + level * 3).rem_euclid(11) as usize,
                &materials,
            );
        }
        // Layered string courses catch daylight and the upward street illumination.
        for (bottom, top, projection) in [
            (-0.13, -0.035, 0.12),
            (-0.035, 0.015, 0.23),
            (0.015, 0.05, 0.16),
        ] {
            scene.add_box(
                Vec3::new(FRONT - 0.04, y + bottom, -23.4),
                Vec3::new(FRONT + projection, y + top, 27.0),
                materials.trim,
            );
        }
    }
    for z in [-16.2, -5.4, 5.4, 16.2] {
        tube(
            scene,
            &[
                Vec3::new(FRONT + 0.15, STREET, z),
                Vec3::new(FRONT + 0.15, 9.2, z),
            ],
            0.045,
            materials.iron,
            10,
        );
        for level in -2..=2 {
            scene.add_box(
                Vec3::new(FRONT, level as f32 * FLOOR_HEIGHT + 1.0, z - 0.06),
                Vec3::new(FRONT + 0.21, level as f32 * FLOOR_HEIGHT + 1.055, z + 0.06),
                materials.iron,
            );
        }
    }
    for (height, depth) in [(9.22, 0.17), (9.32, 0.35), (9.42, 0.48)] {
        scene.add_box(
            Vec3::new(FRONT - 0.3, height, -23.45),
            Vec3::new(FRONT + depth, height + 0.1, 27.05),
            materials.trim,
        );
    }
    let slate = scene.add_material(Material {
        surface: [0.5, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.075, 0.09, 0.11))
    });
    scene.add_quad(
        Vec3::new(FRONT + 0.3, 9.52, -23.4),
        Vec3::Z * 50.4,
        Vec3::new(-1.8, 2.4, 0.0),
        slate,
    );
    street(scene, &materials, night);
    Ok(())
}

fn masonry(scene: &mut Scene, bottom: f32, top: f32, left: f32, right: f32, m: &FacadeMaterials) {
    scene.add_box(
        Vec3::new(FRONT - 0.75, bottom, left),
        Vec3::new(FRONT - 0.025, top, right),
        m.mortar,
    );
    for row in (bottom / 0.43).floor() as i32..(top / 0.43).ceil() as i32 {
        let shift = if row.rem_euclid(2) == 0 { 0.0 } else { 0.47 };
        for column in ((left - shift) / 0.94).floor() as i32..((right - shift) / 0.94).ceil() as i32
        {
            let y0 = bottom.max(row as f32 * 0.43 + 0.002);
            let y1 = top.min((row + 1) as f32 * 0.43 - 0.002);
            let z0 = left.max(column as f32 * 0.94 + shift + 0.002);
            let z1 = right.min((column + 1) as f32 * 0.94 + shift - 0.002);
            if y1 <= y0 || z1 <= z0 {
                continue;
            }
            let variant = (row * 17 + column * 7).rem_euclid(m.stone.len() as i32) as usize;
            scene.add_box(
                Vec3::new(FRONT - 0.03, y0, z0),
                Vec3::new(FRONT + variant as f32 * 0.0006, y1, z1),
                m.stone[variant],
            );
        }
    }
}

fn window(scene: &mut Scene, y: f32, z: f32, variation: usize, m: &FacadeMaterials) {
    let bottom = y + 0.35;
    let top = y + 2.65;
    let left = z - 0.69;
    let right = z + 0.69;
    joinery::frame(
        scene,
        Vec3::new(FRONT + 0.006, bottom - 0.12, left - 0.13),
        [Vec3::Z, Vec3::Y, Vec3::X],
        Vec2::new(1.64, 2.55),
        0.13,
        0.065,
        m.trim,
    );
    for (low, high, projection) in [(-0.13, -0.04, 0.16), (-0.04, 0.015, 0.31)] {
        scene.rounded_box(
            Vec3::new(FRONT - 0.22, bottom + low, left - 0.19),
            Vec3::new(FRONT + projection, bottom + high, right + 0.19),
            0.007,
            m.trim,
        );
    }
    for dz in [-0.5, 0.5] {
        scene.rounded_box(
            Vec3::new(FRONT - 0.02, bottom - 0.3, z + dz - 0.06),
            Vec3::new(FRONT + 0.12, bottom - 0.12, z + dz + 0.06),
            0.012,
            m.trim,
        );
    }
    scene.add_box(
        Vec3::new(FRONT - 0.02, top + 0.12, left - 0.17),
        Vec3::new(FRONT + 0.18, top + 0.19, right + 0.17),
        m.trim,
    );
    scene.add_box(
        Vec3::new(FRONT, top + 0.19, left - 0.21),
        Vec3::new(FRONT + 0.24, top + 0.24, right + 0.21),
        m.trim,
    );
    let x = FRONT - 0.23;
    joinery::frame(
        scene,
        Vec3::new(x, bottom + 0.025, left + 0.025),
        [Vec3::Z, Vec3::Y, Vec3::X],
        Vec2::new(1.33, 2.25),
        0.065,
        0.04,
        m.paint,
    );
    scene.rounded_box(
        Vec3::new(x - 0.03, bottom + 0.07, z - 0.035),
        Vec3::new(x + 0.065, top - 0.07, z + 0.035),
        0.007,
        m.paint,
    );
    for pane in 0..3 {
        let low = bottom + 0.075 + pane as f32 * 0.72;
        for (a, b) in [(left + 0.08, z - 0.04), (z + 0.04, right - 0.08)] {
            joinery::frame(
                scene,
                Vec3::new(x + 0.015, low, a),
                [Vec3::Z, Vec3::Y, Vec3::X],
                Vec2::new(b - a, 0.72),
                0.018,
                0.014,
                m.paint,
            );
            glass(
                scene,
                x,
                low + 0.017,
                low + 0.703,
                a + 0.017,
                b - 0.017,
                m.glass,
            );
        }
        if pane < 2 {
            scene.add_box(
                Vec3::new(x - 0.025, low + 0.70, left + 0.06),
                Vec3::new(x + 0.04, low + 0.745, right - 0.06),
                m.paint,
            );
        }
    }
    let room = m.rooms[variation % m.rooms.len()];
    scene.add_box(
        Vec3::new(FRONT - 2.0, y + 0.05, left - 0.1),
        Vec3::new(FRONT - 1.96, y + 2.95, right + 0.1),
        room,
    );
    for (lo, hi) in [
        (
            Vec3::new(FRONT - 2.0, y + 0.05, left - 0.1),
            Vec3::new(x, bottom, right + 0.1),
        ),
        (
            Vec3::new(FRONT - 2.0, top, left - 0.1),
            Vec3::new(x, y + 2.95, right + 0.1),
        ),
        (
            Vec3::new(FRONT - 2.0, bottom, left - 0.1),
            Vec3::new(x, top, left),
        ),
        (
            Vec3::new(FRONT - 2.0, bottom, right),
            Vec3::new(x, top, right + 0.1),
        ),
    ] {
        scene.add_box(lo, hi, room);
    }
    for (start, width) in [
        (left + 0.01, 0.19 + variation as f32 * 0.013),
        (right - 0.22, 0.21),
    ] {
        for fold in 0..16 {
            let a = fold as f32 / 16.0;
            let b = (fold + 1) as f32 / 16.0;
            let p = |u: f32| {
                Vec3::new(
                    x - 0.14 + 0.027 * (u * TAU * 3.0).sin(),
                    bottom + 0.03,
                    start + width * u,
                )
            };
            scene.add_quad(p(a), p(b) - p(a), Vec3::Y * 2.2, m.curtain);
        }
    }
    if variation.is_multiple_of(3) {
        scene.add_quad(
            Vec3::new(FRONT - 1.4, y + 2.58, z - 0.25),
            Vec3::X * 0.45,
            Vec3::Z * 0.5,
            m.room_lights[variation % 2],
        );
    }
    if variation == 1 || variation == 7 {
        for side in [-1.0, 1.0] {
            let shutter_z = z + side * 1.09 - 0.29;
            joinery::frame(
                scene,
                Vec3::new(FRONT + 0.055, bottom, shutter_z),
                [Vec3::Z, Vec3::Y, Vec3::X],
                Vec2::new(0.58, 2.3),
                0.055,
                0.035,
                m.shutter,
            );
            for slat in 0..24 {
                scene.add_box(
                    Vec3::new(
                        FRONT + 0.04,
                        bottom + 0.065 + slat as f32 * 0.09,
                        shutter_z + 0.05,
                    ),
                    Vec3::new(
                        FRONT + 0.085,
                        bottom + 0.135 + slat as f32 * 0.09,
                        shutter_z + 0.53,
                    ),
                    m.shutter,
                );
            }
        }
    }
    railing(scene, bottom + 0.06, z, m.iron);
}

fn glass(scene: &mut Scene, x: f32, bottom: f32, top: f32, left: f32, right: f32, material: usize) {
    let point = |u: f32, v: f32| {
        let y = bottom + (top - bottom) * v;
        let z = left + (right - left) * u;
        Vec3::new(
            x + 0.0006 * (y * 14.0 + z * 9.0).sin() * (z * 13.0).cos(),
            y,
            z,
        )
    };
    let normal = |u, v| {
        (point(u, v + 0.001) - point(u, v - 0.001))
            .cross(point(u + 0.001, v) - point(u - 0.001, v))
            .normalize()
    };
    for column in 0..4 {
        for row in 0..6 {
            let u = column as f32 / 4.0;
            let v = row as f32 / 6.0;
            let uv = [
                (u, v),
                (u, v + 1.0 / 6.0),
                (u + 0.25, v + 1.0 / 6.0),
                (u + 0.25, v),
            ];
            for indices in [[0, 1, 2], [0, 2, 3]] {
                scene.primitives.push(Primitive::triangle(
                    indices.map(|i| point(uv[i].0, uv[i].1)),
                    Some(indices.map(|i| normal(uv[i].0, uv[i].1))),
                    [Vec2::ZERO; 3],
                    material,
                ));
            }
        }
    }
}

fn railing(scene: &mut Scene, bottom: f32, z: f32, iron: usize) {
    let x = FRONT + 0.29;
    for y in [bottom + 0.03, bottom + 0.82] {
        tube(
            scene,
            &[Vec3::new(x, y, z - 0.81), Vec3::new(x, y, z + 0.81)],
            0.016,
            iron,
            8,
        );
    }
    for i in 0..=8 {
        let a = z - 0.78 + i as f32 * 0.195;
        tube(
            scene,
            &[Vec3::new(x, bottom, a), Vec3::new(x, bottom + 0.83, a)],
            0.008,
            iron,
            6,
        );
        if i < 8 {
            let points: Vec<_> = (0..=16)
                .map(|j| {
                    let t = j as f32 * TAU / 16.0;
                    Vec3::new(
                        x,
                        bottom + 0.31 + 0.15 * t.sin(),
                        a + 0.0975 + 0.075 * t.cos(),
                    )
                })
                .collect();
            tube(scene, &points, 0.0045, iron, 5);
        }
    }
}

fn street(scene: &mut Scene, m: &FacadeMaterials, night: bool) {
    let asphalt = scene.add_material(Material {
        surface: [0.26, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.055, 0.06, 0.065)).patterned(1, 12.0)
    });
    scene.add_box(
        Vec3::new(FRONT, STREET - 0.15, -30.0),
        Vec3::new(-4.6, STREET, 30.0),
        asphalt,
    );
    for (left, right) in [(FRONT, -13.85), (-6.15, -4.6)] {
        scene.add_box(
            Vec3::new(left, STREET, -30.0),
            Vec3::new(right, STREET + 0.12, 30.0),
            m.trim,
        );
    }
    // Continue the apartment building down to street level; lamps cannot shine
    // through a missing lower wall into the room or directly into the camera.
    scene.add_box(
        Vec3::new(-4.72, STREET, -4.5),
        Vec3::new(-4.6, 0.0, 9.0),
        m.stone[2],
    );
    let source = scene.add_material(if night {
        Material::light(Vec3::new(280.0, 185.0, 95.0))
    } else {
        Material::matte(Vec3::splat(0.78))
    });
    for (x, z, height) in [
        (-14.0, -12.0, 4.5),
        (-14.0, -2.0, 4.5),
        (-14.0, 8.0, 4.5),
        (-14.0, 18.0, 4.5),
        (-5.7, -24.3, 7.2),
        (-5.7, -8.0, 7.2),
        (-5.7, 3.2, 7.2),
    ] {
        let bulb = Vec3::new(x, STREET + height, z);
        tube(
            scene,
            &[Vec3::new(x, STREET + 0.12, z), bulb - Vec3::Y * 0.3],
            0.055,
            m.iron,
            10,
        );
        scene.rounded_box(
            Vec3::new(x - 0.13, STREET + 0.12, z - 0.13),
            Vec3::new(x + 0.13, STREET + 0.3, z + 0.13),
            0.025,
            m.iron,
        );
        scene.add_sphere(bulb, 0.075, source);
        for height in [-0.27, 0.27] {
            scene.rounded_box(
                bulb + Vec3::new(-0.19, height - 0.025, -0.19),
                bulb + Vec3::new(0.19, height + 0.025, 0.19),
                0.015,
                m.iron,
            );
        }
        for dx in [-0.17, 0.17] {
            for dz in [-0.17, 0.17] {
                tube(
                    scene,
                    &[
                        bulb + Vec3::new(dx, -0.27, dz),
                        bulb + Vec3::new(dx, 0.27, dz),
                    ],
                    0.012,
                    m.iron,
                    6,
                );
            }
            scene.add_quad(
                bulb + Vec3::new(dx, -0.25, -0.17),
                Vec3::Y * 0.5,
                Vec3::Z * 0.34,
                m.glass,
            );
            scene.add_quad(
                bulb + Vec3::new(-0.17, -0.25, dx),
                Vec3::Y * 0.5,
                Vec3::X * 0.34,
                m.glass,
            );
        }
        scene.add_sphere(bulb + Vec3::Y * 0.32, 0.055, m.iron);
    }
}
