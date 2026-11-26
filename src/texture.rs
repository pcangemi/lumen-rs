//! Linear texture atlas with repeat wrapping and bilinear sampling on CPU and Metal.
use anyhow::{Context, Result, ensure};
use glam::{Vec2, Vec3};
use std::path::Path;

#[derive(Default)]
pub struct TextureAtlas {
    pub pixels: Vec<[f32; 4]>,
    /// offset, width, height, reserved; matches a Metal uint4.
    pub descriptors: Vec<[u32; 4]>,
}
impl TextureAtlas {
    pub fn load(&mut self, path: &Path) -> Result<usize> {
        self.load_encoded(path, true)
    }
    /// Normal, roughness and metallic maps contain linear data, not sRGB colors.
    pub fn load_data(&mut self, path: &Path) -> Result<usize> {
        self.load_encoded(path, false)
    }
    fn load_encoded(&mut self, path: &Path, srgb: bool) -> Result<usize> {
        let image = image::open(path)
            .with_context(|| format!("Loading texture {}", path.display()))?
            .to_rgb32f();
        let (width, height) = image.dimensions();
        ensure!(
            width > 0 && height > 0 && width <= 8192 && height <= 8192,
            "Texture dimensions must be between 1 and 8192"
        );
        ensure!(
            self.pixels.len() as u64 + u64::from(width) * u64::from(height) <= u64::from(u32::MAX),
            "Texture atlas exceeds its index capacity"
        );
        let index = self.descriptors.len();
        self.descriptors
            .push([self.pixels.len() as u32, width, height, 0]);
        self.pixels.extend(image.pixels().map(|pixel| {
            let linear = |x: f32| {
                if !srgb {
                    x
                } else if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                }
            };
            [linear(pixel[0]), linear(pixel[1]), linear(pixel[2]), 0.0]
        }));
        Ok(index)
    }
    #[inline]
    pub fn sample(&self, index: usize, uv: Vec2) -> Vec3 {
        let [offset, width, height, _] = self.descriptors[index];
        let x = uv.x.rem_euclid(1.0) * width as f32 - 0.5;
        let y = (1.0 - uv.y.rem_euclid(1.0)) * height as f32 - 0.5;
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let texel = |x: i32, y: i32| {
            let x = x.rem_euclid(width as i32) as u32;
            let y = y.rem_euclid(height as i32) as u32;
            let p = self.pixels[(offset + y * width + x) as usize];
            Vec3::new(p[0], p[1], p[2])
        };
        let top = texel(ix, iy).lerp(texel(ix + 1, iy), x - x.floor());
        let bottom = texel(ix, iy + 1).lerp(texel(ix + 1, iy + 1), x - x.floor());
        top.lerp(bottom, y - y.floor())
    }
}
