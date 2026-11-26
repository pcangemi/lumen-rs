//! Compare linear RGB images without tone mapping or exposure normalization.
use anyhow::{Result, ensure};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Measure linear RGB error against a high-sample reference")]
struct Args {
    reference: PathBuf,
    candidate: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
    /// Save the candidate using Lumen's display transform for visual inspection.
    #[arg(long)]
    preview: Option<PathBuf>,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let reference = image::open(&args.reference)?.to_rgb32f();
    let candidate = image::open(&args.candidate)?.to_rgb32f();
    ensure!(
        reference.dimensions() == candidate.dimensions(),
        "Image dimensions differ"
    );
    let mut squared_error = 0.0f64;
    let mut reference_energy = 0.0f64;
    let mut absolute_error = 0.0f64;
    let mut maximum_error = 0.0f64;
    let mut pixel_errors = Vec::with_capacity(reference.len() / 3);
    for (a, b) in reference.pixels().zip(candidate.pixels()) {
        let mut pixel_maximum = 0.0f64;
        for channel in 0..3 {
            let a = f64::from(a[channel]);
            let b = f64::from(b[channel]);
            ensure!(a.is_finite() && b.is_finite(), "Non-finite image samples");
            let error = (a - b).abs();
            squared_error += error * error;
            reference_energy += a * a;
            absolute_error += error;
            pixel_maximum = pixel_maximum.max(error);
        }
        maximum_error = maximum_error.max(pixel_maximum);
        pixel_errors.push(pixel_maximum);
    }
    pixel_errors.sort_unstable_by(f64::total_cmp);
    let channels = reference.len() as f64;
    let report = serde_json::json!({
        "reference": args.reference, "candidate": args.candidate,
        "width": reference.width(), "height": reference.height(),
        "linear_rmse": (squared_error / channels).sqrt(),
        "relative_l2": (squared_error / reference_energy.max(1e-30)).sqrt(),
        "mean_absolute_error": absolute_error / channels,
        "maximum_absolute_error": maximum_error,
        "pixel_absolute_error_p99": pixel_errors[(pixel_errors.len() - 1) * 99 / 100],
        "note": "Use raw linear EXR inputs. Denoised results must be reported separately."
    });
    let text = serde_json::to_string_pretty(&report)?;
    if let Some(output) = args.output {
        std::fs::write(output, &text)?;
    }
    if let Some(path) = args.preview {
        let display = image::RgbImage::from_fn(candidate.width(), candidate.height(), |x, y| {
            let linear = glam::Vec3::from(candidate.get_pixel(x, y).0);
            let mapped = lumen_rs::output::tonemap(linear, 0.0);
            image::Rgb(mapped.to_array().map(|value| (value * 255.0 + 0.5) as u8))
        });
        display.save(path)?;
    }
    println!("{text}");
    Ok(())
}
