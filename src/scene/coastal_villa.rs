//! A furnished coastal villa with a tiled pool and licensed, full-resolution source meshes.
mod pool;
use super::*;
use crate::{geometry::pack_vec3, rng::Rng};
use std::collections::HashMap;
use std::f32::consts::TAU;

pub(super) fn build(aspect: f32) -> Result<Scene> {
    let from = Vec3::new(15.0, 4.4, 19.0);
    let target = Vec3::new(-1.8, 1.0, -2.0);
    let mut scene = Scene::empty(Camera::new(
        from,
        target,
        48.0,
        aspect,
        0.0,
        from.distance(target),
    ));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    scene.sky = Vec3::new(0.38, 0.56, 0.82);
    scene.horizon = Vec3::new(0.62, 0.72, 0.78);
    scene.environment_map = Some(EnvironmentMap {
        texture: scene
            .textures
            .load_data(&root.join("environments/kloofendal_48d_partly_cloudy_puresky.hdr"))?,
        intensity: 0.85,
        rotation: 1.3,
        horizon_fade: 0.16,
    });
    let sandstone = surface(
        &mut scene,
        "white_sandstone_blocks_02",
        0.38,
        Vec3::splat(0.95),
    )?;
    let plaster = surface(
        &mut scene,
        "white_plaster_02",
        0.75,
        Vec3::new(0.94, 0.92, 0.86),
    )?;
    let paving = surface(
        &mut scene,
        "concrete_floor_worn_001",
        0.5,
        Vec3::new(0.96, 0.88, 0.76),
    )?;
    scene.materials[paving].base[3] = 0.35;
    scene.materials[plaster].base[3] = 0.35;
    let timber = surface(
        &mut scene,
        "oak_wood_planks",
        0.55,
        Vec3::new(0.75, 0.62, 0.46),
    )?;
    let grass = surface(&mut scene, "grass_ground", 0.9, Vec3::new(0.72, 0.85, 0.60))?;
    scene.materials[grass].base = [0.055, 0.105, 0.025, 0.45];
    scene.materials[grass].maps[0] = 0;
    let dark = scene.add_material(Material::metal(Vec3::new(0.055, 0.063, 0.067), 0.30));
    let mut assets = Assets::default();

    // The source includes actual rooms, openings, frames, doors and fittings.
    let first_material = scene.materials.len();
    assets.place(
        &mut scene,
        "coastal_villa",
        "coastal_villa",
        Vec3::new(3.0, 0.12, -9.0),
        1.0,
        0.0,
    )?;
    for (slot, replacement) in [
        (0, plaster),
        (7, plaster),
        (9, sandstone),
        (10, sandstone),
        (1, timber),
        (2, timber),
        (5, timber),
    ] {
        scene.materials[first_material + slot] = scene.materials[replacement];
    }
    terrace(&mut scene, paving, timber, dark);
    pool::build(&mut scene, paving)?;
    assets.place(
        &mut scene,
        "pool_deck_set",
        "pool_deck_set",
        Vec3::new(7.0, 0.16, 5.8),
        1.0,
        -90.0,
    )?;
    assets.place(
        &mut scene,
        "canopy_daybed",
        "canopy_daybed",
        Vec3::new(-5.5, 0.16, 0.6),
        1.0,
        0.0,
    )?;
    assets.place(
        &mut scene,
        "outdoor_table_chair_set_01",
        "outdoor_table_chair_set_01",
        Vec3::new(6.0, 0.16, 0.0),
        1.1,
        32.0,
    )?;
    assets.place(
        &mut scene,
        "tea_set_01",
        "tea_set_01",
        Vec3::new(6.0, 0.88, 0.0),
        0.65,
        0.0,
    )?;
    for (position, scale) in [
        (Vec3::new(-8.7, 0.17, 0.0), 4.8),
        (Vec3::new(10.4, 0.17, 1.1), 4.1),
        (Vec3::new(5.9, 0.88, 0.0), 0.8),
    ] {
        assets.place(
            &mut scene,
            "potted_plant_04",
            "potted_plant_04",
            position,
            scale,
            position.x * 17.0,
        )?;
    }
    landscape(&mut scene, &mut assets, grass, paving)?;
    sea(&mut scene)?;
    // A finite disk-sized emitter supplements the photographed sky; radiance is
    // shared by direct lighting, reflections and refractive paths.
    let sun = scene.add_material(Material::light(Vec3::new(720.0, 610.0, 465.0)));
    scene.add_sphere(Vec3::new(-90.0, 120.0, 80.0), 5.0, sun);
    Ok(scene)
}

fn surface(scene: &mut Scene, name: &str, scale: f32, color: Vec3) -> Result<usize> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/textures/coastal")
        .join(name);
    let texture = scene.textures.load(&folder.join("diff.jpg"))?;
    let mut material = Material::matte(color).textured(texture, -scale);
    material.surface[0] = 1.0;
    material.maps[0] = scene.textures.load_data(&folder.join("nor_gl.jpg"))? as u32 + 1;
    material.maps[1] = scene.textures.load_data(&folder.join("rough.jpg"))? as u32 + 1;
    Ok(scene.add_material(material))
}

