//! Camera navigation and deliberately approximate, geometry-preserving previews.
use crate::{
    camera::Camera,
    geometry::{Ray, offset, unpack_vec3},
    progressive::RenderControl,
    scene::Scene,
};
use anyhow::{Result, ensure};
use glam::{Vec2, Vec3};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::atomic::Ordering};

/// Walking stays on the initial eye-height plane; flight also follows camera pitch.
pub struct Navigator {
    pub camera: Camera,
    pub flying: bool,
    original: Camera,
}

impl Navigator {
    pub fn new(camera: Camera) -> Self {
        Self {
            camera,
            flying: false,
            original: camera,
        }
    }

    pub fn reset(&mut self) {
        self.camera = self.original;
        self.flying = false;
    }

    pub fn toggle_flight(&mut self) {
        self.flying = !self.flying;
    }

    pub fn look(&mut self, delta: Vec2) {
        if delta == Vec2::ZERO {
            return;
        }
        let forward = unpack_vec3(self.camera.forward);
        let yaw = forward.x.atan2(-forward.z) + delta.x;
        let pitch = (forward.y.asin() + delta.y).clamp(-1.52, 1.52);
        let direction = Vec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        );
        let right = direction.cross(Vec3::Y).normalize();
        let up = right.cross(direction);
        self.camera.forward[..3].copy_from_slice(&direction.to_array());
        self.camera.right[..3].copy_from_slice(&right.to_array());
        self.camera.up[..3].copy_from_slice(&up.to_array());
    }

    /// Conservative swept probes stop ordinary walking through walls and furniture.
    /// This is a camera guard, not a character physics or stair-climbing simulation.
    pub fn translate(&mut self, input: Vec3, distance: f32, scene: &Scene) -> bool {
        if input == Vec3::ZERO {
            return false;
        }
        let forward = unpack_vec3(self.camera.forward);
        let right = unpack_vec3(self.camera.right);
        let direction = if self.flying {
            forward
        } else {
            Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero()
        };
        let mut movement = right * input.x + direction * input.z;
        if self.flying {
            movement += Vec3::Y * input.y;
        }
        movement = movement.normalize_or_zero() * distance;
        let origin = unpack_vec3(self.camera.origin);
        let mut target = origin + movement;
        if !self.flying {
            // Slide along walls, checking each horizontal component separately.
            target = origin;
            for step in [Vec3::X * movement.x, Vec3::Z * movement.z] {
                if walking_clear(scene, target, step) {
                    target += step;
                }
            }
        }
        self.camera.origin[..3].copy_from_slice(&target.to_array());
        target != origin
    }
}

fn walking_clear(scene: &Scene, origin: Vec3, step: Vec3) -> bool {
    let length = step.length();
    if length < 1e-7 {
        return true;
    }
    let direction = step / length;
    let side = direction.cross(Vec3::Y) * 0.16;
    for height in [0.0, -0.65, -1.2] {
        for lateral in [-1.0, 0.0, 1.0] {
            let ray = Ray {
                origin: origin + Vec3::Y * height + side * lateral,
                direction,
            };
            if scene.intersect(ray, length + 0.20, true).is_some() {
                return false;
            }
        }
    }
    true
}

#[derive(Serialize, Deserialize)]
pub struct Viewpoint {
    pub scene: String,
    pub camera: Camera,
    pub exposure: f32,
}

