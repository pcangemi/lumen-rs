//! Native Metal compute backend. Buffers use explicit 16-byte records shared with MSL.
use crate::progressive::{RenderControl, Tile, TileUpdate, tiles};
use crate::{
    camera::Camera,
    geometry::pack_vec3,
    render::{RenderOutput, RenderSettings, stats},
    scene::Scene,
};
use anyhow::{Result, anyhow, ensure};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use metal::*;
use std::time::Instant;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    camera: Camera,
    sky: [f32; 4],
    horizon: [f32; 4],
    image: [u32; 4],
    config: [u32; 4],
    adaptive: [f32; 4],
    tile: [u32; 4],
    medium: crate::medium::Medium,
}
fn buffer<T: Pod>(device: &Device, data: &[T]) -> Buffer {
    if data.is_empty() {
        device.new_buffer(16, MTLResourceOptions::StorageModeShared)
    } else {
        device.new_buffer_with_data(
            data.as_ptr().cast(),
            std::mem::size_of_val(data) as u64,
            MTLResourceOptions::StorageModeShared,
        )
    }
}
pub fn available() -> Option<String> {
    Device::system_default().map(|d| d.name().to_owned())
}
pub fn render(scene: &Scene, s: RenderSettings) -> Result<RenderOutput> {
    render_impl(scene, s, &RenderControl::default(), false, |_| {})
}

/// Keep all Metal objects on the rendering thread and notify the UI after completed tiles.
pub fn render_progressive(
    scene: &Scene,
    s: RenderSettings,
    control: &RenderControl,
    on_tile: impl FnMut(TileUpdate),
) -> Result<RenderOutput> {
    render_impl(scene, s, control, true, on_tile)
}

