use anyhow::{Result, bail, ensure};
use clap::{Parser, ValueEnum};
use lumen_rs::{
    render::{self, RenderSettings},
    scene,
};
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Backend {
    Auto,
    Cpu,
    Metal,
}
#[derive(Parser, Debug)]
#[command(
    name = "lumen",
    version,
    about = "Lumen — Rust / Apple Metal path tracing engine"
)]
struct Args {
    #[arg(long, default_value = "studio")]
    scene: String,
    #[arg(long)]
    scene_file: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "auto")]
    backend: Backend,
    #[arg(long, default_value_t = 960)]
    width: u32,
    #[arg(long, default_value_t = 640)]
    height: u32,
    #[arg(long, default_value_t = 256)]
    samples: u32,
    #[arg(long, default_value_t = 12)]
    bounces: u32,
    #[arg(long, default_value_t = 42)]
    seed: u32,
    #[arg(long, default_value_t = 0.0)]
    adaptive: f32,
    #[arg(long, default_value_t = 64)]
    min_samples: u32,
    #[arg(long, default_value_t = 16)]
    batch: u32,
    #[arg(long, default_value_t = 0.0)]
    exposure: f32,
    #[arg(long)]
    threads: Option<usize>,
    #[arg(short, long, default_value = "renders/render.png")]
    output: PathBuf,
    #[arg(long)]
    exr: bool,
    /// Denoise with Open Image Denoise; also preserve the raw render.
    #[arg(long)]
    denoise: bool,
    #[arg(long)]
    obj: Option<PathBuf>,
    #[arg(long, default_value_t = 1.0)]
    obj_scale: f32,
    #[arg(long)]
    list_devices: bool,
    /// Export the assembled scene for the Cycles comparison script, without rendering.
    #[arg(long)]
    export_comparison: Option<PathBuf>,
}
fn main() -> Result<()> {
    let args = Args::parse();
    if args.list_devices {
        println!(
            "CPU: {} available threads",
            std::thread::available_parallelism()
                .map(|x| x.get())
                .unwrap_or(1)
        );
        #[cfg(target_os = "macos")]
        println!(
            "Metal: {}",
            lumen_rs::metal_backend::available()
                .unwrap_or_else(|| "no GPU accessible to this process".into())
        );
        return Ok(());
    }
    ensure!(
        args.exposure.is_finite() && args.exposure.abs() <= 30.0,
        "Exposure must be between -30 and +30 EV"
    );
    ensure!(
        args.output.extension().and_then(|s| s.to_str()) == Some("png"),
        "Output must use the .png extension; --exr also writes a linear image"
    );
    if let Some(n) = args.threads {
        ensure!(n > 0, "Thread count must be positive");
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()?;
    }
    let settings = RenderSettings {
        width: args.width,
        height: args.height,
        samples: args.samples,
        max_depth: args.bounces,
        seed: args.seed,
        adaptive: args.adaptive,
        min_samples: args.min_samples,
        batch: args.batch,
    };
    settings.validate()?;
    let aspect = args.width as f32 / args.height as f32;
    let mut scene = if let Some(path) = args.scene_file {
        scene::load(&path, aspect)?
    } else {
        scene::preset(&args.scene, aspect)?
    };
    if let Some(path) = args.obj {
        scene.import_obj(&path, glam::Vec3::ZERO, args.obj_scale, 0.0, None)?;
        scene.finish()?;
    }
    if let Some(path) = args.export_comparison {
        lumen_rs::comparison::export(&scene, settings, &path)?;
        println!("Comparison scene: {}", path.join("scene.json").display());
        return Ok(());
    }
    eprintln!(
        "{} primitives | {} BVH nodes | {} lights | {} x {} | {} spp",
        scene.primitives.len(),
        scene.bvh.nodes.len(),
        scene.lights.len(),
        args.width,
        args.height,
        args.samples
    );
    let use_metal = match args.backend {
        Backend::Cpu => false,
        Backend::Metal => true,
        Backend::Auto => {
            #[cfg(target_os = "macos")]
            {
                lumen_rs::metal_backend::available().is_some()
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        }
    };
    let mut result = if use_metal {
        #[cfg(target_os = "macos")]
        {
            lumen_rs::metal_backend::render(&scene, settings)?
        }
        #[cfg(not(target_os = "macos"))]
        {
            bail!("The Metal backend requires macOS")
        }
    } else {
        if matches!(args.backend, Backend::Auto) {
            eprintln!("Automatic backend: CPU (Metal is unavailable to this process)");
        }
        render::cpu(&scene, settings)?
    };
    if result.stats.invalid_samples > 0 {
        bail!(
            "{} non-finite samples: render rejected",
            result.stats.invalid_samples
        )
    }
    if args.denoise {
        let stem = args.output.file_stem().unwrap().to_string_lossy();
        let raw_path = args.output.with_file_name(format!("{stem}-raw.png"));
        lumen_rs::output::save(&result, &raw_path, args.exposure, args.exr)?;
        let denoise_started = std::time::Instant::now();
        let denoiser = lumen_rs::denoise::Denoiser::new()?;
        let guides = lumen_rs::denoise::Guides::new(&scene, settings.width, settings.height);
        result.pixels =
            denoiser.filter(&result.pixels, &guides, settings.width, settings.height)?;
        result.stats.denoiser = Some("Open Image Denoise 2.x / CPU / high".into());
        result.stats.denoise_seconds = denoise_started.elapsed().as_secs_f64();
    }
    lumen_rs::output::save(&result, &args.output, args.exposure, args.exr)?;
    println!(
        "{} | {:.2} s | {:.2} M camera samples/s | {:.1} average spp -> {}",
        result.stats.backend,
        result.stats.render_seconds,
        result.stats.camera_samples_per_second / 1e6,
        result.stats.average_spp,
        args.output.display()
    );
    Ok(())
}