impl Viewpoint {
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn load(path: &Path, scene: &str, aspect: f32) -> Result<Self> {
        let mut view: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        ensure!(
            view.scene == scene,
            "Viewpoint belongs to scene {}",
            view.scene
        );
        let c = &mut view.camera;
        ensure!(
            bytemuck::cast_slice::<Camera, f32>(&[*c])
                .iter()
                .all(|v| v.is_finite()),
            "Invalid camera values"
        );
        let f = unpack_vec3(c.forward);
        let r = unpack_vec3(c.right);
        let u = unpack_vec3(c.up);
        ensure!(
            [f, r, u]
                .iter()
                .all(|v| (v.length_squared() - 1.0).abs() < 0.001)
                && f.dot(r).abs() < 0.001
                && f.dot(u).abs() < 0.001
                && r.dot(u).abs() < 0.001
                && r.cross(f).dot(u) > 0.999,
            "Invalid camera basis"
        );
        ensure!(
            c.origin[3] >= 0.0
                && c.forward[3] > 0.0
                && (0.01..10.0).contains(&c.up[3])
                && view.exposure.is_finite()
                && view.exposure.abs() <= 30.0,
            "Invalid lens or exposure"
        );
        c.right[3] = c.up[3] * aspect;
        Ok(view)
    }
}

/// Stable headlight shading retains all geometry, texture cutouts and a limited glass
/// transmission preview. It intentionally omits physical lighting, smoke and depth of field.
pub fn preview_ray(scene: &Scene, mut ray: Ray) -> Vec3 {
    let mut tint = Vec3::ONE;
    let mut result = Vec3::ZERO;
    for _ in 0..8 {
        let Some(hit) = scene.intersect(ray, f32::INFINITY, false) else {
            return result + tint * scene.environment(ray.direction).max(Vec3::splat(0.12));
        };
        let material = &scene.materials[hit.material];
        let facing = hit.normal.dot(-ray.direction).abs();
        if material.is_glass() {
            result += tint * (0.025 + 0.20 * (1.0 - facing).powi(3));
            tint *= 0.92;
            // Straight transmission makes the silhouette legible without following
            // branching reflection paths. Final rendering uses the full dielectric BSDF.
            ray.origin = offset(hit.p, hit.geometric, ray.direction);
            continue;
        }
        return result
            + tint
                * (scene.surface_color(&hit) * (0.18 + 0.82 * facing)
                    + unpack_vec3(material.emission).min(Vec3::splat(2.0)));
    }
    result + tint * 0.12
}

pub fn preview_cpu(
    scene: &Scene,
    camera: Camera,
    width: u32,
    height: u32,
    control: &RenderControl,
) -> Option<Vec<Vec3>> {
    let pixels = (0..width * height)
        .into_par_iter()
        .map(|index| {
            if control.cancelled.load(Ordering::Relaxed) {
                return Vec3::ZERO;
            }
            preview_ray(
                scene,
                camera.center_ray(index % width, index / width, width, height),
            )
        })
        .collect();
    (!control.cancelled.load(Ordering::Relaxed)).then_some(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_stays_level_and_rotation_preserves_lens() {
        let scene = crate::scene::preset("studio", 16.0 / 9.0).unwrap();
        let mut nav = Navigator::new(scene.camera);
        nav.look(Vec2::new(0.2, 0.6));
        let height = nav.camera.origin[1];
        nav.translate(Vec3::Z, 0.01, &scene);
        assert_eq!(nav.camera.origin[1], height);
        assert_eq!(nav.camera.up[3], scene.camera.up[3]);
        assert_eq!(nav.camera.origin[3], scene.camera.origin[3]);
        nav.toggle_flight();
        nav.translate(Vec3::Y, 0.3, &scene);
        assert!((nav.camera.origin[1] - height - 0.3).abs() < 1e-6);
        nav.reset();
        assert_eq!(
            bytemuck::bytes_of(&nav.camera),
            bytemuck::bytes_of(&scene.camera)
        );
    }

    #[test]
    fn walking_guard_blocks_a_wall_even_for_a_large_step() {
        let mut scene = crate::scene::preset("studio", 1.0).unwrap();
        scene.add_box(Vec3::new(-2.0, 0.0, -1.1), Vec3::new(2.0, 3.0, -1.0), 0);
        scene.finish().unwrap();
        assert!(!walking_clear(
            &scene,
            Vec3::new(0.0, 1.6, 0.0),
            Vec3::new(0.0, 0.0, -3.0)
        ));
    }
}
