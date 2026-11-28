//! Compare compiler variants using identical samples, geometry and raw linear pixels.
#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use anyhow::ensure;
    use clap::Parser;
    use lumen_rs::{
        metal_backend::{MetalRenderer, MetalTuning},
        progressive::RenderControl,
        render::RenderSettings,
        scene,
    };
    use std::time::Instant;
    #[derive(Parser)]
    struct Args {
        #[arg(long, default_value = "interior-neon")]
        scene: String,
        #[arg(long, default_value_t = 640)]
        width: u32,
        #[arg(long, default_value_t = 360)]
        height: u32,
        #[arg(long, default_value_t = 16)]
        samples: u32,
        #[arg(long, default_value_t = 50)]
        bounces: u32,
        #[arg(long, default_value_t = 3)]
        repeats: usize,
        /// Comma-separated variant names; omit to evaluate every variant.
        #[arg(long)]
        variants: Option<String>,
        #[arg(long, default_value = "renders/metal-tuning.json")]
        output: std::path::PathBuf,
    }
    let args = Args::parse();
    ensure!(args.repeats > 0, "At least one repeat is required");
    let settings = RenderSettings {
        width: args.width,
        height: args.height,
        samples: args.samples,
        max_depth: args.bounces,
        adaptive: 0.0,
        ..Default::default()
    };
    settings.validate()?;
    let scene = scene::preset(&args.scene, args.width as f32 / args.height as f32)?;
    let variants = [
        ("generic", MetalTuning::default()),
        (
            "compact-geometry",
            MetalTuning {
                compact_geometry: true,
                ..Default::default()
            },
        ),
        (
            "specialized",
            MetalTuning {
                specialize_scene: true,
                ..Default::default()
            },
        ),
        (
            "compact-stack",
            MetalTuning {
                specialize_scene: true,
                compact_stack: true,
                ..Default::default()
            },
        ),
        (
            "compact-128",
            MetalTuning {
                specialize_scene: true,
                compact_stack: true,
                compact_geometry: false,
                threadgroup_limit: 128,
            },
        ),
        (
            "compact-64",
            MetalTuning {
                specialize_scene: true,
                compact_stack: true,
                compact_geometry: false,
                threadgroup_limit: 64,
            },
        ),
    ];
    let selected: Vec<usize> = variants
        .iter()
        .enumerate()
        .filter(|(_, (name, _))| {
            args.variants
                .as_ref()
                .is_none_or(|names| names.split(',').any(|n| n == *name))
        })
        .map(|(index, _)| index)
        .collect();
    ensure!(
        !selected.is_empty() && selected.contains(&0),
        "Selected variants must include generic as the reference"
    );
    let control = RenderControl::default();
    let mut reference = None;
    let mut records = Vec::new();
    for repeat in 0..args.repeats {
        let order: Vec<_> = if repeat % 2 == 0 {
            selected.clone()
        } else {
            selected.iter().rev().copied().collect()
        };
        for index in order {
            let (name, tuning) = variants[index];
            let start = Instant::now();
            let mut renderer = MetalRenderer::with_tuning(&scene, tuning)?;
            let setup_seconds = start.elapsed().as_secs_f64();
            // Warm the pipeline and resident buffers without including that pass in timings.
            renderer.render(
                scene.camera,
                RenderSettings {
                    width: 32,
                    height: 18,
                    samples: 1,
                    ..settings
                },
                &control,
                false,
                |_| {},
            )?;
            let result = renderer.render(scene.camera, settings, &control, false, |_| {})?;
            ensure!(
                result.stats.invalid_samples == 0
                    && result.samples.iter().all(|&s| s == settings.samples),
                "Incomplete or invalid render"
            );
            let expected = reference.get_or_insert_with(|| result.pixels.clone());
            let max_difference = result
                .pixels
                .iter()
                .zip(expected.iter())
                .map(|(a, b)| (*a - *b).abs().max_element())
                .fold(0.0f32, f32::max);
            let exact = result.pixels == *expected;
            println!(
                "{name} repeat {repeat}: {:.3}s; exact={exact}; max difference={max_difference:.9}",
                result.stats.render_seconds
            );
            records.push(serde_json::json!({"variant": name, "repeat": repeat, "tuning": tuning, "setup_seconds": setup_seconds,
                "render_seconds": result.stats.render_seconds, "gpu": result.stats.gpu_dispatch, "exact_pixels": exact, "max_difference": max_difference}));
            std::fs::create_dir_all(args.output.parent().unwrap_or(std::path::Path::new(".")))?;
            std::fs::write(
                &args.output,
                serde_json::to_vec_pretty(
                    &serde_json::json!({"scene": args.scene, "width": args.width, "height": args.height,
                "samples": args.samples, "bounces": args.bounces, "primitives": scene.primitives.len(), "bvh_depth": scene.bvh.depth, "device": lumen_rs::metal_backend::available(), "runs": records}),
                )?,
            )?;
        }
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This benchmark requires macOS and Metal.");
}
