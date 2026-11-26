use crate::{
    geometry::{pack_vec3, unpack_vec3},
    rng::{INV_PI, Rng, basis, local_to_world},
};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
/// Linear-RGB material record shared with the Metal shader.
/// Glass is smooth; opaque surfaces combine Lambert and visible-normal GGX.
pub struct Material {
    pub base: [f32; 4],
    pub emission: [f32; 4],
    // roughness, metallic, transmission, index of refraction
    pub surface: [f32; 4],
    pub absorption: [f32; 4],
    // Pattern scale, kind (0 solid, 1 stone, 2 wood, 3 checker), texture index + 1, mapping scale.
    pub detail: [f32; 4],
}
impl Material {
    pub fn matte(color: Vec3) -> Self {
        Self {
            base: pack_vec3(color, 0.0),
            emission: [0.0; 4],
            surface: [0.65, 0.0, 0.0, 1.5],
            absorption: [0.0; 4],
            detail: [1.0, 0.0, 0.0, 0.0],
        }
    }
    pub fn metal(color: Vec3, roughness: f32) -> Self {
        Self {
            surface: [roughness, 1.0, 0.0, 1.5],
            ..Self::matte(color)
        }
    }
    pub fn glass(ior: f32, absorption: Vec3) -> Self {
        Self {
            surface: [0.0, 0.0, 1.0, ior],
            absorption: pack_vec3(absorption, 0.0),
            ..Self::matte(Vec3::ONE)
        }
    }
    pub fn light(color: Vec3) -> Self {
        Self {
            emission: pack_vec3(color, 0.0),
            ..Self::matte(Vec3::ZERO)
        }
    }
    pub fn patterned(mut self, kind: u32, scale: f32) -> Self {
        self.detail = [scale, kind as f32, 0.0, 0.0];
        self
    }
    /// Positive scale uses UV coordinates; negative scale projects world coordinates.
    pub fn textured(mut self, texture: usize, scale: f32) -> Self {
        self.base[3] = 1.0;
        self.detail[2] = (texture + 1) as f32;
        self.detail[3] = scale;
        self
    }
    /// Blend the source reflectance map with the material's chosen base color.
    pub fn texture_strength(mut self, strength: f32) -> Self {
        self.base[3] = strength.clamp(0.0, 1.0);
        self
    }
    pub fn color(&self, p: Vec3) -> Vec3 {
        let q = p * self.detail[0];
        let k = match self.detail[1] as u32 {
            1 => {
                let t = (q.x * 1.7
                    + q.z * 2.2
                    + 2.0 * (q.z * 0.7 + q.x * 0.5).sin()
                    + 0.3 * (q.x * 9.0 + q.z * 7.0).sin())
                .sin()
                .abs();
                0.58 + 0.42 * t.powf(0.18)
            }
            2 => {
                let t = (q.x * 3.0 + 0.7 * (q.z * 0.8).sin() + 0.25 * (q.z * 3.0).sin()).sin();
                0.68 + 0.25 * t + 0.07 * (q.x * 47.0 + q.z).sin()
            }
            3 => {
                if ((q.x.floor() + q.z.floor()) as i32) & 1 == 0 {
                    1.0
                } else {
                    0.35
                }
            }
            _ => 1.0,
        };
        unpack_vec3(self.base) * k
    }
    pub fn is_glass(&self) -> bool {
        self.surface[2] > 0.5
    }
    pub fn is_light(&self) -> bool {
        unpack_vec3(self.emission).max_element() > 0.0
    }
    #[inline]
    /// Evaluate BRDF and solid-angle PDF. wi/wo both point away from the surface.
    pub fn eval(&self, color: Vec3, n: Vec3, wo: Vec3, wi: Vec3) -> (Vec3, f32) {
        let nv = n.dot(wo).max(0.0);
        let nl = n.dot(wi).max(0.0);
        if nv <= 0.0 || nl <= 0.0 {
            return (Vec3::ZERO, 0.0);
        }
        let h = (wi + wo).normalize_or_zero();
        let nh = n.dot(h).max(0.0);
        let vh = wo.dot(h).max(0.0);
        let alpha = self.surface[0].max(0.025).powi(2);
        let a2 = alpha * alpha;
        let d = a2 * INV_PI / (nh * nh * (a2 - 1.0) + 1.0).powi(2).max(1e-16);
        let g1 = smith(nv, a2);
        let g2 = smith(nl, a2);
        let f0 = Vec3::splat(0.04).lerp(color, self.surface[1]);
        let f = f0 + (Vec3::ONE - f0) * (1.0 - vh).powi(5);
        let brdf = (Vec3::ONE - f) * color * ((1.0 - self.surface[1]) * INV_PI)
            + f * (d * g1 * g2 / (4.0 * nv * nl).max(1e-12));
        let spec_p = 0.25 + 0.5 * self.surface[1];
        let pdf = spec_p * d * g1 / (4.0 * nv).max(1e-12) + (1.0 - spec_p) * nl * INV_PI;
        (brdf, pdf)
    }
    pub fn sample(&self, n: Vec3, wo: Vec3, rng: &mut Rng) -> Vec3 {
        if rng.uniform() >= 0.25 + 0.5 * self.surface[1] {
            return rng.cosine(n);
        }
        // Heitz visible GGX normals: avoids wasting samples on hidden microfacets.
        let (t, b) = basis(n);
        let alpha = self.surface[0].max(0.025).powi(2);
        let vh = Vec3::new(alpha * wo.dot(t), alpha * wo.dot(b), wo.dot(n)).normalize();
        let lensq = vh.x * vh.x + vh.y * vh.y;
        let t1 = if lensq > 0.0 {
            Vec3::new(-vh.y, vh.x, 0.0) / lensq.sqrt()
        } else {
            Vec3::X
        };
        let t2 = vh.cross(t1);
        let (x, mut y) = rng.disk();
        let s = 0.5 * (1.0 + vh.z);
        y = (1.0 - s) * (1.0 - x * x).max(0.0).sqrt() + s * y;
        let nh = x * t1 + y * t2 + (1.0 - x * x - y * y).max(0.0).sqrt() * vh;
        let h = local_to_world(
            Vec3::new(alpha * nh.x, alpha * nh.y, nh.z.max(0.0)).normalize(),
            n,
        );
        (-wo + 2.0 * wo.dot(h) * h).normalize()
    }
}
#[inline]
fn smith(n: f32, a2: f32) -> f32 {
    2.0 * n / (n + (a2 + (1.0 - a2) * n * n).sqrt()).max(1e-10)
}
pub fn fresnel(cos: f32, eta_i: f32, eta_t: f32) -> f32 {
    let sin_t = eta_i / eta_t * (1.0 - cos * cos).max(0.0).sqrt();
    if sin_t >= 1.0 {
        return 1.0;
    }
    let ct = (1.0 - sin_t * sin_t).sqrt();
    let rs = (eta_i * cos - eta_t * ct) / (eta_i * cos + eta_t * ct);
    let rp = (eta_t * cos - eta_i * ct) / (eta_t * cos + eta_i * ct);
    0.5 * (rs * rs + rp * rp)
}
