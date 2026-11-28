//! Native Metal BVH construction. Custom intersections retain the renderer's
//! exact primitive equations, including millimetre geometry and alpha cutouts.
use super::{buffer, command_error};
use crate::scene::Scene;
use anyhow::{Result, ensure};
use metal::*;

pub(super) fn build(
    device: &Device,
    queue: &CommandQueue,
    scene: &Scene,
) -> Result<AccelerationStructure> {
    ensure!(
        device.supports_raytracing(),
        "This device does not support Metal ray tracing"
    );
    let bounds: Vec<[f32; 6]> = scene
        .primitives
        .iter()
        .map(|p| {
            let (lo, hi) = p.bounds();
            [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]
        })
        .collect();
    let bounds = buffer(device, &bounds);
    let geometry = AccelerationStructureBoundingBoxGeometryDescriptor::descriptor();
    geometry.set_bounding_box_buffer(Some(&bounds));
    geometry.set_bounding_box_count(scene.primitives.len() as u64);
    geometry.set_bounding_box_stride(24);
    geometry.set_intersection_function_table_offset(0);
    geometry.set_opaque(false);
    let descriptor = PrimitiveAccelerationStructureDescriptor::descriptor();
    let geometry: &AccelerationStructureGeometryDescriptorRef = &geometry;
    descriptor.set_geometry_descriptors(Array::from_slice(&[geometry]));
    let sizes = device.acceleration_structure_sizes_with_descriptor(&descriptor);
    let acceleration =
        device.new_acceleration_structure_with_size(sizes.acceleration_structure_size);
    let scratch = device.new_buffer(
        sizes.build_scratch_buffer_size,
        MTLResourceOptions::StorageModePrivate,
    );
    let command = queue.new_command_buffer();
    let encoder = command.new_acceleration_structure_command_encoder();
    encoder.build_acceleration_structure(&acceleration, &descriptor, &scratch, 0);
    encoder.end_encoding();
    command.commit();
    command.wait_until_completed();
    ensure!(
        command.status() == MTLCommandBufferStatus::Completed,
        "Metal acceleration build: {}",
        command_error(command)
    );
    Ok(acceleration)
}

pub(super) fn table(
    pipeline: &ComputePipelineState,
    function: &Function,
) -> Result<IntersectionFunctionTable> {
    let descriptor = IntersectionFunctionTableDescriptor::new();
    descriptor.set_function_count(1);
    let table = pipeline.new_intersection_function_table_with_descriptor(&descriptor);
    let handle = pipeline
        .function_handle_with_function(function)
        .ok_or_else(|| anyhow::anyhow!("Missing native intersection function handle"))?;
    table.set_function(handle, 0);
    Ok(table)
}
