//! Scene ownership, geometry assembly and OBJ / MTL import.
mod apartment;
mod file;
mod presets;
mod water;
pub use file::load;
pub use presets::{PRESET_NAMES, preset};
pub use water::Waves;

use crate::{
    bvh::Bvh,
    camera::Camera,
    geometry::{Primitive, unpack_vec3},
    material::Material,
    rng::luminance,
};
use anyhow::{Context, Result, bail, ensure};
use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Vec2, Vec3};
use serde::Deserialize;
use std::path::Path;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Light {
    pub primitive: u32,
    pub pad: u32,
    pub area: f32,
    pub cdf: f32,
}
pub struct Scene {
    pub medium: Option<crate::medium::Medium>,
    pub textures: crate::texture::TextureAtlas,
    pub primitives: Vec<Primitive>,
    pub materials: Vec<Material>,
    pub bvh: Bvh,
    pub lights: Vec<Light>,
    pub camera: Camera,
    pub sky: Vec3,
    pub horizon: Vec3,
}
impl Scene {
    fn empty(camera: Camera) -> Self {
        Self {
            medium: None,
            textures: crate::texture::TextureAtlas::default(),
            primitives: vec![],
            materials: vec![],
            bvh: Bvh::default(),
            lights: vec![],
            camera,
            sky: Vec3::splat(0.02),
            horizon: Vec3::splat(0.02),
        }
    }
    pub fn finish(&mut self) -> Result<()> {
        ensure!(
            !self.primitives.is_empty(),
            "The scene contains no primitives"
        );
        ensure!(
            self.primitives.len() <= u32::MAX as usize,
            "Too many primitives"
        );
        ensure!(
            self.sky.is_finite()
                && self.horizon.is_finite()
                && self.sky.min_element() >= 0.0
                && self.horizon.min_element() >= 0.0,
            "Invalid environment radiance"
        );
        for material in &self.materials {
            ensure!(
                !material.is_light() || material.maps[3] == 0,
                "Masked emitters are not supported"
            );
            ensure!(
                material
                    .base
                    .iter()
                    .chain(&material.surface)
                    .chain(&material.emission)
                    .chain(&material.absorption)
                    .chain(&material.detail)
                    .all(|x| x.is_finite()),
                "Material parameters must be finite"
            );
            ensure!(
                unpack_vec3(material.base).min_element() >= 0.0
                    && unpack_vec3(material.base).max_element() <= 1.0
                    && (0.0..=1.0).contains(&material.surface[0])
                    && (0.0..=1.0).contains(&material.surface[1])
                    && matches!(material.surface[2], 0.0 | 1.0 | 2.0)
                    && material.surface[3] >= 1.0
                    && unpack_vec3(material.absorption).min_element() >= 0.0
                    && (0.0..=1.0).contains(&material.absorption[3])
                    && (!material.is_glass() || material.absorption[3] == 0.0)
                    && unpack_vec3(material.emission).min_element() >= 0.0,
                "Invalid material reflectance, transmission or absorption"
            );
            ensure!(
                material.detail[2] >= 0.0
                    && material.detail[2].fract() == 0.0
                    && material.detail[2] as usize <= self.textures.descriptors.len(),
                "Invalid texture index"
            );
        }
        for material in &self.materials {
            ensure!(
                material
                    .maps
                    .iter()
                    .all(|&index| index as usize <= self.textures.descriptors.len()),
                "Invalid surface map index"
            );
        }
        for p in &self.primitives {
            ensure!(
                p.info[0] <= 1 && (p.info[0] != 0 || p.p0[3] > 0.0),
                "Invalid primitive type or radius"
            );
            ensure!(
                p.n0.iter()
                    .chain(&p.n1)
                    .chain(&p.n2)
                    .chain(&p.uv01)
                    .chain(&p.uv2)
                    .all(|x| x.is_finite()),
                "Non-finite vertex attributes"
            );
            ensure!(
                (p.info[1] as usize) < self.materials.len(),
                "Material does not exist"
            );
            let (lo, hi) = p.bounds();
            ensure!(
                lo.is_finite() && hi.is_finite() && p.area().is_finite() && p.area() > 1e-12,
                "Degenerate or non-finite primitive"
            );
        }
        self.bvh = Bvh::build(&mut self.primitives);
        self.lights.clear();
        let mut total = 0.0;
        for (i, p) in self.primitives.iter_mut().enumerate() {
            p.info[2] = 0;
            let m = &self.materials[p.info[1] as usize];
            if m.is_light() {
                let area = p.area();
                total += area * luminance(unpack_vec3(m.emission));
                p.info[2] = self.lights.len() as u32 + 1;
                self.lights.push(Light {
                    primitive: i as u32,
                    pad: 0,
                    area,
                    cdf: total,
                });
            }
        }
        if total > 0.0 {
            for l in &mut self.lights {
                l.cdf /= total;
            }
            if let Some(l) = self.lights.last_mut() {
                l.cdf = 1.0
            }
        }
        Ok(())
    }
    /// Alpha cutouts apply equally to camera, indirect, shadow and denoising-guide rays.
    pub fn intersect(
        &self,
        ray: crate::geometry::Ray,
        limit: f32,
        any: bool,
    ) -> Option<crate::geometry::Hit> {
        self.bvh
            .intersect_filtered(&self.primitives, ray, limit, any, |hit| {
                self.accepts_cutout(hit)
            })
    }
    fn accepts_cutout(&self, hit: &crate::geometry::Hit) -> bool {
        let map = self.materials[hit.material].maps[3];
        map == 0
            || self
                .textures
                .sample(map as usize - 1, self.surface_uv(hit))
                .x
                >= 0.5
    }
    /// Keep early-out shadow traversal while accumulating straight thin-glass transmission.
    pub fn shadow_transmittance(&self, ray: crate::geometry::Ray, limit: f32) -> f32 {
        let mut transmission = 1.0;
        let blocker = self
            .bvh
            .intersect_filtered(&self.primitives, ray, limit, true, |hit| {
                if !self.accepts_cutout(hit) {
                    return false;
                }
                let material = self.materials[hit.material];
                if material.is_thin_glass() {
                    transmission *= crate::material::thin_glass_transmittance(
                        hit.geometric.dot(ray.direction),
                        material.surface[3],
                    );
                    false
                } else {
                    true
                }
            });
        if blocker.is_some() { 0.0 } else { transmission }
    }
    /// Texture colors are decoded to linear RGB before shading.
    pub fn surface_color(&self, hit: &crate::geometry::Hit) -> Vec3 {
        let material = &self.materials[hit.material];
        let color = material.color(hit.p);
        if material.detail[2] < 1.0 {
            return color;
        }
        let uv = self.surface_uv(hit);
        color
            * Vec3::ONE.lerp(
                self.textures.sample(material.detail[2] as usize - 1, uv),
                material.base[3],
            )
    }
    fn surface_uv(&self, hit: &crate::geometry::Hit) -> Vec2 {
        let material = &self.materials[hit.material];
        if material.detail[3] >= 0.0 {
            hit.uv * material.detail[3]
        } else {
            let n = hit.normal.abs();
            let uv = if n.y >= n.x && n.y >= n.z {
                Vec2::new(hit.p.x, hit.p.z)
            } else if n.x > n.z {
                Vec2::new(hit.p.z, hit.p.y)
            } else {
                Vec2::new(hit.p.x, hit.p.y)
            };
            uv * -material.detail[3]
        }
    }
    /// Evaluate data maps without changing geometric normals or ray offsets.
    pub fn surface_shading(&self, hit: &crate::geometry::Hit, outgoing: Vec3) -> (Material, Vec3) {
        let mut material = self.materials[hit.material];
        let mut normal = hit.normal;
        if material.is_glass() || material.maps == [0; 4] {
            return (material, normal);
        }
        let uv = self.surface_uv(hit);
        for (lane, slot) in [(1, 0), (2, 1)] {
            if material.maps[lane] > 0 {
                material.surface[slot] *= self
                    .textures
                    .sample(material.maps[lane] as usize - 1, uv)
                    .x
                    .clamp(0.0, 1.0);
            }
        }
        if material.maps[0] > 0 {
            let mapped = self.textures.sample(material.maps[0] as usize - 1, uv) * 2.0 - Vec3::ONE;
            let primitive = &self.primitives[hit.primitive];
            let frame = if material.detail[3] < 0.0 {
                let n = hit.normal.abs();
                Some(if n.y >= n.x && n.y >= n.z {
                    (Vec3::X, Vec3::Z)
                } else if n.x > n.z {
                    (Vec3::Z, Vec3::Y)
                } else {
                    (Vec3::X, Vec3::Y)
                })
            } else if primitive.info[0] == 1 {
                let du = Vec2::new(
                    primitive.uv01[2] - primitive.uv01[0],
                    primitive.uv01[3] - primitive.uv01[1],
                );
                let dv = Vec2::new(
                    primitive.uv2[0] - primitive.uv01[0],
                    primitive.uv2[1] - primitive.uv01[1],
                );
                let determinant = du.x * dv.y - du.y * dv.x;
                if determinant.abs() > 1e-10 {
                    Some((
                        (unpack_vec3(primitive.p1) * dv.y - unpack_vec3(primitive.p2) * du.y)
                            / determinant,
                        (unpack_vec3(primitive.p2) * du.x - unpack_vec3(primitive.p1) * dv.x)
                            / determinant,
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((tangent, bitangent)) = frame {
                let tangent = (tangent - normal * normal.dot(tangent)).normalize_or_zero();
                let sign = if normal.cross(tangent).dot(bitangent) < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                let bitangent = normal.cross(tangent) * sign;
                let candidate =
                    (tangent * mapped.x + bitangent * mapped.y + normal * mapped.z.max(0.01))
                        .normalize_or_zero();
                if candidate.dot(hit.geometric) > 0.05 && candidate.dot(outgoing) > 0.05 {
                    normal = candidate;
                }
            }
        }
        (material, normal)
    }
    pub fn environment(&self, d: Vec3) -> Vec3 {
        self.horizon
            .lerp(self.sky, (d.y * 0.5 + 0.5).clamp(0.0, 1.0))
    }
    pub fn light_probability(&self, id: usize) -> f32 {
        self.lights[id].cdf
            - if id == 0 {
                0.0
            } else {
                self.lights[id - 1].cdf
            }
    }
    pub fn add_material(&mut self, m: Material) -> usize {
        let i = self.materials.len();
        self.materials.push(m);
        i
    }
    pub fn add_sphere(&mut self, c: Vec3, r: f32, m: usize) {
        self.primitives.push(Primitive::sphere(c, r, m));
    }
    pub fn add_quad(&mut self, p: Vec3, u: Vec3, w: Vec3, m: usize) {
        self.primitives.push(Primitive::triangle(
            [p, p + u, p + u + w],
            None,
            [Vec2::ZERO, Vec2::X, Vec2::ONE],
            m,
        ));
        self.primitives.push(Primitive::triangle(
            [p, p + u + w, p + w],
            None,
            [Vec2::ZERO, Vec2::ONE, Vec2::Y],
            m,
        ));
    }
    /// Append a closed box with outward-facing triangles, including dielectric boundaries.
    pub fn add_box(&mut self, min: Vec3, max: Vec3, material: usize) {
        let extent = max - min;
        self.add_quad(min, Vec3::X * extent.x, Vec3::Z * extent.z, material);
        self.add_quad(
            Vec3::new(min.x, max.y, min.z),
            Vec3::Z * extent.z,
            Vec3::X * extent.x,
            material,
        );
        self.add_quad(min, Vec3::Y * extent.y, Vec3::X * extent.x, material);
        self.add_quad(
            Vec3::new(min.x, min.y, max.z),
            Vec3::X * extent.x,
            Vec3::Y * extent.y,
            material,
        );
        self.add_quad(min, Vec3::Z * extent.z, Vec3::Y * extent.y, material);
        self.add_quad(
            Vec3::new(max.x, min.y, min.z),
            Vec3::Y * extent.y,
            Vec3::Z * extent.z,
            material,
        );
    }

    /// Beveled geometry catches edge highlights that a mathematically sharp box cannot.
    pub fn rounded_box(&mut self, min: Vec3, max: Vec3, radius: f32, material: usize) {
        let center = (min + max) * 0.5;
        let half_size = (max - min) * 0.5;
        let radius = radius.min(half_size.min_element() * 0.95).max(1e-5);
        let core = half_size - Vec3::splat(radius);
        let coordinates = |half: f32| -> Vec<f32> {
            (0..=4)
                .map(|i| -half + radius * i as f32 / 4.0)
                .chain((0..=4).map(|i| half - radius + radius * i as f32 / 4.0))
                .collect()
        };
        for axis in 0..3 {
            let u_axis = (axis + 1) % 3;
            let v_axis = (axis + 2) % 3;
            let us = coordinates(half_size[u_axis]);
            let vs = coordinates(half_size[v_axis]);
            for sign in [-1.0, 1.0] {
                for u in us.windows(2) {
                    for v in vs.windows(2) {
                        let mut positions = [Vec3::ZERO; 4];
                        let mut normals = [Vec3::ZERO; 4];
                        for (i, (a, b)) in [(u[0], v[0]), (u[1], v[0]), (u[1], v[1]), (u[0], v[1])]
                            .into_iter()
                            .enumerate()
                        {
                            let mut raw = Vec3::ZERO;
                            raw[axis] = half_size[axis] * sign;
                            raw[u_axis] = a;
                            raw[v_axis] = b;
                            let nearest = raw.clamp(-core, core);
                            normals[i] = (raw - nearest).normalize();
                            positions[i] = center + nearest + normals[i] * radius;
                        }
                        for mut indices in [[0, 1, 2], [0, 2, 3]] {
                            if sign < 0.0 {
                                indices.swap(1, 2);
                            }
                            self.primitives.push(Primitive::triangle(
                                indices.map(|i| positions[i]),
                                Some(indices.map(|i| normals[i])),
                                [Vec2::ZERO; 3],
                                material,
                            ));
                        }
                    }
                }
            }
        }
    }
    pub fn import_obj(
        &mut self,
        path: &Path,
        position: Vec3,
        scale: f32,
        rotation: f32,
        override_material: Option<usize>,
    ) -> Result<usize> {
        ensure!(
            scale.is_finite() && scale > 0.0,
            "OBJ scale must be positive"
        );
        ensure!(
            position.is_finite() && rotation.is_finite(),
            "Invalid OBJ transform"
        );
        if let Some(material) = override_material {
            ensure!(
                material < self.materials.len(),
                "Invalid OBJ material override"
            );
        }
        let mut reader = std::io::BufReader::new(
            std::fs::File::open(path).with_context(|| format!("Opening OBJ {}", path.display()))?,
        );
        let base = path.parent().unwrap_or(Path::new("."));
        let (models, mtls) = tobj::load_obj_buf(
            &mut reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |relative| {
                let mtl_path = base.join(relative);
                let mtl_path = std::fs::canonicalize(&mtl_path)
                    .map_err(|_| tobj::LoadError::OpenFileFailed)?;
                let (mut materials, names) = tobj::load_mtl(&mtl_path)?;
                for material in &mut materials {
                    for key in ["norm", "map_Pr", "map_Pm"] {
                        if let Some(texture) = material.unknown_param.get_mut(key) {
                            *texture = mtl_path
                                .parent()
                                .unwrap_or(base)
                                .join(&*texture)
                                .to_string_lossy()
                                .into_owned();
                        }
                    }
                    if let Some(texture) = &mut material.dissolve_texture {
                        *texture = mtl_path
                            .parent()
                            .unwrap_or(base)
                            .join(&*texture)
                            .to_string_lossy()
                            .into_owned();
                    }
                    if let Some(texture) = &material.diffuse_texture {
                        material.diffuse_texture = Some(
                            mtl_path
                                .parent()
                                .unwrap_or(base)
                                .join(texture)
                                .to_string_lossy()
                                .into_owned(),
                        );
                    }
                }
                Ok((materials, names))
            },
        )
        .with_context(|| format!("Importing OBJ {}", path.display()))?;
        let offset = self.materials.len();
        let mtls = mtls.context("Reading MTL materials")?;
        for m in &mtls {
            let color = Vec3::from(m.diffuse.unwrap_or([0.7; 3]));
            let mut mat = Material::matte(color);
            mat.surface[0] = (2.0 / (m.shininess.unwrap_or(30.0) + 2.0))
                .sqrt()
                .clamp(0.025, 1.0);
            if m.dissolve.unwrap_or(1.0) < 0.5 {
                mat = Material::glass(
                    m.optical_density.unwrap_or(1.5).max(1.0),
                    (Vec3::ONE - color) * 0.2,
                )
            }
            if override_material.is_none()
                && let Some(texture) = &m.diffuse_texture
            {
                let texture_path = path.parent().unwrap_or(Path::new(".")).join(texture);
                let index = self.textures.load(&texture_path)?;
                mat = mat.textured(index, 1.0);
            }
            if override_material.is_none() {
                for (lane, key) in [(0, "norm"), (1, "map_Pr"), (2, "map_Pm")] {
                    if let Some(path) = m.unknown_param.get(key) {
                        mat.maps[lane] = self.textures.load_data(Path::new(path))? as u32 + 1;
                    }
                }
                if let Some(path) = &m.dissolve_texture {
                    mat.maps[3] = self.textures.load_data(Path::new(path))? as u32 + 1;
                }
                if let Some(value) = m.unknown_param.get("Pr") {
                    mat.surface[0] = value.parse().context("Invalid MTL roughness")?;
                }
                if let Some(value) = m.unknown_param.get("Pm") {
                    mat.surface[1] = value.parse().context("Invalid MTL metallic")?;
                }
                if mat.detail[3] == 0.0 {
                    mat.detail[3] = 1.0;
                }
            }
            self.materials.push(mat);
        }
        let fallback = self.add_material(Material::matte(Vec3::splat(0.7)));
        let rot = Mat3::from_rotation_y(rotation.to_radians());
        let before = self.primitives.len();
        for model in models {
            let m = model.mesh;
            let material = override_material.unwrap_or_else(|| {
                m.material_id
                    .filter(|i| *i < mtls.len())
                    .map(|i| offset + i)
                    .unwrap_or(fallback)
            });
            for tri in m.indices.as_chunks::<3>().0 {
                let mut p = [Vec3::ZERO; 3];
                let mut n = [Vec3::ZERO; 3];
                let mut uv = [Vec2::ZERO; 3];
                let normals = !m.normals.is_empty();
                for k in 0..3 {
                    let i = tri[k] as usize;
                    p[k] =
                        position + rot * Vec3::from_slice(&m.positions[i * 3..i * 3 + 3]) * scale;
                    if normals {
                        n[k] = rot * Vec3::from_slice(&m.normals[i * 3..i * 3 + 3]);
                    }
                    if !m.texcoords.is_empty() {
                        uv[k] = Vec2::from_slice(&m.texcoords[i * 2..i * 2 + 2]);
                    }
                }
                // Match scene validation: discarding small, valid triangles can
                // perforate a closed refractive mesh after it is scaled down.
                if (p[1] - p[0]).cross(p[2] - p[0]).length_squared() > 4e-24 {
                    self.primitives.push(Primitive::triangle(
                        p,
                        if normals { Some(n) } else { None },
                        uv,
                        material,
                    ));
                }
            }
        }
        ensure!(
            self.primitives.len() > before,
            "OBJ contains no valid triangles"
        );
        Ok(self.primitives.len() - before)
    }
}
