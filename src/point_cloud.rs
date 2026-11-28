//! Area-sampled architectural point clouds for navigation only.
use crate::{
    camera::Camera,
    geometry::{Primitive, unpack_vec3},
    rng::Rng,
    scene::Scene,
};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::collections::HashMap;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Point {
    pub position: [f32; 4],
    pub color: [f32; 4],
}

pub struct PointCloud {
    pub points: Vec<Point>,
}

impl PointCloud {
    /// Sample surface area, including broad walls with few vertices. Large triangles
    /// are clipped to the navigation neighbourhood so distant ground cannot consume
    /// the point budget. The original rendering geometry is never modified.
    pub fn new(scene: &Scene, count: usize) -> Self {
        let center = unpack_vec3(scene.camera.origin);
        let lo = center - Vec3::splat(12.0);
        let hi = center + Vec3::splat(12.0);
        let mut clipped = HashMap::new();
        let mut cumulative = Vec::with_capacity(scene.primitives.len());
        let mut total = 0.0f64;
        for (index, primitive) in scene.primitives.iter().enumerate() {
            let (p_lo, p_hi) = primitive.bounds();
            let outside = p_hi.cmplt(lo).any() || p_lo.cmpgt(hi).any();
            let area = if outside {
                0.0
            } else if primitive.info[0] == 1 && (p_lo.cmplt(lo).any() || p_hi.cmpgt(hi).any()) {
                let polygon = clip_triangle(primitive, lo, hi);
                let area = polygon_area(&polygon);
                if area > 0.0 {
                    clipped.insert(index, polygon);
                }
                area
            } else {
                primitive.area()
            };
            let distance = ((p_lo + p_hi) * 0.5).clamp(lo, hi).distance(center);
            total += f64::from(area / (1.0 + (distance / 8.0).powi(2)));
            cumulative.push(total);
        }
        if total <= 0.0 {
            return Self { points: Vec::new() };
        }
        let points = (0..count)
            .map(|sample| {
                let mut rng = Rng::new(sample as u32, 0, 0x706f696e);
                let target = f64::from(rng.uniform()) * total;
                let index = cumulative
                    .partition_point(|value| *value <= target)
                    .min(cumulative.len() - 1);
                let primitive = &scene.primitives[index];
                let (position, normal) = if let Some(polygon) = clipped.get(&index) {
                    sample_polygon(polygon, &mut rng)
                } else {
                    primitive.sample(&mut rng)
                };
                let material = &scene.materials[primitive.info[1] as usize];
                if material.maps[3] != 0 {
                    let ray = crate::geometry::Ray {
                        origin: position + normal * 0.001,
                        direction: -normal,
                    };
                    if let Some(hit) = primitive.intersect(ray, 0.002, index)
                        && !scene.accepts_cutout(&hit)
                    {
                        return Point {
                            position: [position.x, position.y, position.z, 1.0],
                            color: [0.0; 4],
                        };
                    }
                }
                let base = if material.is_glass() {
                    Vec3::new(0.45, 0.78, 0.82)
                } else if material.is_light() {
                    Vec3::new(1.0, 0.86, 0.59)
                } else {
                    Vec3::new(0.70, 0.76, 0.79).lerp(unpack_vec3(material.base), 0.50)
                };
                let shade = 0.38 + 0.62 * normal.dot(Vec3::new(-0.35, 0.8, 0.45).normalize()).abs();
                Point {
                    position: [position.x, position.y, position.z, 1.0],
                    color: [base.x * shade, base.y * shade, base.z * shade, 1.0],
                }
            })
            .collect();
        Self { points }
    }