fn render_impl(
    scene: &Scene,
    s: RenderSettings,
    control: &RenderControl,
    progressive: bool,
    mut on_tile: impl FnMut(TileUpdate),
) -> Result<RenderOutput> {
    s.validate()?;
    objc::rc::autoreleasepool(|| {
        let device = Device::system_default().ok_or_else(|| {
            anyhow!("No Metal GPU is accessible. Use --backend cpu or launch from a regular Terminal session.")
        })?;
        let options = CompileOptions::new();
        options.set_fast_math_enabled(false);
        let library = device
            .new_library_with_source(include_str!("pathtrace.metal"), &options)
            .map_err(|e| anyhow!("Metal shader compilation: {e}"))?;
        let function = library
            .get_function("pathtrace", None)
            .map_err(|e| anyhow!("Kernel pathtrace : {e}"))?;
        let pipeline = device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|e| anyhow!("Pipeline Metal : {e}"))?;
        let queue = device.new_command_queue();
        let count = s.width as usize * s.height as usize;
        let primitives = buffer(&device, &scene.primitives);
        let nodes = buffer(&device, &scene.bvh.nodes);
        let materials = buffer(&device, &scene.materials);
        let lights = buffer(&device, &scene.lights);
        let texels = buffer(&device, &scene.textures.pixels);
        let texture_info = buffer(&device, &scene.textures.descriptors);
        let film = buffer(&device, &vec![[0.0f32; 4]; count]);
        let moments = buffer(&device, &vec![[0.0f32; 4]; count]);
        let threads = pipeline.thread_execution_width() * 4;
        let threads = threads.min(pipeline.max_total_threads_per_threadgroup());
        let start = Instant::now();
        let mut completed_work = 0u64;
        let mut completed_samples = 0u64;
        let mut previous_samples = if progressive {
            vec![0u32; count]
        } else {
            Vec::new()
        };
        // Offline rendering dispatches the full film and only reads it after the last batch.
        // The viewer retains small tiles for interactive progress and cancellation.
        let tiles = if progressive {
            tiles(s.width, s.height)
        } else {
            vec![Tile {
                x: 0,
                y: 0,
                width: s.width,
                height: s.height,
            }]
        };
        'passes: for sample in (0..s.samples).step_by(s.batch as usize) {
            for (tile_index, &tile) in tiles.iter().enumerate() {
                if !control.wait_until_ready() {
                    break 'passes;
                }
                let params = Params {
                    camera: scene.camera,
                    sky: pack_vec3(scene.sky, 0.0),
                    horizon: pack_vec3(scene.horizon, 0.0),
                    image: [s.width, s.height, sample, s.samples.min(sample + s.batch)],
                    config: [
                        s.max_depth,
                        s.seed,
                        scene.lights.len() as u32,
                        s.min_samples,
                    ],
                    adaptive: [s.adaptive, 0.0, 0.0, 0.0],
                    tile: [tile.x, tile.y, tile.width, tile.height],
                    medium: scene.medium.unwrap_or_else(crate::medium::Medium::zeroed),
                };
                objc::rc::autoreleasepool(|| -> Result<()> {
                    let cb = queue.new_command_buffer();
                    let encoder = cb.new_compute_command_encoder();
                    encoder.set_compute_pipeline_state(&pipeline);
                    for (i, b) in [&primitives, &nodes, &materials, &lights, &film, &moments]
                        .iter()
                        .enumerate()
                    {
                        encoder.set_buffer(i as u64, Some(b), 0);
                    }
                    encoder.set_buffer(7, Some(&texels), 0);
                    encoder.set_buffer(8, Some(&texture_info), 0);
                    encoder.set_bytes(
                        6,
                        std::mem::size_of::<Params>() as u64,
                        (&params as *const Params).cast(),
                    );
                    encoder.dispatch_threads(
                        MTLSize::new(tile.pixel_count() as u64, 1, 1),
                        MTLSize::new(threads, 1, 1),
                    );
                    encoder.end_encoding();
                    cb.commit();
                    cb.wait_until_completed();
                    ensure!(
                        cb.status() == MTLCommandBufferStatus::Completed,
                        "Metal command buffer failed: {:?}",
                        cb.status()
                    );
                    Ok(())
                })?;
                if !progressive {
                    continue;
                }
                // SAFETY: The command buffer has completed, so the shared film is stable.
                let values = unsafe {
                    std::slice::from_raw_parts(film.contents().cast::<[f32; 4]>(), count)
                };
                let mut pixels = Vec::with_capacity(tile.pixel_count());
                for local in 0..tile.pixel_count() {
                    let index = tile.image_index(local, s.width);
                    let value = values[index];
                    let samples = value[3] as u32;
                    completed_samples += u64::from(samples - previous_samples[index]);
                    previous_samples[index] = samples;
                    pixels.push(Vec3::new(value[0], value[1], value[2]) / value[3].max(1.0));
                }
                let pass_end = s.samples.min(sample + s.batch);
                completed_work += tile.pixel_count() as u64 * u64::from(pass_end - sample);
                on_tile(TileUpdate {
                    tile,
                    pixels,
                    completed_samples,
                    progress: completed_work as f32 / (count as f32 * s.samples as f32),
                    pass_samples: pass_end,
                    pass_complete: tile_index + 1 == tiles.len(),
                    elapsed: start.elapsed(),
                });
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        // SAFETY: Shared buffers were allocated for count float4s; the GPU has completed all writes.
        let values =
            unsafe { std::slice::from_raw_parts(film.contents().cast::<[f32; 4]>(), count) };
        // SAFETY: The same completed-command guarantee applies to the moment buffer.
        let moment_values =
            unsafe { std::slice::from_raw_parts(moments.contents().cast::<[f32; 4]>(), count) };
        let samples: Vec<u32> = values.iter().map(|v| v[3] as u32).collect();
        let pixels = values
            .iter()
            .map(|v| Vec3::new(v[0], v[1], v[2]) / v[3].max(1.0))
            .collect();
        let bad = moment_values.iter().map(|v| v[3] as u64).sum();
        let stats = stats(scene, s, "metal", device.name(), elapsed, &samples, bad);
        Ok(RenderOutput {
            pixels,
            samples,
            stats,
        })
    })
}
