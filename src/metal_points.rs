//! Depth-tested Metal point sprites, isolated from the physically based renderer.
use crate::{camera::Camera, point_cloud::PointCloud};
use anyhow::{Result, anyhow, ensure};
use metal::*;

pub struct PointRenderer {
    device: Device,
    queue: CommandQueue,
    pipeline: RenderPipelineState,
    occlusion_pipeline: RenderPipelineState,
    depth_state: DepthStencilState,
    points: Buffer,
    point_count: usize,
    pub last_gpu_seconds: f64,
    target: Option<Target>,
}
struct Target {
    width: u32,
    height: u32,
    color: Texture,
    depth: Texture,
    readback: Buffer,
    row_bytes: usize,
}
impl PointRenderer {
    pub fn new(cloud: &PointCloud) -> Result<Self> {
        objc::rc::autoreleasepool(|| {
            let device = Device::system_default().ok_or_else(|| anyhow!("No Metal device"))?;
            let library = device
                .new_library_with_source(include_str!("point_cloud.metal"), &CompileOptions::new())
                .map_err(|e| anyhow!("Point shader: {e}"))?;
            let descriptor = RenderPipelineDescriptor::new();
            let vertex = library
                .get_function("cloud_vertex", None)
                .map_err(|e| anyhow!(e))?;
            let fragment = library
                .get_function("cloud_fragment", None)
                .map_err(|e| anyhow!(e))?;
            descriptor.set_vertex_function(Some(&vertex));
            descriptor.set_fragment_function(Some(&fragment));
            descriptor
                .color_attachments()
                .object_at(0)
                .unwrap()
                .set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            descriptor.set_depth_attachment_pixel_format(MTLPixelFormat::Depth32Float);
            let pipeline = device
                .new_render_pipeline_state(&descriptor)
                .map_err(|e| anyhow!("Point pipeline: {e}"))?;

            let occlusion_pipeline = device
                .new_render_pipeline_state(&descriptor)
                .map_err(|e| anyhow!("Point depth pipeline: {e}"))?;
            let depth = DepthStencilDescriptor::new();
            depth.set_depth_compare_function(MTLCompareFunction::Less);
            depth.set_depth_write_enabled(true);
            let points = if cloud.points.is_empty() {
                device.new_buffer(32, MTLResourceOptions::StorageModeShared)
            } else {
                device.new_buffer_with_data(
                    cloud.points.as_ptr().cast(),
                    std::mem::size_of_val(cloud.points.as_slice()) as u64,
                    MTLResourceOptions::StorageModeShared,
                )
            };
            Ok(Self {
                queue: device.new_command_queue(),
                depth_state: device.new_depth_stencil_state(&depth),
                device,
                pipeline,
                occlusion_pipeline,
                points,
                point_count: cloud.points.len(),
                last_gpu_seconds: 0.0,
                target: None,
            })
        })
    }
    pub fn render(
        &mut self,
        camera: Camera,
        width: u32,
        height: u32,
        budget: usize,
    ) -> Result<Vec<u32>> {
        ensure!(
            width > 0 && height > 0 && width <= 1920 && height <= 1920,
            "Invalid point preview dimensions"
        );
        objc::rc::autoreleasepool(|| {
            if self
                .target
                .as_ref()
                .is_none_or(|target| target.width != width || target.height != height)
            {
                let make_texture = |format| {
                    let descriptor = TextureDescriptor::new();
                    descriptor.set_width(u64::from(width));
                    descriptor.set_height(u64::from(height));
                    descriptor.set_pixel_format(format);
                    descriptor.set_storage_mode(MTLStorageMode::Private);
                    descriptor.set_usage(MTLTextureUsage::RenderTarget);
                    self.device.new_texture(&descriptor)
                };
                let row_bytes = (width as usize * 4).div_ceil(256) * 256;
                self.target = Some(Target {
                    width,
                    height,
                    color: make_texture(MTLPixelFormat::BGRA8Unorm),
                    depth: make_texture(MTLPixelFormat::Depth32Float),
                    readback: self.device.new_buffer(
                        (row_bytes * height as usize) as u64,
                        MTLResourceOptions::StorageModeShared,
                    ),
                    row_bytes,
                });
            }
            let target = self.target.as_ref().unwrap();
            let pass = RenderPassDescriptor::new();
            let color = pass.color_attachments().object_at(0).unwrap();
            color.set_texture(Some(&target.color));
            color.set_load_action(MTLLoadAction::Clear);
            color.set_store_action(MTLStoreAction::Store);
            color.set_clear_color(MTLClearColor::new(
                16.0 / 255.0,
                25.0 / 255.0,
                33.0 / 255.0,
                1.0,
            ));
            let depth = pass.depth_attachment().unwrap();
            depth.set_texture(Some(&target.depth));
            depth.set_load_action(MTLLoadAction::Clear);
            depth.set_store_action(MTLStoreAction::DontCare);
            depth.set_clear_depth(1.0);
            let command = self.queue.new_command_buffer();
            let encoder = command.new_render_command_encoder(pass);
            encoder.set_render_pipeline_state(&self.occlusion_pipeline);
            encoder.set_depth_stencil_state(&self.depth_state);
            encoder.set_vertex_buffer(0, Some(&self.points), 0);
            encoder.set_vertex_bytes(
                1,
                std::mem::size_of::<Camera>() as u64,
                (&camera as *const Camera).cast(),
            );
            // A wider depth-only splat closes small gaps between points. The visible
            // dots stay crisp; a small camera-space tolerance retains coplanar neighbours.
            let occlusion_style = [4.0f32, 0.0];
            encoder.set_vertex_bytes(2, 8, occlusion_style.as_ptr().cast());
            encoder.draw_primitives(
                MTLPrimitiveType::Point,
                0,
                budget.min(self.point_count) as u64,
            );
            encoder.set_render_pipeline_state(&self.pipeline);
            let visible_style = [1.0f32, 0.05];
            encoder.set_vertex_bytes(2, 8, visible_style.as_ptr().cast());
            encoder.draw_primitives(
                MTLPrimitiveType::Point,
                0,
                budget.min(self.point_count) as u64,
            );
            encoder.end_encoding();
            let blit = command.new_blit_command_encoder();
            blit.copy_from_texture_to_buffer(
                &target.color,
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize::new(u64::from(width), u64::from(height), 1),
                &target.readback,
                0,
                target.row_bytes as u64,
                (target.row_bytes * height as usize) as u64,
                MTLBlitOption::None,
            );
            blit.end_encoding();
            command.commit();
            command.wait_until_completed();
            ensure!(
                command.status() == MTLCommandBufferStatus::Completed,
                "Metal point preview failed: {}",
                crate::metal_backend::command_error(command)
            );
            self.last_gpu_seconds = crate::metal_backend::gpu_seconds(command);
            // SAFETY: The blit has completed, and the buffer includes every padded row.
            let values = unsafe {
                std::slice::from_raw_parts(
                    target.readback.contents().cast::<u32>(),
                    target.row_bytes / 4 * height as usize,
                )
            };
            let mut pixels = Vec::with_capacity(width as usize * height as usize);
            for row in values.chunks_exact(target.row_bytes / 4) {
                pixels.extend(
                    row[..width as usize]
                        .iter()
                        .map(|value| *value & 0x00ff_ffff),
                );
            }
            Ok(pixels)
        })
    }
}
