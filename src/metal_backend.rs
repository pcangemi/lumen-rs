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
#[path = "metal_layout.rs"]
mod layout;
#[path = "metal_native.rs"]
mod native;
#[path = "metal_submission.rs"]
mod submission;
use layout::Geometry as IntersectionPrimitive;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    camera: Camera,
    sky: [f32; 4],
    horizon: [f32; 4],
    environment: [f32; 4],
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
pub(crate) fn command_error(command: &CommandBufferRef) -> String {
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
pub(crate) fn gpu_seconds(command: &CommandBufferRef) -> f64 {
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

/// Explicit experiments used by the quality-gated Metal benchmark.
/// Default is the original reference; ordinary rendering uses `production()` instead.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct MetalTuning {
    pub specialize_scene: bool,
    pub compact_stack: bool,
    pub compact_geometry: bool,
    pub split_geometry: bool,
    pub compact_nodes: bool,
    pub cached_bounds: bool,
    /// Experimental: does not pass the apartment exact-pixel gate.
    pub native_traversal: bool,
    pub wide_bvh: bool,
    /// Zero or one submits synchronously; two overlaps independent tile commands.
    pub command_window: usize,
    /// Zero keeps the compiler default; otherwise constrain its register allocation.
    pub threadgroup_limit: u64,
}

impl MetalTuning {
    /// Lossless configuration validated on the M1 Pro at delivery resolution.
    pub fn production() -> Self {
        Self {
            split_geometry: true,
            compact_nodes: true,
            cached_bounds: true,
            command_window: 2,
            ..Self::default()
        }
    }
}

/// Scene buffers and pipelines stay resident across camera changes.
/// All methods run on the owning render thread; submissions complete before buffer reuse.
pub struct MetalRenderer<'a> {
    scene: &'a Scene,
    command_window: usize,
    device: Device,
    pipeline: ComputePipelineState,
    preview_pipeline: ComputePipelineState,
    queue: CommandQueue,
    acceleration: Option<AccelerationStructure>,
    intersections: Option<IntersectionFunctionTable>,
    preview_intersections: Option<IntersectionFunctionTable>,
    primitives: Buffer,
    geometry: Buffer,
    nodes: Buffer,
    materials: Buffer,
    lights: Buffer,
    texels: Buffer,
    texture_info: Buffer,
    film: Buffer,
    moments: Buffer,
    pixel_count: usize,
    preview_film: Buffer,
    preview_pixel_count: usize,
}

fn render_impl(
    scene: &Scene,
    s: RenderSettings,
    control: &RenderControl,
    progressive: bool,
    on_tile: impl FnMut(TileUpdate),
) -> Result<RenderOutput> {
    s.validate()?;
    MetalRenderer::new(scene)?.render(scene.camera, s, control, progressive, on_tile)
}

impl<'a> MetalRenderer<'a> {
    /// Capture explicit diagnostic work; this is never enabled during ordinary rendering.
    pub fn start_capture(&self, path: &std::path::Path) -> Result<()> {
        let manager = CaptureManager::shared();
        ensure!(
            manager.supports_destination(MTLCaptureDestination::GpuTraceDocument),
            "GPU capture unavailable; launch with MTL_CAPTURE_ENABLED=1"
        );
        let descriptor = CaptureDescriptor::new();
        descriptor.set_capture_command_queue(&self.queue);
        descriptor.set_destination(MTLCaptureDestination::GpuTraceDocument);
        let path = std::path::absolute(path)?;
        ensure!(!path.exists(), "Capture output already exists");
        descriptor.set_output_url(path);
        manager
            .start_capture(&descriptor)
            .map_err(|e| anyhow!("Metal capture: {e}"))
    }

    /// Resident GPU scene storage, excluding film, compiler allocations and temporary build scratch.
    pub fn scene_buffer_bytes(&self) -> u64 {
        [
            &self.primitives,
            &self.geometry,
            &self.nodes,
            &self.materials,
            &self.lights,
            &self.texels,
            &self.texture_info,
        ]
        .iter()
        .map(|b| b.length())
        .sum::<u64>()
            + self.acceleration.as_ref().map_or(0, |a| a.allocated_size())
    }

    pub fn new(scene: &'a Scene) -> Result<Self> {
        Self::with_tuning(scene, MetalTuning::production())
    }

