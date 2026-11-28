//! A glazed incense dish, a smouldering stick and a bounded volumetric plume.
use super::*;
use crate::medium::Medium;

pub(super) fn build(scene: &mut Scene) -> Result<()> {
    let ceramic = scene.add_material(Material {
        surface: [0.22, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.035, 0.065, 0.075))
    });
    let brass = scene.add_material(Material::metal(Vec3::new(0.65, 0.43, 0.16), 0.24));
    let bamboo = scene.add_material(Material::matte(Vec3::new(0.48, 0.29, 0.12)));
    let coating = scene.add_material(Material::matte(Vec3::new(0.15, 0.075, 0.035)));
    let ash = scene.add_material(Material::matte(Vec3::new(0.36, 0.34, 0.30)));
    let ember = scene.add_material(Material::light(Vec3::new(8.0, 1.2, 0.16)));
    let center = Vec3::new(3.88, 0.6945, -1.16);
    dish(scene, center, ceramic);

    let bottom = Vec3::new(3.88, 0.704, -1.27);
    let tip = Vec3::new(3.87, 0.993, -1.10);
    let direction = (tip - bottom).normalize();
    cylinder(scene, bottom - Vec3::Y * 0.004, 0.005, 0.010, brass);
    details::tube(
        scene,
        &[bottom, bottom + direction * 0.055],
        0.0011,
        bamboo,
        12,
    );
    details::tube(
        scene,
        &[bottom + direction * 0.05, tip - direction * 0.006],
        0.0016,
        coating,
        16,
    );
    details::tube(scene, &[tip - direction * 0.006, tip], 0.00165, ash, 16);
    scene.add_sphere(tip, 0.00145, ember);
    let mut rng = Rng::new(74, 0, 42);
    for _ in 0..18 {
        scene.add_sphere(
            center
                + Vec3::new(
                    (rng.uniform() - 0.5) * 0.020,
                    0.005,
                    (rng.uniform() - 0.5) * 0.15,
                ),
            0.0004 + rng.uniform() * 0.0005,
            ash,
        );
    }
    // Keep the participating medium local to the stick; the room remains clear.
    scene.medium = Some(Medium::incense(tip + Vec3::Y * 0.0022, 0.78, 75.0)?);
    Ok(())
}

fn dish(scene: &mut Scene, center: Vec3, material: usize) {
    let first = scene.primitives.len();
    lathe_segments(
        scene,
        Vec3::ZERO,
        &[
            (0.0, 0.0),
            (0.11, 0.0),
            (0.135, 0.006),
            (0.14, 0.011),
            (0.133, 0.014),
            (0.12, 0.006),
            (0.0, 0.004),
        ],
        material,
        96,
    );
    let scale = Vec3::new(0.36, 1.0, 1.0);
    for primitive in &mut scene.primitives[first..] {
        primitive.p0 = pack_vec3(center + unpack_vec3(primitive.p0) * scale, 0.0);
        primitive.p1 = pack_vec3(unpack_vec3(primitive.p1) * scale, 0.0);
        primitive.p2 = pack_vec3(unpack_vec3(primitive.p2) * scale, 0.0);
        for normal in [&mut primitive.n0, &mut primitive.n1, &mut primitive.n2] {
            *normal = pack_vec3((unpack_vec3(*normal) / scale).normalize(), 0.0);
        }
    }
}
