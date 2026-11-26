//! Strict JSON scene descriptions, validated before BVH construction.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneFile {
    pub camera: CameraDef,
    #[serde(default = "default_sky")]
    pub sky: [f32; 3],
    #[serde(default = "default_sky")]
    pub horizon: [f32; 3],
    pub materials: Vec<MaterialDef>,
    pub objects: Vec<ObjectDef>,
    #[serde(default)]
    pub medium: Option<MediumDef>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediumDef {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub extinction: f32,
    pub albedo: [f32; 3],
    pub anisotropy: f32,
    #[serde(default)]
    pub noise_scale: f32,
}
fn default_sky() -> [f32; 3] {
    [0.05; 3]
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraDef {
    pub from: [f32; 3],
    pub target: [f32; 3],
    #[serde(default = "default_fov")]
    pub fov: f32,
    #[serde(default)]
    pub aperture: f32,
    #[serde(default)]
    pub focus: Option<f32>,
}
fn default_fov() -> f32 {
    40.0
}
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MaterialDef {
    pub color: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub transmission: f32,
    pub ior: f32,
    pub absorption: [f32; 3],
    pub emission: [f32; 3],
    pub pattern: u32,
    pub scale: f32,
    pub texture: Option<String>,
    pub texture_scale: f32,
    pub texture_strength: f32,
}
impl Default for MaterialDef {
    fn default() -> Self {
        Self {
            color: [0.7; 3],
            roughness: 0.5,
            metallic: 0.0,
            transmission: 0.0,
            ior: 1.5,
            absorption: [0.0; 3],
            emission: [0.0; 3],
            pattern: 0,
            scale: 1.0,
            texture: None,
            texture_scale: 1.0,
            texture_strength: 1.0,
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectDef {
    Sphere {
        center: [f32; 3],
        radius: f32,
        material: usize,
    },
    Quad {
        origin: [f32; 3],
        u: [f32; 3],
        v: [f32; 3],
        material: usize,
    },
    Box {
        min: [f32; 3],
        max: [f32; 3],
        material: usize,
    },
    Water {
        min: [f32; 3],
        max: [f32; 3],
        material: usize,
        #[serde(default)]
        waves: Waves,
    },
    Mesh {
        path: String,
        #[serde(default)]
        position: [f32; 3],
        #[serde(default = "one")]
        scale: f32,
        #[serde(default)]
        rotation: f32,
        material: Option<usize>,
    },
}
fn one() -> f32 {
    1.0
}
pub fn load(path: &Path, aspect: f32) -> Result<Scene> {
    let f: SceneFile =
        serde_json::from_str(&std::fs::read_to_string(path)?).context("Invalid scene JSON")?;
    let from = Vec3::from(f.camera.from);
    let to = Vec3::from(f.camera.target);
    let focus = f.camera.focus.unwrap_or(from.distance(to));
    ensure!(
        from.is_finite()
            && to.is_finite()
            && from.distance(to) > 1e-5
            && f.camera.fov > 0.0
            && f.camera.fov < 179.0
            && focus.is_finite()
            && focus > 0.0
            && f.camera.aperture.is_finite()
            && f.camera.aperture >= 0.0,
        "Invalid camera"
    );
    let mut s = Scene::empty(Camera::new(
        from,
        to,
        f.camera.fov,
        aspect,
        f.camera.aperture,
        focus,
    ));
    s.sky = Vec3::from(f.sky);
    s.horizon = Vec3::from(f.horizon);
    ensure!(
        s.sky.min_element() >= 0.0 && s.horizon.min_element() >= 0.0,
        "Environment radiance cannot be negative"
    );
    for m in f.materials {
        ensure!(
            (0.0..=1.0).contains(&m.roughness)
                && (0.0..=1.0).contains(&m.metallic)
                && (m.transmission == 0.0 || m.transmission == 1.0)
                && m.ior >= 1.0
                && Vec3::from(m.color).min_element() >= 0.0
                && Vec3::from(m.color).max_element() <= 1.0
                && Vec3::from(m.absorption).min_element() >= 0.0
                && Vec3::from(m.emission).min_element() >= 0.0,
            "Invalid material parameters (transmission must be 0 or 1)"
        );
        let mut material = Material {
            base: crate::geometry::pack_vec3(Vec3::from(m.color), 0.0),
            surface: [m.roughness, m.metallic, m.transmission, m.ior],
            emission: crate::geometry::pack_vec3(Vec3::from(m.emission), 0.0),
            absorption: crate::geometry::pack_vec3(Vec3::from(m.absorption), 0.0),
            detail: [m.scale, m.pattern as f32, 0.0, 0.0],
        };
        if let Some(texture) = m.texture {
            ensure!(m.texture_scale.is_finite(), "Texture scale must be finite");
            let index = s
                .textures
                .load(&path.parent().unwrap_or(Path::new(".")).join(texture))?;
            ensure!(
                (0.0..=1.0).contains(&m.texture_strength),
                "Texture strength must be in [0, 1]"
            );
            material = material
                .textured(index, m.texture_scale)
                .texture_strength(m.texture_strength);
        }
        s.materials.push(material);
    }
    let count = s.materials.len();
    for o in f.objects {
        let mat = match &o {
            ObjectDef::Sphere { material, .. }
            | ObjectDef::Quad { material, .. }
            | ObjectDef::Water { material, .. }
            | ObjectDef::Box { material, .. } => Some(*material),
            ObjectDef::Mesh { material, .. } => *material,
        };
        if let Some(m) = mat {
            ensure!(m < count, "Invalid material index: {m}");
        }
        match o {
            ObjectDef::Water {
                min,
                max,
                material,
                waves,
            } => {
                s.add_water(min.into(), max.into(), material, waves)?;
            }
            ObjectDef::Sphere {
                center,
                radius,
                material,
            } => {
                ensure!(radius > 0.0, "Sphere radius must be positive");
                s.add_sphere(center.into(), radius, material)
            }
            ObjectDef::Quad {
                origin,
                u,
                v,
                material,
            } => s.add_quad(origin.into(), u.into(), v.into(), material),
            ObjectDef::Box { min, max, material } => {
                ensure!(
                    (Vec3::from(max) - Vec3::from(min)).min_element() > 0.0,
                    "Invalid box dimensions"
                );
                s.add_box(min.into(), max.into(), material)
            }
            ObjectDef::Mesh {
                path: p,
                position,
                scale,
                rotation,
                material,
            } => {
                s.import_obj(
                    &path.parent().unwrap_or(Path::new(".")).join(p),
                    position.into(),
                    scale,
                    rotation,
                    material,
                )?;
            }
        }
    }
    if let Some(medium) = f.medium {
        s.medium = Some(crate::medium::Medium::new(
            medium.min.into(),
            medium.max.into(),
            medium.extinction,
            medium.albedo.into(),
            medium.anisotropy,
            medium.noise_scale,
        )?);
    }
    s.finish()?;
    Ok(s)
}
