//! Built-in demonstration scenes; bundled assets require no runtime downloads.
use super::*;

pub fn preset(name: &str, aspect: f32) -> Result<Scene> {
    let mut s = match name {
        "studio" => studio(aspect)?,
        "interior" => interior(aspect)?,
        "outdoor" => outdoor(aspect)?,
        "clouds" => clouds(aspect)?,
        _ => bail!("Unknown scene: {name}. Choose studio, interior, outdoor, or clouds"),
    };
    s.finish()?;
    Ok(s)
}
fn asset() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/vessel.obj")
}
fn studio(aspect: f32) -> Result<Scene> {
    let from = Vec3::new(5.8, 3.5, 8.0);
    let to = Vec3::new(0.0, 1.1, 0.0);
    let mut s = Scene::empty(Camera::new(
        from,
        to,
        30.0,
        aspect,
        0.035,
        from.distance(to),
    ));
    s.sky = Vec3::splat(0.025);
    s.horizon = s.sky;
    let floor = s.add_material(Material::matte(Vec3::new(0.34, 0.28, 0.23)).patterned(1, 3.0));
    let stone = s.add_material(Material::matte(Vec3::new(0.72, 0.70, 0.65)).patterned(1, 4.0));
    let gold = s.add_material(Material::metal(Vec3::new(0.88, 0.61, 0.26), 0.22));
    let glass = s.add_material(Material::glass(1.5, Vec3::new(0.12, 0.025, 0.055)));
    let ceramic = s.add_material(Material {
        surface: [0.19, 0.0, 0.0, 1.5],
        ..Material::matte(Vec3::new(0.055, 0.19, 0.16))
    });
    s.add_quad(
        Vec3::new(-100.0, 0.0, -100.0),
        Vec3::Z * 200.0,
        Vec3::X * 200.0,
        floor,
    );
    s.add_quad(
        Vec3::new(-40.0, 0.0, -12.0),
        Vec3::X * 80.0,
        Vec3::Y * 30.0,
        floor,
    );
    s.rounded_box(
        Vec3::new(-2.3, 0.0, -1.2),
        Vec3::new(2.3, 0.35, 1.2),
        0.04,
        stone,
    );
    s.add_sphere(Vec3::new(-1.25, 1.22, 0.15), 0.87, glass);
    s.add_sphere(Vec3::new(1.25, 0.97, 0.45), 0.62, gold);
    s.import_obj(
        &asset(),
        Vec3::new(0.2, 0.35, -0.38),
        1.3,
        0.0,
        Some(ceramic),
    )?;
    let key = s.add_material(Material::light(Vec3::new(15.0, 13.5, 11.5)));
    let fill = s.add_material(Material::light(Vec3::new(10.0, 13.0, 17.0)));
    s.add_quad(
        Vec3::new(-3.5, 6.0, -2.0),
        Vec3::X * 4.0,
        Vec3::Z * 3.0,
        key,
    );
    s.add_quad(
        Vec3::new(7.0, 1.0, -2.0),
        Vec3::Y * 4.0,
        Vec3::Z * 3.0,
        fill,
    );
    let strip = s.add_material(Material::light(Vec3::splat(7.0)));
    s.add_quad(
        Vec3::new(-6.0, 0.7, 1.0),
        Vec3::Y * 3.0,
        Vec3::Z * 1.0,
        strip,
    );
    Ok(s)
}
fn interior(aspect: f32) -> Result<Scene> {
    super::apartment::build(aspect)
}
fn outdoor(aspect: f32) -> Result<Scene> {
    let from = Vec3::new(8.5, 4.8, 11.5);
    let to = Vec3::new(0.0, 1.3, 0.0);
    let mut s = Scene::empty(Camera::new(from, to, 40.0, aspect, 0.0, from.distance(to)));
    s.sky = Vec3::new(0.18, 0.32, 0.55);
    s.horizon = Vec3::new(0.65, 0.66, 0.65);
    let ground = s.add_material(Material::matte(Vec3::new(0.30, 0.32, 0.23)).patterned(1, 6.0));
    let wall = s.add_material(Material::matte(Vec3::new(0.72, 0.66, 0.53)).patterned(1, 2.0));
    let wood = s.add_material(Material::matte(Vec3::new(0.27, 0.12, 0.045)).patterned(2, 8.0));
    let stone = s.add_material(Material::matte(Vec3::splat(0.36)).patterned(1, 4.0));
    let water = s.add_material(Material::glass(1.333, Vec3::new(0.45, 0.08, 0.035)));
    s.add_quad(
        Vec3::new(-200.0, -0.15, -200.0),
        Vec3::Z * 400.0,
        Vec3::X * 400.0,
        ground,
    );
    s.add_box(Vec3::new(-4.8, -0.1, -3.5), Vec3::new(4.8, 0.18, 3.5), wall);
    s.add_box(Vec3::new(-4.8, 0.18, -3.5), Vec3::new(4.8, 3.7, -3.2), wall);
    s.add_box(Vec3::new(-4.8, 0.18, -3.2), Vec3::new(-4.5, 3.7, 1.0), wall);
    s.add_box(
        Vec3::new(-3.8, 0.18, -2.6),
        Vec3::new(-3.6, 4.0, -2.4),
        wood,
    );
    s.add_box(Vec3::new(3.6, 0.18, -2.6), Vec3::new(3.8, 4.0, -2.4), wood);
    s.add_box(Vec3::new(-3.8, 0.18, 2.4), Vec3::new(-3.6, 4.0, 2.6), wood);
    s.add_box(Vec3::new(3.6, 0.18, 2.4), Vec3::new(3.8, 4.0, 2.6), wood);
    for i in 0..12 {
        let z = -2.5 + i as f32 * 0.45;
        s.add_box(
            Vec3::new(-4.0, 4.0, z),
            Vec3::new(4.0, 4.17, z + 0.17),
            wood,
        );
    }
    s.add_box(
        Vec3::new(-2.5, 0.18, -1.1),
        Vec3::new(2.5, 0.35, 1.75),
        stone,
    );
    s.add_water(
        Vec3::new(-2.3, 0.35, -0.9),
        Vec3::new(2.3, 0.65, 1.55),
        water,
        Waves::default(),
    )?;
    s.add_box(
        Vec3::new(-2.6, 0.18, -2.8),
        Vec3::new(1.5, 0.7, -2.05),
        wood,
    );
    let leaf = s.add_material(Material::matte(Vec3::new(0.12, 0.22, 0.055)));
    let pot = s.add_material(Material::matte(Vec3::new(0.38, 0.17, 0.09)));
    s.import_obj(&asset(), Vec3::new(3.0, 0.18, -1.6), 0.72, 0.0, Some(pot))?;
    s.add_box(
        Vec3::new(2.96, 0.6, -1.64),
        Vec3::new(3.04, 2.5, -1.56),
        wood,
    );
    // Small individual leaves make the canopy silhouette read as foliage rather than spheres.
    let pale_leaf = s.add_material(Material::matte(Vec3::new(0.22, 0.29, 0.09)));
    let mut foliage_rng = crate::rng::Rng::new(17, 0, 42);
    for index in 0..1400 {
        let distribution = foliage_rng.sphere() * foliage_rng.uniform().cbrt();
        let center = Vec3::new(3.0, 2.0, -1.6) + distribution * Vec3::new(0.85, 0.75, 0.65);
        let normal = (foliage_rng.sphere() + Vec3::Y * 0.5).normalize();
        let (tangent, bitangent) = crate::rng::basis(normal);
        let length = 0.06 + foliage_rng.uniform() * 0.06;
        let width = length * 0.3;
        let points = [
            center - tangent * length,
            center - bitangent * width,
            center + tangent * length,
            center + bitangent * width,
        ];
        let material = if index % 3 == 0 { pale_leaf } else { leaf };
        for indices in [[0, 1, 2], [0, 2, 3]] {
            s.primitives.push(Primitive::triangle(
                indices.map(|i| points[i]),
                Some([normal; 3]),
                [Vec2::ZERO; 3],
                material,
            ));
        }
    }
    // A stone rim conceals the water volume's side faces and leaves the surface visible.
    for (min, max) in [
        (Vec3::new(-2.5, 0.35, -1.1), Vec3::new(-2.3, 0.73, 1.75)),
        (Vec3::new(2.3, 0.35, -1.1), Vec3::new(2.5, 0.73, 1.75)),
        (Vec3::new(-2.3, 0.35, -1.1), Vec3::new(2.3, 0.73, -0.9)),
        (Vec3::new(-2.3, 0.35, 1.55), Vec3::new(2.3, 0.73, 1.75)),
    ] {
        s.rounded_box(min, max, 0.025, stone);
    }
    let sun = s.add_material(Material::light(Vec3::new(110.0, 83.0, 52.0)));
    s.add_sphere(Vec3::new(-15.0, 22.0, 8.0), 2.0, sun);
    Ok(s)
}

fn clouds(aspect: f32) -> Result<Scene> {
    let from = Vec3::new(22.0, 8.0, 42.0);
    let target = Vec3::new(0.0, 8.0, -4.0);
    let mut scene = Scene::empty(Camera::new(
        from,
        target,
        43.0,
        aspect,
        0.0,
        from.distance(target),
    ));
    scene.sky = Vec3::new(0.2, 0.4, 0.8);
    scene.horizon = Vec3::new(0.75, 0.82, 0.95);
    let ground = scene.add_material(Material::matte(Vec3::new(0.16, 0.21, 0.10)));
    scene.add_quad(
        Vec3::new(-500.0, -2.0, -500.0),
        Vec3::Z * 1000.0,
        Vec3::X * 1000.0,
        ground,
    );
    let sun = scene.add_material(Material::light(Vec3::new(110.0, 99.0, 82.0)));
    scene.add_sphere(Vec3::new(-25.0, 38.0, 12.0), 3.0, sun);
    scene.medium = Some(crate::medium::Medium::new(
        Vec3::new(-15.0, 3.0, -15.0),
        Vec3::new(15.0, 15.0, 5.0),
        1.5,
        Vec3::splat(0.985),
        0.55,
        0.30,
    )?);
    Ok(scene)
}
