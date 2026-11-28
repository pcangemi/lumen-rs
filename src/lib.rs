//! Lumen: a physically based path tracer with CPU and native Metal backends.
//!
//! Scene units are meters, radiance is linear RGB, and directions are normalized.
//! GPU records have explicit layouts; transport code uses stack storage per path.
#![deny(unsafe_op_in_unsafe_fn)]

pub mod bvh;
pub mod camera;
pub mod geometry;
pub mod material;
#[cfg(target_os = "macos")]
pub mod metal_backend;
pub mod navigation;
pub mod render;
pub mod rng;
pub mod scene;

pub mod integrator;
pub mod output;
pub mod progressive;
pub mod viewer_ui;

pub mod denoise;

pub mod texture;

pub mod medium;

pub mod comparison;

#[cfg(target_os = "macos")]
pub mod metal_points;
pub mod point_cloud;

pub mod frame_pacing;
