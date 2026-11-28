//! Measure point-cloud navigation including CPU interface drawing, with optional presentation.
#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use clap::Parser;
    use lumen_rs::{
        metal_points::PointRenderer,
        navigation::Navigator,
        point_cloud::PointCloud,
        scene,
        viewer_ui::{self, ViewerState},
    };
    use std::time::Instant;
    #[derive(Parser)]
    struct Args {
        #[arg(long, default_value = "interior-neon")]
        scene: String,
        #[arg(long, default_value_t = 960)]
        preview_width: u32,
        #[arg(long, default_value_t = 240)]
        frames: usize,
        #[arg(long, default_value_t = 1_200_000)]
        points: usize,
        /// Open a temporary window and include presentation in a separate timing.
        #[arg(long)]
        window: bool,
    }
    let args = Args::parse();
    anyhow::ensure!(
        (160..=1280).contains(&args.preview_width)
            && args.frames > 0
            && (10_000..=2_000_000).contains(&args.points),
        "Invalid benchmark parameters"
    );
    let scene = scene::preset(&args.scene, 16.0 / 9.0)?;
    let started = Instant::now();
    let cloud = PointCloud::new(&scene, args.points);
    let mut renderer = PointRenderer::new(&cloud)?;
    let setup_seconds = started.elapsed().as_secs_f64();
    let mut nav = Navigator::new(scene.camera);
    let width = args.preview_width;
    let height = (width as f32 * 9.0 / 16.0).round() as u32;
    renderer.render(nav.camera, width, height, args.points)?;
    let mut state = ViewerState::new(1920, 1080, 512, 50, &args.scene, 0.0);
    state.navigating = true;
    state.backend = "Metal GPU".into();
    state.device = lumen_rs::metal_backend::available().unwrap();
    state.status = "POINT CLOUD PREVIEW".into();
    state.primitives = scene.primitives.len();
    state.bvh_nodes = scene.bvh.nodes.len();
    let mut cache = viewer_ui::FrameCache::default();
    let mut window = if args.window {
        let mut window = minifb::Window::new(
            "Lumen navigation benchmark",
            1280,
            820,
            minifb::WindowOptions::default(),
        )?;
        window.set_target_fps(0);
        Some(window)
    } else {
        None
    };
    let mut frame_pacer = lumen_rs::frame_pacing::FramePacer::new(60);
    let mut budget = args.points;
    let mut minimum_budget = budget;
    let mut durations = Vec::new();
    let mut presented = Vec::new();
    let mut gpu_frames = Vec::new();
    for index in 0..args.frames {
        if window.as_ref().is_some_and(|w| !w.is_open()) {
            break;
        }
        let started = Instant::now();
        nav.look(glam::Vec2::new(
            if index < args.frames / 2 {
                0.0015
            } else {
                -0.0015
            },
            0.0,
        ));
        nav.translate(
            glam::Vec3::Z * if index < args.frames / 2 { 1.0 } else { -1.0 },
            0.004,
            &scene,
        );
        let pixels = renderer.render(nav.camera, width, height, budget)?;
        let preview_seconds = started.elapsed().as_secs_f64();
        gpu_frames.push(preview_seconds);
        budget =
            lumen_rs::point_cloud::adjust_budget(budget, args.points, renderer.last_gpu_seconds);
        minimum_budget = minimum_budget.min(budget);
        state.preview = Some(viewer_ui::PreviewFrame {
            pixels: Vec::new(),
            packed: Some(pixels),
            width,
            height,
        });
        state.notice = format!("{budget} points / {width}x{height}");
        cache.invalidate();
        let (display, _) = viewer_ui::draw_cached(&state, 1280, 820, &mut cache);
        durations.push(started.elapsed().as_secs_f64());
        if let Some(window) = &mut window {
            window.update_with_buffer(&display, 1280, 820)?;
            frame_pacer.wait();
        }
        presented.push(started.elapsed().as_secs_f64());
    }
    let (display, _) = viewer_ui::draw(&state, 1920, 1080);
    let bytes: Vec<_> = display
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();
    std::fs::create_dir_all("renders")?;
    image::save_buffer(
        "renders/navigation-preview.png",
        &bytes,
        1920,
        1080,
        image::ColorType::Rgb8,
    )?;
    durations.sort_by(f64::total_cmp);
    gpu_frames.sort_by(f64::total_cmp);
    presented.sort_by(f64::total_cmp);
    let percentile = |values: &[f64], percent: usize| {
        values[(values.len() * percent / 100).min(values.len() - 1)]
    };
    anyhow::ensure!(
        !durations.is_empty(),
        "Window closed before the first benchmark frame"
    );
    let result = serde_json::json!({"scene": args.scene, "preview_width": width, "preview_height": height, "frames": durations.len(), "points": args.points, "minimum_adaptive_points": minimum_budget,
        "setup_seconds": setup_seconds, "median_frame_seconds": percentile(&durations, 50), "p95_frame_seconds": percentile(&durations, 95), "p99_frame_seconds": percentile(&durations, 99),
        "max_frame_seconds": durations.last(), "frames_over_16_67ms": durations.iter().filter(|time| **time > 1.0/60.0).count(), "median_gpu_and_readback_seconds": percentile(&gpu_frames, 50),
        "window": args.window, "median_with_presentation_seconds": percentile(&presented, 50), "primitives": scene.primitives.len(), "timings_seconds": durations,
        "note": "Point cloud, no textures. Work timings include GPU completion, readback and 1280x820 UI drawing. Presentation/pacing measured separately when --window is used. Not final rendering throughput."});
    std::fs::write(
        "renders/navigation-benchmark.json",
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This benchmark requires macOS and Metal.");
}
