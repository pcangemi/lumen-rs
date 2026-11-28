//! Individually beveled mosaic tesserae beneath a closed, displaced water volume.
use super::*;

pub(super) const MIN: Vec3 = Vec3::new(-9.0, -1.46, 3.0);
pub(super) const MAX: Vec3 = Vec3::new(3.0, 0.04, 9.0);
const PITCH: f32 = 0.04;

pub(super) fn build(scene: &mut Scene, coping: usize) -> Result<()> {
    let grout = scene.add_material(Material::matte(Vec3::new(0.23, 0.31, 0.36)));
    let blues: Vec<_> = (0..16)
        .map(|i| {
            let t = i as f32 / 15.0;
            scene.add_material(Material {
                surface: [0.18 + 0.10 * t, 0.0, 0.0, 1.5],
                ..Material::matte(Vec3::new(0.025, 0.14, 0.30).lerp(Vec3::new(0.09, 0.35, 0.56), t))
            })
        })
        .collect();
    let golds: Vec<_> = (0..8)
        .map(|i| {
            let t = i as f32 / 7.0;
            scene.add_material(Material::metal(
                Vec3::new(0.83, 0.48, 0.12).lerp(Vec3::new(1.0, 0.78, 0.34), t),
                0.20 + t * 0.10,
            ))
        })
        .collect();
    let mut rng = Rng::new(913, 0, 42);
    let width = MAX.x - MIN.x;
    let depth = MAX.z - MIN.z;
    let height = MAX.y + 0.10 - MIN.y;
    for (origin, u, v, motif) in [
        (MIN, Vec3::Z * depth, Vec3::X * width, true),
        (MIN, Vec3::X * width, Vec3::Y * height, false),
        (
            Vec3::new(MAX.x, MIN.y, MAX.z),
            -Vec3::X * width,
            Vec3::Y * height,
            false,
        ),
        (
            Vec3::new(MIN.x, MIN.y, MAX.z),
            -Vec3::Z * depth,
            Vec3::Y * height,
            false,
        ),
        (
            Vec3::new(MAX.x, MIN.y, MIN.z),
            Vec3::Z * depth,
            Vec3::Y * height,
            false,
        ),
    ] {
        scene.add_quad(origin, u, v, grout);
        mosaic(scene, origin, u, v, &blues, &golds, motif, &mut rng);
    }
    // Each submerged tread is tiled; the shallow landing is at the house end.
    for i in 0..4 {
        let y = -0.22 - i as f32 * 0.30;
        let z = MIN.z + (i + 1) as f32 * 0.38;
        scene.add_box(
            Vec3::new(1.1, MIN.y - 0.03, MIN.z),
            Vec3::new(MAX.x - 0.01, y, z),
            grout,
        );
        mosaic(
            scene,
            Vec3::new(1.1, y, MIN.z + i as f32 * 0.38),
            Vec3::Z * 0.38,
            Vec3::X * 1.89,
            &blues,
            &golds,
            false,
            &mut rng,
        );
        mosaic(
            scene,
            Vec3::new(1.1, y - 0.30, z),
            Vec3::X * 1.89,
            Vec3::Y * 0.30,
            &blues,
            &golds,
            false,
            &mut rng,
        );
    }
    // Coping joints continue the terrace grid and conceal the water's side faces.
    for i in 0..13 {
        let x = MIN.x - 0.5 + i as f32;
        for z in [MIN.z - 0.50, MAX.z] {
            scene.rounded_box(
                Vec3::new(x + 0.003, 0.08, z),
                Vec3::new(x + 0.997, 0.19, z + 0.50),
                0.012,
                coping,
            );
        }
    }
    for i in 0..6 {
        let z = MIN.z + i as f32;
        for x in [MIN.x - 0.50, MAX.x] {
            scene.rounded_box(
                Vec3::new(x, 0.08, z + 0.003),
                Vec3::new(x + 0.50, 0.19, z + 0.997),
                0.012,
                coping,
            );
        }
    }
    let water = scene.add_material(Material::glass(1.333, Vec3::new(0.22, 0.045, 0.018)));
    scene.add_wind_water(
        MIN - Vec3::Y * 0.02,
        MAX,
        water,
        Waves {
            amplitude: 0.024,
            wavelength: 0.70,
            time: 2.3,
            resolution: 384,
        },
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn mosaic(
    scene: &mut Scene,
    origin: Vec3,
    u: Vec3,
    v: Vec3,
    blues: &[usize],
    golds: &[usize],
    motif: bool,
    rng: &mut Rng,
) {
    let nu = (u.length() / PITCH).round().max(1.0) as u32;
    let nv = (v.length() / PITCH).round().max(1.0) as u32;
    let du = u / nu as f32;
    let dv = v / nv as f32;
    let normal = u.cross(v).normalize();
    for j in 0..nv {
        for i in 0..nu {
            let center = origin + du * (i as f32 + 0.5) + dv * (j as f32 + 0.5);
            let gold =
                motif && dolphin(Vec2::new((center.x + 3.1) / 1.65, (center.z - 6.1) / 1.65));
            let palette = if gold { golds } else { blues };
            let material = palette[(rng.uniform() * palette.len() as f32) as usize % palette.len()];
            // The grout is recessed 3–4 mm; a 0.7 mm bevel catches glints.
            let top = center + normal * (0.003 + rng.uniform() * 0.001);
            let a = du * 0.465;
            let b = dv * 0.465;
            let inner_a = a - du.normalize() * 0.0007;
            let inner_b = b - dv.normalize() * 0.0007;
            scene.add_quad(
                top - inner_a - inner_b,
                inner_a * 2.0,
                inner_b * 2.0,
                material,
            );
            let outer = [
                center - a - b,
                center + a - b,
                center + a + b,
                center - a + b,
            ];
            let inner = [
                top - inner_a - inner_b,
                top + inner_a - inner_b,
                top + inner_a + inner_b,
                top - inner_a + inner_b,
            ];
            for k in 0..4 {
                let n = (k + 1) % 4;
                for points in [
                    [outer[k], outer[n], inner[n]],
                    [outer[k], inner[n], inner[k]],
                ] {
                    scene.primitives.push(Primitive::triangle(
                        points,
                        None,
                        [Vec2::ZERO; 3],
                        material,
                    ));
                }
            }
        }
    }
}

/// Original dolphin silhouette: arched back, beak, dorsal/pectoral fins and tail flukes.
fn dolphin(p: Vec2) -> bool {
    const OUTLINE: &[[f32; 2]] = &[
        [-1.57, 0.09],
        [-1.46, 0.03],
        [-1.30, 0.01],
        [-1.18, -0.19],
        [-1.00, -0.36],
        [-0.75, -0.49],
        [-0.44, -0.55],
        [-0.18, -0.55],
        [0.00, -0.73],
        [0.26, -0.88],
        [0.19, -0.49],
        [0.49, -0.35],
        [0.72, -0.13],
        [0.91, 0.15],
        [1.10, 0.43],
        [1.25, 0.43],
        [1.53, 0.28],
        [1.45, 0.51],
        [1.26, 0.64],
        [1.38, 0.86],
        [1.14, 0.77],
        [1.02, 0.57],
        [0.79, 0.38],
        [0.52, 0.23],
        [0.20, 0.11],
        [-0.10, 0.08],
        [-0.01, 0.40],
        [-0.25, 0.29],
        [-0.46, 0.06],
        [-0.76, -0.01],
        [-1.04, -0.02],
        [-1.27, 0.10],
    ];
    let mut inside = false;
    let mut previous = Vec2::from_array(OUTLINE[OUTLINE.len() - 1]);
    for &point in OUTLINE {
        let next = Vec2::from_array(point);
        if (next.y > p.y) != (previous.y > p.y)
            && p.x < (previous.x - next.x) * (p.y - next.y) / (previous.y - next.y) + next.x
        {
            inside = !inside;
        }
        previous = next;
    }
    // A dark mosaic eye remains part of the blue tesserae, not a painted decal.
    inside && p.distance(Vec2::new(-1.15, -0.12)) > 0.035
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dolphin_mosaic_has_body_fins_and_clear_surround() {
        assert!(dolphin(Vec2::new(-0.6, -0.2)));
        assert!(dolphin(Vec2::new(0.1, -0.65)));
        assert!(!dolphin(Vec2::new(-1.15, -0.12)));
        assert!(!dolphin(Vec2::new(0.0, 1.1)));
    }
}
