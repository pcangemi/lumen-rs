//! Bounded participating media: delta tracking, ratio tracking and Henyey-Greenstein scattering.
use crate::{
    geometry::{Ray, pack_vec3, unpack_vec3},
    rng::{Rng, hash, local_to_world},
};
use anyhow::{Result, ensure};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::f32::consts::{PI, TAU};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Medium {
    /// xyz bounds; w is the extinction majorant in inverse meters.
    pub min: [f32; 4],
    /// xyz bounds; w is phase anisotropy.
    pub max: [f32; 4],
    /// RGB single-scattering albedo; w selects heterogeneous cloud density.
    pub scattering: [f32; 4],
    /// Noise scale, reserved values.
    pub parameters: [f32; 4],
}
impl Medium {
    pub fn new(
        min: Vec3,
        max: Vec3,
        extinction: f32,
        albedo: Vec3,
        anisotropy: f32,
        noise_scale: f32,
    ) -> Result<Self> {
        ensure!(
            min.is_finite() && max.is_finite() && (max - min).min_element() > 0.0,
            "Invalid medium bounds"
        );
        ensure!(
            extinction.is_finite() && extinction > 0.0 && extinction <= 100.0,
            "Extinction must be in (0, 100]"
        );
        ensure!(
            albedo.is_finite() && albedo.min_element() >= 0.0 && albedo.max_element() <= 1.0,
            "Scattering albedo must be in [0, 1]"
        );
        ensure!(
            anisotropy.is_finite()
                && anisotropy.abs() < 0.99
                && noise_scale.is_finite()
                && noise_scale >= 0.0,
            "Invalid medium phase or noise parameters"
        );
        Ok(Self {
            min: pack_vec3(min, extinction),
            max: pack_vec3(max, anisotropy),
            scattering: pack_vec3(albedo, if noise_scale > 0.0 { 1.0 } else { 0.0 }),
            parameters: [noise_scale, 0.0, 0.0, 0.0],
        })
    }
    pub fn interval(&self, ray: Ray, limit: f32) -> Option<(f32, f32)> {
        let mut near = 0.0f32;
        let mut far = limit;
        for axis in 0..3 {
            if ray.direction[axis].abs() < 1e-20 {
                if ray.origin[axis] < self.min[axis] || ray.origin[axis] > self.max[axis] {
                    return None;
                }
            } else {
                let a = (self.min[axis] - ray.origin[axis]) / ray.direction[axis];
                let b = (self.max[axis] - ray.origin[axis]) / ray.direction[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
            }
        }
        (far > near).then_some((near, far))
    }
    pub fn density(&self, position: Vec3) -> f32 {
        if self.scattering[3] == 0.0 {
            return 1.0;
        }
        let center = (unpack_vec3(self.min) + unpack_vec3(self.max)) * 0.5;
        let half_size = (unpack_vec3(self.max) - unpack_vec3(self.min)) * 0.5;
        let q = (position - center) / half_size;
        let lobe = |center: Vec3, radius: Vec3| ((q - center) / radius).length() - 1.0;
        let shape = lobe(Vec3::new(0.0, -0.2, 0.0), Vec3::new(0.65, 0.5, 0.65))
            .min(lobe(
                Vec3::new(-0.45, -0.15, 0.05),
                Vec3::new(0.42, 0.5, 0.5),
            ))
            .min(lobe(
                Vec3::new(-0.12, 0.22, -0.05),
                Vec3::new(0.4, 0.68, 0.45),
            ))
            .min(lobe(Vec3::new(0.42, 0.0, 0.0), Vec3::new(0.44, 0.6, 0.48)));
        let p = position * self.parameters[0];
        let noise =
            0.55 * value_noise(p) + 0.3 * value_noise(p * 2.03) + 0.15 * value_noise(p * 4.07);
        let density = ((-shape + 0.7 * (noise - 0.5)) * 7.0).clamp(0.0, 1.0);
        density * density * (3.0 - 2.0 * density)
    }
    /// Sample a true collision; rejected Poisson events are null collisions.
    pub fn sample(&self, ray: Ray, limit: f32, rng: &mut Rng) -> Option<f32> {
        let (mut distance, end) = self.interval(ray, limit)?;
        loop {
            distance += -(1.0 - rng.uniform()).ln() / self.min[3];
            if distance >= end {
                return None;
            }
            if self.scattering[3] == 0.0 || rng.uniform() < self.density(ray.at(distance)) {
                return Some(distance);
            }
        }
    }
    /// Ratio tracking estimates heterogeneous shadow transmittance without fixed ray-march steps.
    pub fn transmittance(&self, ray: Ray, limit: f32, rng: &mut Rng) -> f32 {
        let Some((mut distance, end)) = self.interval(ray, limit) else {
            return 1.0;
        };
        if self.scattering[3] == 0.0 {
            return (-self.min[3] * (end - distance)).exp();
        }
        let mut weight = 1.0;
        loop {
            distance += -(1.0 - rng.uniform()).ln() / self.min[3];
            if distance >= end {
                return weight;
            }
            weight *= 1.0 - self.density(ray.at(distance));
            if weight == 0.0 {
                return 0.0;
            }
        }
    }
    pub fn phase(&self, cosine: f32) -> f32 {
        let g = self.max[3];
        (1.0 - g * g) / (4.0 * PI * (1.0 + g * g - 2.0 * g * cosine).max(1e-8).powf(1.5))
    }
    pub fn sample_phase(&self, forward: Vec3, rng: &mut Rng) -> Vec3 {
        let g = self.max[3];
        let u = rng.uniform();
        let cosine = if g.abs() < 1e-3 {
            1.0 - 2.0 * u
        } else {
            let ratio = (1.0 - g * g) / (1.0 - g + 2.0 * g * u);
            (1.0 + g * g - ratio * ratio) / (2.0 * g)
        }
        .clamp(-1.0, 1.0);
        let angle = TAU * rng.uniform();
        let sine = (1.0 - cosine * cosine).max(0.0).sqrt();
        local_to_world(
            Vec3::new(sine * angle.cos(), sine * angle.sin(), cosine),
            forward,
        )
    }
}
fn value_noise(p: Vec3) -> f32 {
    let cell = p.floor();
    let fraction = p - cell;
    let smooth = fraction * fraction * (Vec3::splat(3.0) - 2.0 * fraction);
    let value = |x: i32, y: i32, z: i32| {
        (hash(
            (x as u32).wrapping_mul(73856093)
                ^ (y as u32).wrapping_mul(19349663)
                ^ (z as u32).wrapping_mul(83492791),
        ) >> 8) as f32
            / 16777216.0
    };
    let mut layers = [0.0; 2];
    for (z, layer) in layers.iter_mut().enumerate() {
        let mut rows = [0.0; 2];
        for (y, row) in rows.iter_mut().enumerate() {
            let a = value(
                cell.x as i32,
                cell.y as i32 + y as i32,
                cell.z as i32 + z as i32,
            );
            let b = value(
                cell.x as i32 + 1,
                cell.y as i32 + y as i32,
                cell.z as i32 + z as i32,
            );
            *row = a + (b - a) * smooth.x;
        }
        *layer = rows[0] + (rows[1] - rows[0]) * smooth.y;
    }
    layers[0] + (layers[1] - layers[0]) * smooth.z
}
