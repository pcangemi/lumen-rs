//! Parallel CPU rendering, validation and reproducible performance reports.
use crate::{
    integrator::trace,
    rng::{Rng, luminance},
    scene::Scene,
};
use anyhow::{Result, ensure};
use glam::Vec3;
use rayon::prelude::*;
use serde::Serialize;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub max_depth: u32,
    pub seed: u32,
    pub adaptive: f32,
    pub min_samples: u32,
    pub batch: u32,
}
impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 960,
            height: 640,
            samples: 256,
            max_depth: 12,
            seed: 42,
            adaptive: 0.0,
            min_samples: 64,
            batch: 16,
        }
    }
}
impl RenderSettings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.width > 0 && self.height > 0 && self.width <= 16384 && self.height <= 16384,
            "Dimensions must be between 1 and 16384"
        );
        ensure!(
            u64::from(self.width) * u64::from(self.height) <= 100_000_000,
            "Image is too large (100 megapixels maximum)"
        );
        ensure!(
            self.samples > 0 && self.samples <= 1_048_576,
            "Samples must be between 1 and 1048576"
        );
        ensure!(
            (1..=64).contains(&self.max_depth),
            "Path depth must be between 1 and 64"
        );
        ensure!(
            self.adaptive.is_finite() && self.adaptive >= 0.0 && self.adaptive <= 1.0,
            "Adaptive threshold must be between 0 and 1"
        );
        ensure!(
            self.min_samples >= 2 && self.batch > 0 && self.batch <= 256,
            "Minimum samples must be at least 2; batch size must be 1..256"
        );
        Ok(())
    }
}
#[derive(Serialize, Debug)]
pub struct RenderStats {
    pub backend: String,
    pub device: String,
    pub width: u32,
    pub height: u32,
    pub requested_spp: u32,
    pub average_spp: f64,
    pub max_depth: u32,
    pub seed: u32,
    pub adaptive_threshold: f32,
    pub render_seconds: f64,
    pub camera_samples_per_second: f64,
    pub primitives: usize,
    pub bvh_nodes: usize,
    pub bvh_depth: usize,
    pub invalid_samples: u64,
    pub denoiser: Option<String>,
    pub denoise_seconds: f64,
}
pub struct RenderOutput {
    pub pixels: Vec<Vec3>,
    pub samples: Vec<u32>,
    pub stats: RenderStats,
}

pub fn cpu(scene: &Scene, settings: RenderSettings) -> Result<RenderOutput> {
    settings.validate()?;
    let start = Instant::now();
    let count = settings.width as usize * settings.height as usize;
    let results: Vec<(Vec3, u32, u64)> = (0..count)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i as u32 % settings.width, i as u32 / settings.width);
            let mut sum = Vec3::ZERO;
            let mut n = 0;
            let mut mean = 0.0;
            let mut m2 = 0.0;
            let mut bad = 0;
            for sample in 0..settings.samples {
                let mut rng = Rng::new(i as u32, sample, settings.seed);
                let ray = scene
                    .camera
                    .ray(x, y, settings.width, settings.height, &mut rng);
                let mut c = trace(scene, ray, &mut rng, settings.max_depth);
                if !c.is_finite() {
                    bad += 1;
                    c = Vec3::ZERO
                }
                sum += c;
                n += 1;
                let l = luminance(c);
                let d = l - mean;
                mean += d / n as f32;
                m2 += d * (l - mean);
                if settings.adaptive > 0.0 && n >= settings.min_samples && n.is_multiple_of(16) {
                    let error = 1.96 * (m2 / (n - 1) as f32 / n as f32).sqrt();
                    if error < settings.adaptive * mean.max(0.02) {
                        break;
                    }
                }
            }
            (sum / n as f32, n, bad)
        })
        .collect();
    let elapsed = start.elapsed().as_secs_f64();
    let pixels = results.iter().map(|r| r.0).collect();
    let samples: Vec<u32> = results.iter().map(|r| r.1).collect();
    let bad = results.iter().map(|r| r.2).sum();
    let stats = stats(
        scene,
        settings,
        "cpu",
        &format!("{} Rayon threads", rayon::current_num_threads()),
        elapsed,
        &samples,
        bad,
    );
    Ok(RenderOutput {
        pixels,
        samples,
        stats,
    })
}
pub fn stats(
    scene: &Scene,
    s: RenderSettings,
    backend: &str,
    device: &str,
    elapsed: f64,
    samples: &[u32],
    invalid_samples: u64,
) -> RenderStats {
    let total: u64 = samples.iter().map(|n| *n as u64).sum();
    RenderStats {
        backend: backend.into(),
        device: device.into(),
        width: s.width,
        height: s.height,
        requested_spp: s.samples,
        average_spp: total as f64 / samples.len() as f64,
        max_depth: s.max_depth,
        seed: s.seed,
        adaptive_threshold: s.adaptive,
        render_seconds: elapsed,
        camera_samples_per_second: total as f64 / elapsed.max(1e-9),
        primitives: scene.primitives.len(),
        bvh_nodes: scene.bvh.nodes.len(),
        bvh_depth: scene.bvh.depth,
        invalid_samples,
        denoiser: None,
        denoise_seconds: 0.0,
    }
}
