//! Native Metal compute backend. Buffers use explicit 16-byte records shared with MSL.
use crate::progressive::{RenderControl, TileUpdate, tiles_with_size};
use crate::{
    camera::Camera,
    geometry::pack_vec3,
    render::{GpuDispatchStats, RenderOutput, RenderSettings, stats},
    scene::Scene,
};
use anyhow::{Result, anyhow, ensure};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use metal::*;
use objc::{msg_send, sel, sel_impl};
use std::ffi::CStr;
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

#[allow(
    unexpected_cfgs,
    reason = "objc 0.2 macros contain legacy cargo-clippy feature checks"
)]
fn command_error(command: &CommandBufferRef) -> String {
    // SAFETY: Metal owns the completed command and its NSError for this autorelease pool.
    // Both Objective-C selectors return the documented NSError/NSString types.
    unsafe {
        let error: *mut objc::runtime::Object = msg_send![command, error];
        if error.is_null() {
            return format!("status {:?}, no diagnostic supplied", command.status());
        }
        let code: isize = msg_send![error, code];
        let description: *mut objc::runtime::Object = msg_send![error, localizedDescription];
        let message: *const std::ffi::c_char = msg_send![description, UTF8String];
        if message.is_null() {
            return format!("error code {code}");
        }
        format!(
            "error code {code}: {}",
            CStr::from_ptr(message).to_string_lossy()
        )
    }
}

#[allow(
    unexpected_cfgs,
    reason = "objc 0.2 macros contain legacy cargo-clippy feature checks"
)]
fn gpu_seconds(command: &CommandBufferRef) -> f64 {
    // SAFETY: Call only after completion. These Metal selectors return CFTimeInterval
    // (f64) and are supported on macOS 10.15 and every Apple Silicon Mac.
    unsafe {
        let start: f64 = msg_send![command, GPUStartTime];
        let end: f64 = msg_send![command, GPUEndTime];
        (end - start).max(0.0)
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
        // Start conservatively, then use the slowest tile's GPU time to size the next
        // pass. This limits individual submissions without sleeping or dropping samples.
        const TARGET_COMMAND_SECONDS: f64 = 0.020;
        // More independent paths per dispatch improve GPU utilization. Retain
        // short, timed submissions rather than dispatching the entire film.
        const TILE_SIZE: u32 = 128;
        let mut sample_batch = 1;
        let mut sample = 0;
        let mut dispatch_stats = GpuDispatchStats {
            command_count: 0,
            gpu_seconds: 0.0,
            max_command_seconds: 0.0,
            max_samples_per_command: 0,
            threads_per_threadgroup: threads,
            tile_size: TILE_SIZE,
        };
        let tiles = tiles_with_size(s.width, s.height, TILE_SIZE);
        'passes: while sample < s.samples {
            let pass_end = s.samples.min(sample + sample_batch);
            let mut slowest_command: f64 = 0.0;
            for (tile_index, &tile) in tiles.iter().enumerate() {
                if !control.wait_until_ready() {
                    break 'passes;
                }
                let params = Params {
                    camera: scene.camera,
                    sky: pack_vec3(scene.sky, 0.0),
                    horizon: pack_vec3(scene.horizon, 0.0),
                    image: [s.width, s.height, sample, pass_end],
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
                let seconds = objc::rc::autoreleasepool(|| -> Result<f64> {
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
                        "Metal command buffer failed: {} (tile {}, {}; samples {}..{})",
                        command_error(cb),
                        tile.x,
                        tile.y,
                        sample,
                        pass_end
                    );
                    Ok(gpu_seconds(cb))
                })?;
                slowest_command = slowest_command.max(seconds);
                dispatch_stats.command_count += 1;
                dispatch_stats.gpu_seconds += seconds;
                dispatch_stats.max_command_seconds =
                    dispatch_stats.max_command_seconds.max(seconds);
                dispatch_stats.max_samples_per_command = dispatch_stats
                    .max_samples_per_command
                    .max(pass_end - sample);
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
            // Grow at most twofold; shrink immediately after an expensive pass. Timing
            // is a scheduling heuristic, not a hard guarantee about future GPU latency.
            let suggested = (f64::from(sample_batch) * TARGET_COMMAND_SECONDS
                / slowest_command.max(1e-6)) as u32;
            sample_batch = suggested.clamp(1, sample_batch * 2).min(s.batch).min(16);
            sample = pass_end;
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
        let mut stats = stats(scene, s, "metal", device.name(), elapsed, &samples, bad);
        stats.gpu_dispatch = Some(dispatch_stats);
        Ok(RenderOutput {
            pixels,
            samples,
            stats,
        })
    })
}
