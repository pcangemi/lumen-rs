use glam::Vec3;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy)]
pub struct Rng(pub u32);
impl Rng {
    pub fn new(pixel: u32, sample: u32, seed: u32) -> Self {
        Self(hash(pixel ^ hash(sample.wrapping_add(seed.wrapping_mul(747796405)))) | 1)
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    #[inline]
    pub fn uniform(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16777216.0)
    }
    pub fn disk(&mut self) -> (f32, f32) {
        let r = self.uniform().sqrt();
        let a = TAU * self.uniform();
        (r * a.cos(), r * a.sin())
    }
    pub fn cosine(&mut self, n: Vec3) -> Vec3 {
        let (x, y) = self.disk();
        local_to_world(Vec3::new(x, y, (1.0 - x * x - y * y).max(0.0).sqrt()), n)
    }
    pub fn sphere(&mut self) -> Vec3 {
        let z = 1.0 - 2.0 * self.uniform();
        let a = TAU * self.uniform();
        let r = (1.0 - z * z).sqrt();
        Vec3::new(r * a.cos(), z, r * a.sin())
    }
}
#[inline]
pub fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}
#[inline]
pub fn basis(n: Vec3) -> (Vec3, Vec3) {
    let t = if n.z.abs() < 0.999 {
        Vec3::Z.cross(n).normalize()
    } else {
        Vec3::X.cross(n).normalize()
    };
    (t, n.cross(t))
}
#[inline]
pub fn local_to_world(v: Vec3, n: Vec3) -> Vec3 {
    let (t, b) = basis(n);
    t * v.x + b * v.y + n * v.z
}
#[inline]
pub fn power(a: f32, b: f32) -> f32 {
    let a = a * a;
    a / (a + b * b).max(1e-30)
}
#[inline]
pub fn luminance(v: Vec3) -> f32 {
    v.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}
pub const INV_PI: f32 = 1.0 / PI;