fn terrace(scene: &mut Scene, stone: usize, timber: usize, metal: usize) {
    // Continuous foundations stay below the basin. No hidden slab fills the pool.
    scene.add_box(
        Vec3::new(-11.4, -3.1, -19.0),
        Vec3::new(13.0, -1.50, 12.0),
        stone,
    );
    for (min, max) in [
        (
            Vec3::new(-11.0, -1.5, -19.0),
            Vec3::new(pool::MIN.x, 0.13, 12.0),
        ),
        (
            Vec3::new(pool::MAX.x, -1.5, -19.0),
            Vec3::new(13.0, 0.13, 12.0),
        ),
        (
            Vec3::new(pool::MIN.x, -1.5, -19.0),
            Vec3::new(pool::MAX.x, 0.13, pool::MIN.z),
        ),
        (
            Vec3::new(pool::MIN.x, -1.5, pool::MAX.z),
            Vec3::new(pool::MAX.x, 0.13, 12.0),
        ),
    ] {
        scene.add_box(min, max, stone);
    }
    for iz in 0..31 {
        for ix in 0..24 {
            let x = -11.0 + ix as f32;
            let z = -19.0 + iz as f32;
            if x + 1.0 > pool::MIN.x - 0.5
                && x < pool::MAX.x + 0.5
                && z + 1.0 > pool::MIN.z - 0.5
                && z < pool::MAX.z + 0.5
            {
                continue;
            }
            scene.rounded_box(
                Vec3::new(x + 0.003, -0.06, z + 0.003),
                Vec3::new(x + 0.997, 0.15, z + 0.997),
                0.006,
                stone,
            );
        }
    }
    // Cedar soffit strips, shadow gaps and a slim fascia add construction detail
    // to the existing covered terrace without filling the source's glazed openings.
    for i in 0..145 {
        let x = -5.8 + i as f32 * 0.12;
        scene.rounded_box(
            Vec3::new(x, 3.13, -1.30),
            Vec3::new(x + 0.095, 3.17, 1.20),
            0.003,
            timber,
        );
    }
    scene.rounded_box(
        Vec3::new(-5.9, 3.17, -1.35),
        Vec3::new(11.6, 3.35, 1.30),
        0.014,
        stone,
    );
    for x in [-5.75, 2.8, 11.45] {
        scene.rounded_box(
            Vec3::new(x, 0.15, 1.10),
            Vec3::new(x + 0.065, 3.17, 1.165),
            0.006,
            metal,
        );
    }
    // Timber screening alongside the service wing and concealed warm entry lights.
    for i in 0..33 {
        let z = -8.0 + i as f32 * 0.16;
        scene.rounded_box(
            Vec3::new(12.15, 0.30, z),
            Vec3::new(12.24, 3.20, z + 0.085),
            0.005,
            timber,
        );
    }
}

fn landscape(scene: &mut Scene, assets: &mut Assets, grass: usize, stone: usize) -> Result<()> {
    scene.add_box(
        Vec3::new(13.0, -3.0, -2000.0),
        Vec3::new(2000.0, 0.04, 2000.0),
        grass,
    );
    scene.add_box(
        Vec3::new(-11.0, -3.0, 12.0),
        Vec3::new(13.0, 0.04, 2000.0),
        grass,
    );
    scene.add_box(
        Vec3::new(-11.0, -3.0, -2000.0),
        Vec3::new(13.0, 0.04, -19.0),
        grass,
    );
    let mut rng = Rng::new(7281, 0, 42);
    for _ in 0..200_000 {
        let x = -10.9 + rng.uniform() * 28.5;
        let z = -4.0 + rng.uniform() * 24.0;
        if x < 13.02 && z < 12.02 {
            continue;
        }
        let variant = ["small_a", "small_c", "medium_a"][(rng.uniform() * 3.0) as usize % 3];
        let scale = 0.85 + rng.uniform() * 0.65;
        assets.place(
            scene,
            "grass_bermuda_01",
            variant,
            Vec3::new(x, 0.035, z),
            scale,
            rng.uniform() * 360.0,
        )?;
    }
    let soil =
        scene.add_material(Material::matte(Vec3::new(0.055, 0.035, 0.019)).patterned(1, 18.0));
    for (position, scale) in [
        (Vec3::new(15.1, 0.04, -4.5), 1.8),
        (Vec3::new(-8.6, 0.15, -6.0), 2.1),
        (Vec3::new(-9.0, 0.15, -15.5), 1.7),
        (Vec3::new(18.5, 0.04, 7.8), 1.5),
    ] {
        if position.x < 0.0 {
            // Trees on the paved terrace sit in raised mineral planters with soil.
            let min = position - Vec3::new(0.78, 0.0, 0.78);
            let max = position + Vec3::new(0.78, 0.48, 0.78);
            for (a, b) in [
                (min, Vec3::new(min.x + 0.08, max.y, max.z)),
                (Vec3::new(max.x - 0.08, min.y, min.z), max),
                (
                    Vec3::new(min.x + 0.08, min.y, min.z),
                    Vec3::new(max.x - 0.08, max.y, min.z + 0.08),
                ),
                (
                    Vec3::new(min.x + 0.08, min.y, max.z - 0.08),
                    Vec3::new(max.x - 0.08, max.y, max.z),
                ),
            ] {
                scene.rounded_box(a, b, 0.008, stone);
            }
            scene.add_box(
                min + Vec3::new(0.08, 0.0, 0.08),
                max - Vec3::new(0.08, 0.06, 0.08),
                soil,
            );
        }
        assets.place(
            scene,
            "quiver_tree_01",
            "quiver_tree_01",
            position,
            scale,
            position.x * 21.0,
        )?;
    }
    for i in 0..28 {
        let position = if i < 18 {
            Vec3::new(14.1 + rng.uniform() * 2.0, 0.04, -16.0 + i as f32 * 0.85)
        } else {
            Vec3::new(-9.7, 0.15, -17.0 + (i - 18) as f32 * 1.4)
        };
        assets.place(
            scene,
            "shrub_04",
            "shrub_04",
            position,
            2.2 + rng.uniform(),
            rng.uniform() * 360.0,
        )?;
    }
    // One complete photogrammetry coastline retains 756,940 source triangles.
    assets.place(
        scene,
        "coast_line_02",
        "coast_line_02",
        Vec3::new(-25.0, -4.3, -3.0),
        1.0,
        90.0,
    )?;
    for i in 0..8 {
        let z = 11.7 + i as f32 * 0.9;
        scene.rounded_box(
            Vec3::new(6.7, 0.045, z),
            Vec3::new(8.1, 0.13, z + 0.60),
            0.012,
            stone,
        );
    }
    Ok(())
}

