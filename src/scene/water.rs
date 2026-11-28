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
        self.add_water_surface(min, max, material, waves, false)?;
        self.add_water_boundary(min, max, material);
        Ok(())
    }

    /// A deterministic directional gravity-wave spectrum, evaluated as geometry.
    /// Independent wavelengths and phases avoid a repeating standing-wave grid.
    pub fn add_wind_water(
        &mut self,
        min: Vec3,
        max: Vec3,
        material: usize,
        waves: Waves,
    ) -> Result<()> {
        self.add_wind_water_patch(min, max, material, waves)?;
        self.add_water_boundary(min, max, material);
        Ok(())
    }

    /// Surface-only patch for a continuous ocean with one enclosing boundary.
    pub(crate) fn add_wind_water_patch(
        &mut self,
        min: Vec3,
        max: Vec3,
        material: usize,
        waves: Waves,
    ) -> Result<()> {
        self.add_water_surface(min, max, material, waves, true)
    }

    fn add_water_surface(
        &mut self,
        min: Vec3,
        max: Vec3,
        material: usize,
        waves: Waves,
        wind: bool,
    ) -> Result<()> {
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
        let mut rng = crate::rng::Rng::new(691, 0, 42);
        let components: Vec<_> = (0..24)
            .map(|i| {
                let wavelength =
                    waves.wavelength * (0.30 + 1.70 * (i as f32 + rng.uniform()) / 24.0);
                let direction = 0.65 + (rng.uniform() - 0.5) * 1.9;
                let k = TAU / wavelength;
                let phase = rng.uniform() * TAU - (9.81 * k).sqrt() * waves.time;
                let weight = (wavelength / waves.wavelength).powf(0.8);
                (
                    Vec2::new(direction.cos(), direction.sin()) * k,
                    phase,
                    weight,
                )
            })
            .collect();
        let normalization = components.iter().map(|c| c.2).sum::<f32>();
        let height = |x: f32, z: f32| {
            let u = (x - min.x) / extent.x;
            let v = (z - min.z) / extent.z;
            let k = TAU / waves.wavelength;
            let phase = (9.81 * k).sqrt() * waves.time;
            let signal = if wind {
                components
                    .iter()
                    .map(|(direction, phase, weight)| {
                        (direction.dot(Vec2::new(x, z)) + phase).sin() * weight
                    })
                    .sum::<f32>()
                    * 2.0
                    / normalization
            } else {
                (k * x - phase).sin() * (k * z / 1.4 + phase * 0.7).cos()
                    + 0.4 * (1.7 * k * x - 0.6 * k * z + phase * 1.3).sin()
            };
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
        let mut vertices = Vec::with_capacity(((n + 1) * (n + 1)) as usize);
        for y in 0..=n {
            for x in 0..=n {
                vertices.push(vertex(x as f32 / n as f32, y as f32 / n as f32));
            }
        }
        let at = |x: u32, y: u32| vertices[(y * (n + 1) + x) as usize];
        for y in 0..n {
            for x in 0..n {
                let a = at(x, y);
                let b = at(x, y + 1);
                let c = at(x + 1, y + 1);
                let d = at(x + 1, y);
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
        Ok(())
    }

    pub(crate) fn add_water_boundary(&mut self, min: Vec3, max: Vec3, material: usize) {
        let extent = max - min;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wind_water_is_repeatable_and_meets_its_closed_boundary() {
        let camera = Camera::new(Vec3::new(0.0, 3.0, 4.0), Vec3::ZERO, 40.0, 1.5, 0.0, 5.0);
        let build = || {
            let mut scene = Scene::empty(camera);
            let material = scene.add_material(Material::glass(1.333, Vec3::splat(0.01)));
            scene
                .add_wind_water(
                    Vec3::new(-3.0, -1.5, -2.0),
                    Vec3::new(3.0, 0.0, 2.0),
                    material,
                    Waves {
                        resolution: 32,
                        amplitude: 0.02,
                        ..Waves::default()
                    },
                )
                .unwrap();
            scene
        };
        let first = build();
        let second = build();
        assert_eq!(
            bytemuck::cast_slice::<Primitive, u8>(&first.primitives),
            bytemuck::cast_slice::<Primitive, u8>(&second.primitives)
        );
        assert_eq!(first.primitives.len(), 32 * 32 * 2 + 10);
        let mut displacement: f32 = 0.0;
        for triangle in &first.primitives[..32 * 32 * 2] {
            let a = unpack_vec3(triangle.p0);
            for p in [
                a,
                a + unpack_vec3(triangle.p1),
                a + unpack_vec3(triangle.p2),
            ] {
                assert!(p.is_finite());
                assert!(p.y.abs() <= 0.04);
                if (p.x.abs() - 3.0).abs() < 1e-6 || (p.z.abs() - 2.0).abs() < 1e-6 {
                    assert!(p.y.abs() < 1e-6, "Water must meet the vertical side faces");
                }
                displacement = displacement.max(p.y.abs());
            }
        }
        assert!(displacement > 0.001);
    }
}
