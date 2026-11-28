use crate::{
    geometry::{Ray, pack_vec3, unpack_vec3},
    rng::Rng,
};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use serde::{Deserialize, Serialize};
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, Serialize, Deserialize)]
pub struct Camera {
    pub origin: [f32; 4],
    pub forward: [f32; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
}
impl Camera {
    /// Pinhole ray through a pixel center, used only for interactive navigation.
    pub fn center_ray(&self, x: u32, y: u32, width: u32, height: u32) -> Ray {
        let sx = 2.0 * (x as f32 + 0.5) / width as f32 - 1.0;
        let sy = 1.0 - 2.0 * (y as f32 + 0.5) / height as f32;
        Ray {
            origin: unpack_vec3(self.origin),
            direction: (unpack_vec3(self.forward)
                + sx * self.right[3] * unpack_vec3(self.right)
                + sy * self.up[3] * unpack_vec3(self.up))
            .normalize(),
        }
    }
    pub fn new(from: Vec3, to: Vec3, vfov: f32, aspect: f32, aperture: f32, focus: f32) -> Self {
        let f = (to - from).normalize();
        let reference = if f.y.abs() > 0.999 { Vec3::Z } else { Vec3::Y };
        let r = f.cross(reference).normalize();
        let u = r.cross(f);
        let h = (vfov.to_radians() * 0.5).tan();
        Self {
            origin: pack_vec3(from, aperture * 0.5),
            forward: pack_vec3(f, focus),
            right: pack_vec3(r, aspect * h),
            up: pack_vec3(u, h),
        }
    }
    pub fn ray(&self, x: u32, y: u32, width: u32, height: u32, rng: &mut Rng) -> Ray {
        let sx = 2.0 * (x as f32 + rng.uniform()) / width as f32 - 1.0;
        let sy = 1.0 - 2.0 * (y as f32 + rng.uniform()) / height as f32;
        let d = unpack_vec3(self.forward)
            + sx * self.right[3] * unpack_vec3(self.right)
            + sy * self.up[3] * unpack_vec3(self.up);
        let (dx, dy) = rng.disk();
        let offset = (unpack_vec3(self.right) * dx + unpack_vec3(self.up) * dy) * self.origin[3];
        Ray {
            origin: unpack_vec3(self.origin) + offset,
            direction: (d * self.forward[3] - offset).normalize(),
        }
    }
}