fn sea(scene: &mut Scene) -> Result<()> {
    let water = scene.add_material(Material::glass(1.333, Vec3::new(0.32, 0.065, 0.028)));
    scene.add_wind_water_patch(
        Vec3::new(-180.0, -35.0, -200.0),
        Vec3::new(-11.5, -3.05, 200.0),
        water,
        Waves {
            amplitude: 0.23,
            wavelength: 7.5,
            time: 5.1,
            resolution: 512,
        },
    )?;
    // The distant surface continues beyond the near wave patch to the horizon.
    for (min, max) in [
        (
            Vec3::new(-8000.0, -35.0, -8000.0),
            Vec3::new(-180.0, -3.05, 8000.0),
        ),
        (
            Vec3::new(-180.0, -35.0, -8000.0),
            Vec3::new(-11.5, -3.05, -200.0),
        ),
        (
            Vec3::new(-180.0, -35.0, 200.0),
            Vec3::new(-11.5, -3.05, 8000.0),
        ),
    ] {
        scene.add_quad(
            Vec3::new(min.x, max.y, min.z),
            Vec3::Z * (max.z - min.z),
            Vec3::X * (max.x - min.x),
            water,
        );
    }
    // One volume avoids artificial refraction at adjacent patch boundaries.
    scene.add_water_boundary(
        Vec3::new(-8000.0, -35.0, -8000.0),
        Vec3::new(-11.5, -3.05, 8000.0),
        water,
    );
    Ok(())
}

/// Reuse imported material/texture records when placing repeated full-detail meshes.
/// Geometry is transformed exactly once per placement, with no distance-based LOD.
#[derive(Default)]
struct Assets {
    meshes: HashMap<String, Vec<Primitive>>,
}
impl Assets {
    fn place(
        &mut self,
        scene: &mut Scene,
        folder: &str,
        name: &str,
        position: Vec3,
        scale: f32,
        angle: f32,
    ) -> Result<()> {
        let key = format!("{folder}/{name}");
        if !self.meshes.contains_key(&key) {
            let start = scene.primitives.len();
            let shared_grass = if folder == "grass_bermuda_01" {
                self.meshes
                    .iter()
                    .find(|(key, _)| key.starts_with("grass_bermuda_01/"))
                    .map(|(_, mesh)| mesh[0].info[1] as usize)
            } else {
                None
            };
            scene.import_obj(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets/models")
                    .join(folder)
                    .join(format!("{name}.obj")),
                Vec3::ZERO,
                1.0,
                0.0,
                shared_grass,
            )?;
            self.meshes
                .insert(key.clone(), scene.primitives.split_off(start));
        }
        let rotation = Mat3::from_rotation_y(angle * TAU / 360.0);
        for original in &self.meshes[&key] {
            let mut primitive = *original;
            primitive.p0 = pack_vec3(
                position + rotation * unpack_vec3(original.p0) * scale,
                original.p0[3] * scale,
            );
            primitive.p1 = pack_vec3(rotation * unpack_vec3(original.p1) * scale, 0.0);
            primitive.p2 = pack_vec3(rotation * unpack_vec3(original.p2) * scale, 0.0);
            primitive.n0 = pack_vec3(rotation * unpack_vec3(original.n0), 0.0);
            primitive.n1 = pack_vec3(rotation * unpack_vec3(original.n1), 0.0);
            primitive.n2 = pack_vec3(rotation * unpack_vec3(original.n2), 0.0);
            scene.primitives.push(primitive);
        }
        Ok(())
    }
}
