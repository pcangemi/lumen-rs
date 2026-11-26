//! Closed water meshes with analytic, time-parameterized surface displacement.
use super::*;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Waves {
    pub amplitude: f32,
    pub wavelength: f32,
    pub time: f32,
    pub resolution: u32,
}
impl Default for Waves {
    fn default() -> Self {
        Self {
            amplitude: 0.012,
            wavelength: 0.8,
            time: 0.0,
            resolution: 64,
        }
    }
}
impl Scene {
    pub fn add_water(&mut self, min: Vec3, max: Vec3, material: usize, waves: Waves) -> Result<()> {
        let extent = max - min;
        ensure!(
            min.is_finite() && max.is_finite() && extent.min_element() > 0.0,
            "Invalid water volume bounds"
        );
        ensure!(
            waves.amplitude.is_finite()
                && waves.amplitude >= 0.0
                && waves.amplitude < extent.y * 0.25
                && waves.wavelength.is_finite()
                && waves.wavelength > 0.0
                && waves.time.is_finite()
                && (2..=512).contains(&waves.resolution),
            "Invalid wave parameters"
        );
        let height = |x: f32, z: f32| {
            let u = (x - min.x) / extent.x;
            let v = (z - min.z) / extent.z;
            let k = TAU / waves.wavelength;
            let phase = (9.81 * k).sqrt() * waves.time;
            let signal = (k * x - phase).sin() * (k * z / 1.4 + phase * 0.7).cos()
                + 0.4 * (1.7 * k * x - 0.6 * k * z + phase * 1.3).sin();
            max.y + waves.amplitude * signal * (PI * u).sin() * (PI * v).sin()
        };
        let vertex = |u: f32, v: f32| {
            let x = min.x + u * extent.x;
            let z = min.z + v * extent.z;
            let step = 0.0005;
            let dx = (height(x + step, z) - height(x - step, z)) / (2.0 * step);
            let dz = (height(x, z + step) - height(x, z - step)) / (2.0 * step);
            (
                Vec3::new(x, height(x, z), z),
                Vec3::new(-dx, 1.0, -dz).normalize(),
                Vec2::new(u, v),
            )
        };
        let n = waves.resolution;
        for y in 0..n {
            for x in 0..n {
                let a = vertex(x as f32 / n as f32, y as f32 / n as f32);
                let b = vertex(x as f32 / n as f32, (y + 1) as f32 / n as f32);
                let c = vertex((x + 1) as f32 / n as f32, (y + 1) as f32 / n as f32);
                let d = vertex((x + 1) as f32 / n as f32, y as f32 / n as f32);
                for points in [[a, b, c], [a, c, d]] {
                    self.primitives.push(Primitive::triangle(
                        points.map(|p| p.0),
                        Some(points.map(|p| p.1)),
                        points.map(|p| p.2),
                        material,
                    ));
                }
            }
        }
        self.add_quad(min, Vec3::X * extent.x, Vec3::Z * extent.z, material);
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
        Ok(())
    }
}