    pub fn with_tuning(scene: &'a Scene, tuning: MetalTuning) -> Result<Self> {
        ensure!(
            tuning.command_window <= 2,
            "At most two commands may be outstanding"
        );
        ensure!(
            matches!(tuning.threadgroup_limit, 0 | 32 | 64 | 128 | 256),
            "Unsupported threadgroup limit"
        );
        ensure!(
            !tuning.native_traversal || tuning.split_geometry,
            "Native traversal requires split geometry"
        );
        objc::rc::autoreleasepool(|| {
            let device =
                Device::system_default().ok_or_else(|| anyhow!("No Metal GPU is accessible"))?;
            let options = CompileOptions::new();
            options.set_fast_math_enabled(false);
            // Depth-first binary traversal needs at most one pending sibling per level,
            // plus the current node. A four-way node adds at most three pending
            // siblings per two binary levels: depth 60 fits in 128 entries.
            let stack_size = if tuning.wide_bvh {
                128
            } else if tuning.compact_stack {
                (scene.bvh.depth + 1).next_power_of_two()
            } else {
                64
            };
            ensure!(
                scene.bvh.depth <= 60,
                "BVH exceeds the supported traversal depth"
            );
            let source = format!(
                "#define LUMEN_BVH_STACK_SIZE {stack_size}\n#define LUMEN_WIDE_BVH {}\n#define LUMEN_NATIVE_TRAVERSAL {}\n#define LUMEN_SPLIT_GEOMETRY {}\n#define LUMEN_COMPACT_NODES {}\n#define LUMEN_CACHED_BOUNDS {}\n{}",
                u8::from(tuning.wide_bvh),
                u8::from(tuning.native_traversal),
                u8::from(tuning.split_geometry),
                u8::from(tuning.compact_nodes),
                u8::from(tuning.cached_bounds),
                include_str!("pathtrace.metal")
            );
            let library = device
                .new_library_with_source(&source, &options)
                .map_err(|e| anyhow!("Metal shader compilation: {e}"))?;
            let make_constants = || {
                let values = FunctionConstantValues::new();
                let volumes = !tuning.specialize_scene || scene.medium.is_some();
                let opacity =
                    !tuning.specialize_scene || scene.materials.iter().any(|m| m.maps[3] > 0);
                values.set_constant_value_at_index(
                    (&raw const volumes).cast(),
                    MTLDataType::Bool,
                    0,
                );
                values.set_constant_value_at_index(
                    (&raw const opacity).cast(),
                    MTLDataType::Bool,
                    1,
                );
                let compact_geometry = tuning.compact_geometry;
                values.set_constant_value_at_index(
                    (&raw const compact_geometry).cast(),
                    MTLDataType::Bool,
                    2,
                );
                values
            };
            let intersection_function = tuning
                .native_traversal
                .then(|| {
                    library
                        .get_function("native_intersection", Some(make_constants()))
                        .map_err(|e| anyhow!("Native intersection: {e}"))
                })
                .transpose()?;
            let make_pipeline = |name| -> Result<ComputePipelineState> {
                let constants = Some(make_constants());
                let function = library
                    .get_function(name, constants)
                    .map_err(|e| anyhow!("Kernel {name}: {e}"))?;
                if tuning.threadgroup_limit == 0 && !tuning.native_traversal {
                    return device
                        .new_compute_pipeline_state_with_function(&function)
                        .map_err(|e| anyhow!("Metal pipeline: {e}"));
                }
                let descriptor = ComputePipelineDescriptor::new();
                descriptor.set_compute_function(Some(&function));
                if tuning.threadgroup_limit > 0 {
                    descriptor.set_max_total_threads_per_threadgroup(tuning.threadgroup_limit);
                }
                if let Some(function) = &intersection_function {
                    let linked = LinkedFunctions::new();
                    linked.set_functions(&[function]);
                    descriptor.set_linked_functions(&linked);
                }
                if let Some(buffers) = descriptor.buffers() {
                    for index in [0, 1, 2, 3, 6, 7, 8, 9] {
                        if let Some(buffer) = buffers.object_at(index) {
                            buffer.set_mutability(MTLMutability::Immutable);
                        }
                    }
                }
                device
                    .new_compute_pipeline_state(&descriptor)
                    .map_err(|e| anyhow!("Metal pipeline: {e}"))
            };
            let pipeline = make_pipeline("pathtrace")?;
            let preview_pipeline = make_pipeline("navigation_preview")?;
            let intersections = intersection_function
                .as_ref()
                .map(|f| native::table(&pipeline, f))
                .transpose()?;
            let preview_intersections = intersection_function
                .as_ref()
                .map(|f| native::table(&preview_pipeline, f))
                .transpose()?;
            let queue = device.new_command_queue();
            let acceleration = tuning
                .native_traversal
                .then(|| native::build(&device, &queue, scene))
                .transpose()?;
            Ok(Self {
                command_window: tuning.command_window.max(1),
                pipeline,
                preview_pipeline,
                queue,
                acceleration,
                intersections,
                preview_intersections,
                primitives: if tuning.split_geometry {
                    buffer(
                        &device,
                        &scene
                            .primitives
                            .iter()
                            .map(layout::Attributes::from)
                            .collect::<Vec<_>>(),
                    )
                } else {
                    buffer(&device, &scene.primitives)
                },
                geometry: if tuning.compact_geometry || tuning.split_geometry {
                    let values: Vec<_> = scene
                        .primitives
                        .iter()
                        .map(|p| IntersectionPrimitive {
                            p0: p.p0,
                            p1: p.p1,
                            p2: p.p2,
                            info: p.info,
                        })
                        .collect();
                    buffer(&device, &values)
                } else {
                    buffer::<IntersectionPrimitive>(&device, &[])
                },
                nodes: if tuning.wide_bvh {
                    buffer(&device, &layout::wide_nodes(&scene.bvh.nodes)?)
                } else if tuning.compact_nodes {
                    buffer(&device, &layout::compact_nodes(&scene.bvh.nodes)?)
                } else {
                    buffer(&device, &scene.bvh.nodes)
                },
                materials: buffer(&device, &scene.materials),
                lights: buffer(&device, &scene.lights),
                texels: buffer(&device, &scene.textures.pixels),
                texture_info: buffer(&device, &scene.textures.descriptors),
                film: buffer::<[f32; 4]>(&device, &[]),
                moments: buffer::<[f32; 4]>(&device, &[]),
                pixel_count: 0,
                preview_film: buffer::<[f32; 4]>(&device, &[]),
                preview_pixel_count: 0,
                device,
                scene,
            })
        })
    }

