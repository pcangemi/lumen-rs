use crate::geometry::{Hit, Primitive, Ray, pack_vec3, unpack_vec3};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

const BINS: usize = 16;
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Node {
    pub min: [f32; 4],
    pub max: [f32; 4],
    pub data: [u32; 4],
}
#[derive(Default)]
pub struct Bvh {
    pub nodes: Vec<Node>,
    pub depth: usize,
}
#[derive(Clone, Copy)]
struct Bounds {
    lo: Vec3,
    hi: Vec3,
}
impl Bounds {
    fn empty() -> Self {
        Self {
            lo: Vec3::splat(f32::INFINITY),
            hi: Vec3::splat(f32::NEG_INFINITY),
        }
    }
    fn add(self, b: Self) -> Self {
        Self {
            lo: self.lo.min(b.lo),
            hi: self.hi.max(b.hi),
        }
    }
    fn area(self) -> f32 {
        let e = (self.hi - self.lo).max(Vec3::ZERO);
        2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
    }
}
impl Bvh {
    pub fn build(primitives: &mut [Primitive]) -> Self {
        let mut b = Self::default();
        if !primitives.is_empty() {
            b.build_node(primitives, 0, 0);
        }
        b
    }
    fn build_node(&mut self, p: &mut [Primitive], start: usize, depth: usize) -> usize {
        self.depth = self.depth.max(depth);
        let index = self.nodes.len();
        self.nodes.push(Node::zeroed());
        let mut bounds = Bounds::empty();
        let mut centers = Bounds::empty();
        for x in p.iter() {
            let (lo, hi) = x.bounds();
            bounds = bounds.add(Bounds { lo, hi });
            let c = (lo + hi) * 0.5;
            centers = centers.add(Bounds { lo: c, hi: c });
        }
        self.nodes[index] = Node {
            min: pack_vec3(bounds.lo, 0.0),
            max: pack_vec3(bounds.hi, 0.0),
            data: [start as u32, p.len() as u32, 0, 0],
        };
        if p.len() <= 4 || depth >= 60 {
            return index;
        }
        let mut best = (f32::INFINITY, 0, 0);
        for axis in 0..3 {
            let extent = centers.hi[axis] - centers.lo[axis];
            if extent < 1e-7 {
                continue;
            }
            let mut bs = [Bounds::empty(); BINS];
            let mut counts = [0usize; BINS];
            for x in p.iter() {
                let (lo, hi) = x.bounds();
                let bin =
                    ((((lo[axis] + hi[axis]) * 0.5 - centers.lo[axis]) / extent * BINS as f32)
                        as usize)
                        .min(BINS - 1);
                bs[bin] = bs[bin].add(Bounds { lo, hi });
                counts[bin] += 1;
            }
            let mut right = [0.0; BINS];
            let mut b = Bounds::empty();
            let mut n = 0;
            for i in (0..BINS).rev() {
                b = b.add(bs[i]);
                n += counts[i];
                right[i] = b.area() * n as f32;
            }
            let mut b = Bounds::empty();
            let mut n = 0;
            for i in 0..BINS - 1 {
                b = b.add(bs[i]);
                n += counts[i];
                let cost = b.area() * n as f32 + right[i + 1];
                if n > 0 && n < p.len() && cost < best.0 {
                    best = (cost, axis, i);
                }
            }
        }
        if !best.0.is_finite() || (p.len() <= 8 && best.0 >= bounds.area() * (p.len() as f32 - 1.0))
        {
            return index;
        }
        let axis = best.1;
        let split = centers.lo[axis]
            + (centers.hi[axis] - centers.lo[axis]) * (best.2 + 1) as f32 / BINS as f32;
        let mut mid = 0;
        for i in 0..p.len() {
            let (lo, hi) = p[i].bounds();
            if (lo[axis] + hi[axis]) * 0.5 < split {
                p.swap(i, mid);
                mid += 1;
            }
        }
        if mid == 0 || mid == p.len() {
            return index;
        }
        let (left, right) = p.split_at_mut(mid);
        let li = self.build_node(left, start, depth + 1);
        let ri = self.build_node(right, start + mid, depth + 1);
        self.nodes[index].data = [li as u32, 0, ri as u32, axis as u32];
        index
    }
    #[inline]
    pub fn intersect(
        &self,
        primitives: &[Primitive],
        ray: Ray,
        max_t: f32,
        any: bool,
    ) -> Option<Hit> {
        self.intersect_filtered(primitives, ray, max_t, any, |_| true)
    }
    /// Reject masked candidates during traversal, retaining the nearest accepted hit.
    #[inline]
    pub fn intersect_filtered(
        &self,
        primitives: &[Primitive],
        ray: Ray,
        mut max_t: f32,
        any: bool,
        accept: impl Fn(&Hit) -> bool,
    ) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut stack = [0u32; 64];
        let mut sp = 1;
        let mut hit = None;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            if !box_hit(unpack_vec3(node.min), unpack_vec3(node.max), ray, max_t) {
                continue;
            }
            if node.data[1] > 0 {
                for i in node.data[0]..node.data[0] + node.data[1] {
                    if let Some(h) = primitives[i as usize].intersect(ray, max_t, i as usize)
                        && accept(&h)
                    {
                        if any {
                            return Some(h);
                        }
                        max_t = h.t;
                        hit = Some(h);
                    }
                }
            } else {
                let (near, far) = if ray.direction[node.data[3] as usize] >= 0.0 {
                    (node.data[0], node.data[2])
                } else {
                    (node.data[2], node.data[0])
                };
                stack[sp] = far;
                stack[sp + 1] = near;
                sp += 2;
            }
        }
        hit
    }
}
#[inline]
pub fn box_hit(lo: Vec3, hi: Vec3, ray: Ray, max_t: f32) -> bool {
    let mut t0 = 0.0f32;
    let mut t1 = max_t;
    for axis in 0..3 {
        let d = ray.direction[axis];
        if d.abs() < 1e-20 {
            if ray.origin[axis] < lo[axis] || ray.origin[axis] > hi[axis] {
                return false;
            }
            continue;
        }
        let a = (lo[axis] - ray.origin[axis]) / d;
        let b = (hi[axis] - ray.origin[axis]) / d;
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t1 < t0 {
            return false;
        }
    }
    true
}
