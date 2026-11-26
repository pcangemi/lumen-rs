//! Progressive tiles and cooperative controls, shared by the CPU and Metal backends.
use crate::{
    integrator::trace,
    render::{RenderOutput, RenderSettings, stats},
    rng::{Rng, luminance},
    scene::Scene,
};
use anyhow::Result;
use glam::Vec3;
use rayon::prelude::*;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub const TILE_SIZE: u32 = 64;

/// Controls are polled between tiles; no rendering resource is touched by the UI thread.
#[derive(Default)]
pub struct RenderControl {
    pub paused: AtomicBool,
    pub cancelled: AtomicBool,
}
impl RenderControl {
    pub fn wait_until_ready(&self) -> bool {
        while self.paused.load(Ordering::Relaxed) && !self.cancelled.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
        }
        !self.cancelled.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl Tile {
    pub fn pixel_count(self) -> usize {
        self.width as usize * self.height as usize
    }
    pub fn image_index(self, local: usize, image_width: u32) -> usize {
        let x = self.x as usize + local % self.width as usize;
        let y = self.y as usize + local / self.width as usize;
        y * image_width as usize + x
    }
}

/// Tiles start near the image center, so the subject appears early.
pub fn tiles(width: u32, height: u32) -> Vec<Tile> {
    let mut tiles = Vec::new();
    for y in (0..height).step_by(TILE_SIZE as usize) {
        for x in (0..width).step_by(TILE_SIZE as usize) {
            tiles.push(Tile {
                x,
                y,
                width: TILE_SIZE.min(width - x),
                height: TILE_SIZE.min(height - y),
            });
        }
    }
    tiles.sort_by_key(|tile| {
        let dx = i64::from(tile.x * 2 + tile.width) - i64::from(width);
        let dy = i64::from(tile.y * 2 + tile.height) - i64::from(height);
        dx * dx + dy * dy
    });
    tiles
}

/// Linear pixels for one finished tile. Consumers own the copy and may tone-map freely.
pub struct TileUpdate {
    pub tile: Tile,
    pub pixels: Vec<Vec3>,
    pub completed_samples: u64,
    pub progress: f32,
    pub pass_samples: u32,
    pub pass_complete: bool,
    pub elapsed: Duration,
}

#[derive(Clone, Copy, Default)]
struct PixelAccumulator {
    sum: Vec3,
    samples: u32,
    mean_luminance: f32,
    squared_deviation: f32,
    invalid_samples: u64,
    converged: bool,
}
impl PixelAccumulator {
    fn average(self) -> Vec3 {
        self.sum / self.samples.max(1) as f32
    }
    fn add(&mut self, mut radiance: Vec3, settings: RenderSettings) {
        if !radiance.is_finite() {
            self.invalid_samples += 1;
            radiance = Vec3::ZERO;
        }
        self.sum += radiance;
        self.samples += 1;
        let brightness = luminance(radiance);
        let difference = brightness - self.mean_luminance;
        self.mean_luminance += difference / self.samples as f32;
        self.squared_deviation += difference * (brightness - self.mean_luminance);
        if settings.adaptive > 0.0
            && self.samples >= settings.min_samples
            && self.samples.is_multiple_of(16)
        {
            let standard_error =
                (self.squared_deviation / (self.samples - 1) as f32 / self.samples as f32).sqrt();
            self.converged =
                1.96 * standard_error < settings.adaptive * self.mean_luminance.max(0.02);
        }
    }
}

/// Render in progressively refined tiles. Cancellation returns the partial image.
pub fn cpu(
    scene: &Scene,
    settings: RenderSettings,
    control: &RenderControl,
    mut on_tile: impl FnMut(TileUpdate),
) -> Result<RenderOutput> {
    settings.validate()?;
    let started = Instant::now();
    let count = settings.width as usize * settings.height as usize;
    let mut film = vec![PixelAccumulator::default(); count];
    let tiles = tiles(settings.width, settings.height);
    let mut completed_samples = 0;
    let mut completed_work = 0u64;
    let total_work = count as u64 * u64::from(settings.samples);
    'passes: for pass_start in (0..settings.samples).step_by(settings.batch as usize) {
        let pass_end = settings.samples.min(pass_start + settings.batch);
        for (tile_index, &tile) in tiles.iter().enumerate() {
            if !control.wait_until_ready() {
                break 'passes;
            }
            let updated: Vec<PixelAccumulator> = (0..tile.pixel_count())
                .into_par_iter()
                .map(|local| {
                    let index = tile.image_index(local, settings.width);
                    let mut pixel = film[index];
                    for sample in pass_start..pass_end {
                        if pixel.converged {
                            break;
                        }
                        let mut rng = Rng::new(index as u32, sample, settings.seed);
                        let ray = scene.camera.ray(
                            index as u32 % settings.width,
                            index as u32 / settings.width,
                            settings.width,
                            settings.height,
                            &mut rng,
                        );
                        pixel.add(trace(scene, ray, &mut rng, settings.max_depth), settings);
                    }
                    pixel
                })
                .collect();
            let mut pixels = Vec::with_capacity(tile.pixel_count());
            for (local, pixel) in updated.into_iter().enumerate() {
                let index = tile.image_index(local, settings.width);
                completed_samples += u64::from(pixel.samples - film[index].samples);
                film[index] = pixel;
                pixels.push(pixel.average());
            }
            completed_work += tile.pixel_count() as u64 * u64::from(pass_end - pass_start);
            on_tile(TileUpdate {
                tile,
                pixels,
                completed_samples,
                progress: completed_work as f32 / total_work as f32,
                pass_samples: pass_end,
                pass_complete: tile_index + 1 == tiles.len(),
                elapsed: started.elapsed(),
            });
        }
    }
    let samples: Vec<u32> = film.iter().map(|pixel| pixel.samples).collect();
    let invalid_samples = film.iter().map(|pixel| pixel.invalid_samples).sum();
    let stats = stats(
        scene,
        settings,
        "cpu",
        &format!("{} Rayon threads", rayon::current_num_threads()),
        started.elapsed().as_secs_f64(),
        &samples,
        invalid_samples,
    );
    Ok(RenderOutput {
        pixels: film.into_iter().map(PixelAccumulator::average).collect(),
        samples,
        stats,
    })
}
