use crate::rng::Rng;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};

#[derive(Clone, Copy, Debug)]
/// A geometric ray. Direction must have unit length.
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}
impl Ray {
    pub fn at(self, t: f32) -> Vec3 {
        self.origin + t * self.direction
    }
}
#[inline]
pub fn offset(p: Vec3, n: Vec3, d: Vec3) -> Vec3 {
    p + n * if n.dot(d) >= 0.0 {
        epsilon(p)
    } else {
        -epsilon(p)
    }
}
#[inline]
pub fn epsilon(p: Vec3) -> f32 {
    2e-5 * p.abs().max_element().max(1.0)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
/// Packed GPU record: p0 is position/radius; p1/p2 are triangle edges.
/// info stores primitive kind, material index, and one-based light index.
pub struct Primitive {
    pub p0: [f32; 4],
    pub p1: [f32; 4],
    pub p2: [f32; 4],
    pub n0: [f32; 4],
    pub n1: [f32; 4],
    pub n2: [f32; 4],
    pub uv01: [f32; 4],
    pub uv2: [f32; 4],
    pub info: [u32; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub p: Vec3,
    pub normal: Vec3,
    pub geometric: Vec3,
    pub uv: Vec2,
    pub front: bool,
    pub primitive: usize,
    pub material: usize,
}
#[inline]
/// Read the xyz part of a packed float4.
pub fn unpack_vec3(a: [f32; 4]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}
#[inline]
/// Pack a vector and scalar into a Metal-compatible float4.
pub fn pack_vec3(v: Vec3, w: f32) -> [f32; 4] {
    [v.x, v.y, v.z, w]
}
impl Primitive {
    pub fn sphere(center: Vec3, radius: f32, material: usize) -> Self {
        Self {
            p0: pack_vec3(center, radius),
            info: [0, material as u32, 0, 0],
            ..Self::zeroed()
        }
    }
    pub fn triangle(
        p: [Vec3; 3],
        normals: Option<[Vec3; 3]>,
        uv: [Vec2; 3],
        material: usize,
    ) -> Self {
        let n = normals.unwrap_or([(p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero(); 3]);
        Self {
            p0: pack_vec3(p[0], 0.0),
            p1: pack_vec3(p[1] - p[0], 0.0),
            p2: pack_vec3(p[2] - p[0], 0.0),
            n0: pack_vec3(n[0], 0.0),
            n1: pack_vec3(n[1], 0.0),
            n2: pack_vec3(n[2], 0.0),
            uv01: [uv[0].x, uv[0].y, uv[1].x, uv[1].y],
            uv2: [uv[2].x, uv[2].y, 0.0, 0.0],
            info: [1, material as u32, 0, 0],
        }
    }
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let p = unpack_vec3(self.p0);
        if self.info[0] == 0 {
            (p - Vec3::splat(self.p0[3]), p + Vec3::splat(self.p0[3]))
        } else {
            let q = p + unpack_vec3(self.p1);
            let r = p + unpack_vec3(self.p2);
            (
                p.min(q).min(r) - Vec3::splat(1e-5),
                p.max(q).max(r) + Vec3::splat(1e-5),
            )
        }
    }
    pub fn area(&self) -> f32 {
        if self.info[0] == 0 {
            4.0 * std::f32::consts::PI * self.p0[3] * self.p0[3]
        } else {
            0.5 * unpack_vec3(self.p1).cross(unpack_vec3(self.p2)).length()
        }
    }
    pub fn sample(&self, rng: &mut Rng) -> (Vec3, Vec3) {
        if self.info[0] == 0 {
            let n = rng.sphere();
            (unpack_vec3(self.p0) + n * self.p0[3], n)
        } else {
            let u = rng.uniform().sqrt();
            let w = rng.uniform();
            (
                unpack_vec3(self.p0)
                    + unpack_vec3(self.p1) * (u * (1.0 - w))
                    + unpack_vec3(self.p2) * (u * w),
                unpack_vec3(self.p1).cross(unpack_vec3(self.p2)).normalize(),
            )
        }
    }
    #[inline]
    pub fn intersect(&self, ray: Ray, max_t: f32, index: usize) -> Option<Hit> {
        let (t, outward, mut shading, uv) = if self.info[0] == 0 {
            let oc = ray.origin - unpack_vec3(self.p0);
            let b = oc.dot(ray.direction);
            let c = oc.length_squared() - self.p0[3] * self.p0[3];
            let disc = b * b - c;
            if disc < 0.0 {
                return None;
            }
            let d = disc.sqrt();
            let mut t = -b - d;
            if t <= 1e-5 {
                t = -b + d
            }
            if t <= 1e-5 || t >= max_t {
                return None;
            }
            let n = (ray.at(t) - unpack_vec3(self.p0)) / self.p0[3];
            (
                t,
                n,
                n,
                Vec2::new(
                    0.5 + n.z.atan2(n.x) / std::f32::consts::TAU,
                    0.5 - n.y.asin() / std::f32::consts::PI,
                ),
            )
        } else {
            let e1 = unpack_vec3(self.p1);
            let e2 = unpack_vec3(self.p2);
            let p = ray.direction.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-9 {
                return None;
            }
            let inv = 1.0 / det;
            let s = ray.origin - unpack_vec3(self.p0);
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                return None;
            }
            let q = s.cross(e1);
            let w = ray.direction.dot(q) * inv;
            if w < 0.0 || u + w > 1.0 {
                return None;
            }
            let t = e2.dot(q) * inv;
            if t <= 1e-5 || t >= max_t {
                return None;
            }
            let n = e1.cross(e2).normalize();
            let sn = (unpack_vec3(self.n0) * (1.0 - u - w)
                + unpack_vec3(self.n1) * u
                + unpack_vec3(self.n2) * w)
                .normalize_or_zero();
            let uv = Vec2::new(self.uv01[0], self.uv01[1]) * (1.0 - u - w)
                + Vec2::new(self.uv01[2], self.uv01[3]) * u
                + Vec2::new(self.uv2[0], self.uv2[1]) * w;
            (t, n, if sn.length_squared() > 0.5 { sn } else { n }, uv)
        };
        if shading.dot(outward) < 0.0 {
            shading = -shading
        }
        let front = ray.direction.dot(outward) < 0.0;
        let geometric = if front { outward } else { -outward };
        let mut normal = if front { shading } else { -shading };
        if normal.dot(-ray.direction) <= 0.0 {
            normal = geometric
        }
        Some(Hit {
            t,
            p: ray.at(t),
            normal,
            geometric,
            uv,
            front,
            primitive: index,
            material: self.info[1] as usize,
        })
    }
}