    /// Portable projection fallback. The Metal backend uses depth-tested point sprites.
    pub fn render_cpu(&self, camera: Camera, width: u32, height: u32, budget: usize) -> Vec<u32> {
        let mut pixels = vec![0x101921; width as usize * height as usize];
        let mut depths = vec![f32::INFINITY; pixels.len()];
        let origin = unpack_vec3(camera.origin);
        for point in self.points.iter().take(budget) {
            if point.color[3] == 0.0 {
                continue;
            }
            let relative = unpack_vec3(point.position) - origin;
            let depth = relative.dot(unpack_vec3(camera.forward));
            if !(0.03..100.0).contains(&depth) {
                continue;
            }
            let x = relative.dot(unpack_vec3(camera.right)) / (depth * camera.right[3]);
            let y = relative.dot(unpack_vec3(camera.up)) / (depth * camera.up[3]);
            if x.abs() > 1.0 || y.abs() > 1.0 {
                continue;
            }
            let x = ((x * 0.5 + 0.5) * width as f32) as i32;
            let y = ((0.5 - y * 0.5) * height as f32) as i32;
            let fade = 1.0 / (1.0 + depth * 0.045);
            let c = unpack_vec3(point.color) * fade;
            let color =
                ((c.x * 255.0) as u32) << 16 | ((c.y * 255.0) as u32) << 8 | (c.z * 255.0) as u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let (px, py) = (x + dx, y + dy);
                    if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                        continue;
                    }
                    let index = py as usize * width as usize + px as usize;
                    if depth < depths[index] {
                        depths[index] = depth;
                        pixels[index] = color;
                    }
                }
            }
        }
        pixels
    }
}

fn clip_triangle(primitive: &Primitive, lo: Vec3, hi: Vec3) -> Vec<Vec3> {
    let p = unpack_vec3(primitive.p0);
    let mut polygon = vec![
        p,
        p + unpack_vec3(primitive.p1),
        p + unpack_vec3(primitive.p2),
    ];
    for axis in 0..3 {
        for (plane, sign) in [(lo[axis], 1.0), (hi[axis], -1.0)] {
            let mut clipped = Vec::new();
            for index in 0..polygon.len() {
                let a = polygon[index];
                let b = polygon[(index + 1) % polygon.len()];
                let da = (a[axis] - plane) * sign;
                let db = (b[axis] - plane) * sign;
                if da >= 0.0 {
                    clipped.push(a);
                }
                if (da >= 0.0) != (db >= 0.0) {
                    clipped.push(a.lerp(b, da / (da - db)));
                }
            }
            polygon = clipped;
        }
    }
    polygon
}
fn polygon_area(polygon: &[Vec3]) -> f32 {
    if polygon.len() < 3 {
        return 0.0;
    }
    (1..polygon.len() - 1)
        .map(|i| {
            (polygon[i] - polygon[0])
                .cross(polygon[i + 1] - polygon[0])
                .length()
                * 0.5
        })
        .sum()
}
fn sample_polygon(polygon: &[Vec3], rng: &mut Rng) -> (Vec3, Vec3) {
    let mut remaining = rng.uniform() * polygon_area(polygon);
    for index in 1..polygon.len() - 1 {
        let u = polygon[index] - polygon[0];
        let v = polygon[index + 1] - polygon[0];
        let cross = u.cross(v);
        let area = cross.length() * 0.5;
        if remaining <= area || index + 2 == polygon.len() {
            let root = rng.uniform().sqrt();
            let t = rng.uniform();
            return (
                polygon[0] + u * (root * (1.0 - t)) + v * (root * t),
                cross.normalize_or_zero(),
            );
        }
        remaining -= area;
    }
    (polygon[0], Vec3::Y)
}

/// Adjust only the navigation point count, leaving room for input and presentation.
pub fn adjust_budget(current: usize, maximum: usize, seconds: f64) -> usize {
    if seconds > 0.008 {
        (current * 4 / 5).max(400_000).min(maximum)
    } else if seconds < 0.004 {
        (current * 21 / 20).min(maximum)
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn giant_triangle_is_clipped_without_spending_points_outside_the_room() {
        let triangle = Primitive::triangle(
            [
                Vec3::new(-1000.0, 0.0, -1000.0),
                Vec3::new(1000.0, 0.0, -1000.0),
                Vec3::new(0.0, 0.0, 1000.0),
            ],
            None,
            [glam::Vec2::ZERO; 3],
            0,
        );
        let polygon = clip_triangle(&triangle, Vec3::splat(-2.0), Vec3::splat(2.0));
        assert!((polygon_area(&polygon) - 16.0).abs() < 0.001);
        let mut rng = Rng::new(0, 0, 1);
        for _ in 0..1000 {
            let (point, normal) = sample_polygon(&polygon, &mut rng);
            assert!(point.is_finite() && normal.is_finite() && point.abs().max_element() <= 2.001);
        }
    }
}