    fn prepare_film(&mut self, count: usize) {
        if self.pixel_count != count {
            let bytes = (count * std::mem::size_of::<[f32; 4]>()) as u64;
            self.film = self
                .device
                .new_buffer(bytes, MTLResourceOptions::StorageModeShared);
            self.moments = self
                .device
                .new_buffer(bytes, MTLResourceOptions::StorageModeShared);
            self.pixel_count = count;
        }
        // SAFETY: Buffers contain count float4 records; all prior submissions completed.
        // Clearing both buffers prevents old views or adaptive convergence flags leaking.
        unsafe {
            std::ptr::write_bytes(self.film.contents().cast::<[f32; 4]>(), 0, count);
            std::ptr::write_bytes(self.moments.contents().cast::<[f32; 4]>(), 0, count);
        }
    }

    pub fn render(
        &mut self,
        camera: Camera,
        s: RenderSettings,
        control: &RenderControl,
        progressive: bool,
        mut on_tile: impl FnMut(TileUpdate),
    ) -> Result<RenderOutput> {
        s.validate()?;
        let count = s.width as usize * s.height as usize;
        self.prepare_film(count);
        let scene = self.scene;
        let Self {
            device,
            pipeline,
            queue,
            primitives,
            geometry,
            nodes,
            materials,
            lights,
            texels,
            texture_info,
            film,
            moments,
            ..
        } = &*self;
        objc::rc::autoreleasepool(|| {
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
                for (chunk_index, chunk) in tiles.chunks(self.command_window).enumerate() {
                    if !control.wait_until_ready() {
                        break 'passes;
                    }
                    let mut pending = submission::SubmittedBatch::new(chunk.len());
                    for tile in chunk {
                        let params = Params {
                            camera,
                            sky: pack_vec3(scene.sky, 0.0),
                            horizon: pack_vec3(scene.horizon, 0.0),
                            environment: scene.packed_environment(),
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
                        objc::rc::autoreleasepool(|| {
                            let cb = queue.new_command_buffer();
                            let encoder = cb.new_compute_command_encoder();
                            encoder.set_compute_pipeline_state(pipeline);
                            for (i, b) in [primitives, nodes, materials, lights, film, moments]
                                .iter()
                                .enumerate()
                            {
                                encoder.set_buffer(i as u64, Some(b), 0);
                            }
                            encoder.set_buffer(7, Some(texels), 0);
                            encoder.set_buffer(8, Some(texture_info), 0);
                            encoder.set_buffer(9, Some(geometry), 0);
                            if let Some(acceleration) = &self.acceleration {
                                encoder.set_acceleration_structure(10, Some(acceleration));
                                encoder.set_intersection_function_table(
                                    11,
                                    self.intersections.as_deref(),
                                );
                            }
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
                            pending.push(cb);
                        });
                    }
                    for (local_index, (tile, cb)) in
                        chunk.iter().zip(pending.commands()).enumerate()
                    {
                        let tile_index = chunk_index * self.command_window + local_index;
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
                        let seconds = gpu_seconds(cb);
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
                        let mut pixels = Vec::with_capacity(tile.pixel_count());
                        for local in 0..tile.pixel_count() {
                            let index = tile.image_index(local, s.width);
                            // SAFETY: Only this completed tile is read. Another command may
                            // write a disjoint tile, so do not borrow the entire film as a slice.
                            let value =
                                unsafe { film.contents().cast::<[f32; 4]>().add(index).read() };
                            let samples = value[3] as u32;
                            completed_samples += u64::from(samples - previous_samples[index]);
                            previous_samples[index] = samples;
                            pixels
                                .push(Vec3::new(value[0], value[1], value[2]) / value[3].max(1.0));
                        }
                        completed_work += tile.pixel_count() as u64 * u64::from(pass_end - sample);
                        on_tile(TileUpdate {
                            tile: *tile,
                            pixels,
                            completed_samples,
                            progress: completed_work as f32 / (count as f32 * s.samples as f32),
                            pass_samples: pass_end,
                            pass_complete: tile_index + 1 == tiles.len(),
                            elapsed: start.elapsed(),
                        });
                    }
                }
                // Grow at most twofold; shrink immediately after an expensive pass. Timing
                // is a scheduling heuristic, not a hard guarantee about future GPU latency.
                let suggested = (f64::from(sample_batch) * TARGET_COMMAND_SECONDS
                    / slowest_command.max(1e-6)) as u32;
                sample_batch = suggested.clamp(1, sample_batch * 2).min(s.batch).min(16);
                sample = pass_end;
            }
            ensure!(
                !(control.discard_cancelled
                    && control.cancelled.load(std::sync::atomic::Ordering::Relaxed)),
                "Render cancelled"
            );
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

    /// Fast camera composition preview, using the same full-resolution scene geometry.
    pub fn preview(
        &mut self,
        camera: Camera,
        width: u32,
        height: u32,
        control: &RenderControl,
    ) -> Result<Option<Vec<Vec3>>> {
        ensure!(
            width > 0 && height > 0 && width <= 1920 && height <= 1920,
            "Invalid preview dimensions"
        );
        let count = width as usize * height as usize;
        if self.preview_pixel_count != count {
            self.preview_film = self.device.new_buffer(
                (count * std::mem::size_of::<[f32; 4]>()) as u64,
                MTLResourceOptions::StorageModeShared,
            );
            self.preview_pixel_count = count;
        }
        let threads = (self.preview_pipeline.thread_execution_width() * 4)
            .min(self.preview_pipeline.max_total_threads_per_threadgroup());
        for tile in tiles_with_size(width, height, 128) {
            if !control.wait_until_ready() {
                return Ok(None);
            }
            let params = Params {
                camera,
                sky: pack_vec3(self.scene.sky, 0.0),
                horizon: pack_vec3(self.scene.horizon, 0.0),
                environment: self.scene.packed_environment(),
                image: [width, height, 0, 1],
                config: [0; 4],
                adaptive: [0.0; 4],
                tile: [tile.x, tile.y, tile.width, tile.height],
                medium: crate::medium::Medium::zeroed(),
            };
            objc::rc::autoreleasepool(|| -> Result<()> {
                let cb = self.queue.new_command_buffer();
                let encoder = cb.new_compute_command_encoder();
                encoder.set_compute_pipeline_state(&self.preview_pipeline);
                if let Some(acceleration) = &self.acceleration {
                    encoder.set_acceleration_structure(10, Some(acceleration));
                    encoder
                        .set_intersection_function_table(11, self.preview_intersections.as_deref());
                }
                for (index, resource) in [
                    (0, &self.primitives),
                    (1, &self.nodes),
                    (2, &self.materials),
                    (4, &self.preview_film),
                    (7, &self.texels),
                    (8, &self.texture_info),
                    (9, &self.geometry),
                ] {
                    encoder.set_buffer(index, Some(resource), 0);
                }
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
                    "Metal navigation preview failed: {}",
                    command_error(cb)
                );
                Ok(())
            })?;
        }
        // SAFETY: Every tile completed; the buffer contains exactly count float4s.
        let pixels = unsafe {
            std::slice::from_raw_parts(self.preview_film.contents().cast::<[f32; 4]>(), count)
        };
        Ok(Some(
            pixels.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect(),
        ))
    }
}
