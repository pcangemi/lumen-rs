//! Native framebuffer window with rendering on a worker thread and bounded tile updates.
use anyhow::{Context, Result, ensure};
use clap::{Parser, ValueEnum};
use lumen_rs::{
    navigation::{Navigator, Viewpoint},
    output,
    progressive::{self, RenderControl},
    render::RenderSettings,
    scene,
    viewer_ui::{self, Action, ViewerState},
};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Backend {
    Auto,
    Cpu,
    Metal,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum PreviewStyle {
    Points,
    Materials,
}

#[derive(Parser, Clone)]
#[command(about = "Lumen Studio — progressive native Rust renderer")]
struct Args {
    #[arg(long, default_value = "studio")]
    scene: String,
    #[arg(long)]
    scene_file: Option<PathBuf>,
    /// Restore a saved camera and exposure on startup.
    #[arg(long)]
    viewpoint: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "auto")]
    backend: Backend,
    #[arg(long, default_value_t = 1920)]
    width: u32,
    #[arg(long, default_value_t = 1080)]
    height: u32,
    #[arg(long, default_value_t = 512)]
    samples: u32,
    #[arg(long, default_value_t = 12)]
    bounces: u32,
    #[arg(long, default_value_t = 8)]
    batch: u32,
    #[arg(long, default_value_t = 0.0)]
    exposure: f32,
    #[arg(long, default_value_t = 0.0)]
    adaptive: f32,
    /// CPU rendering and auxiliary work; does not set Metal GPU thread counts.
    #[arg(long)]
    threads: Option<usize>,
    /// Export the actual UI drawing without creating a window (CPU validation).
    #[arg(long)]
    headless_preview: Option<PathBuf>,
    /// Width of the approximate navigation preview; final resolution is unchanged.
    #[arg(long, default_value_t = 960, value_parser = clap::value_parser!(u32).range(160..=1280))]
    preview_width: u32,
    #[arg(long, value_enum, default_value = "points")]
    preview_style: PreviewStyle,
    /// Quiet time before starting a fresh full-quality render.
    #[arg(long, default_value_t = 450, value_parser = clap::value_parser!(u64).range(100..=5000))]
    settle_ms: u64,
    /// Disable automatic progressive denoising.
    #[arg(long)]
    raw: bool,
}
impl Args {
    fn settings(&self) -> RenderSettings {
        RenderSettings {
            width: self.width,
            height: self.height,
            samples: self.samples,
            max_depth: self.bounces,
            batch: self.batch,
            adaptive: self.adaptive,
            ..Default::default()
        }
    }
}
#[path = "viewer/input.rs"]
mod input;
#[path = "viewer/worker.rs"]
mod worker;
use worker::{Event, start};

fn update(state: &mut ViewerState, event: Event, cancelled: bool) {
    match event {
        Event::Ready(_) => {}
        Event::Preview {
            pixels,
            packed,
            width,
            height,
            seconds,
        } => {
            state.preview = Some(viewer_ui::PreviewFrame {
                pixels,
                packed,
                width,
                height,
            });
            state.filtered_pixels = None;
            state.notice = format!("Preview {width}x{height}: {:.0} ms", seconds * 1000.0);
        }
        Event::Started {
            backend,
            device,
            primitives,
            nodes,
        } => {
            state.backend = backend;
            state.device = device;
            state.primitives = primitives;
            state.bvh_nodes = nodes;
            state.status = "RENDERING".into();
        }
        Event::Tile(update) => {
            for (local, pixel) in update.pixels.into_iter().enumerate() {
                state.pixels[update.tile.image_index(local, state.image_width)] = pixel;
            }
            state.completed_samples = update.completed_samples;
            state.progress = update.progress;
            state.pass_samples = update.pass_samples;
            state.seconds = update.elapsed.as_secs_f64();
            state.last_tile = Some(update.tile);
        }
        Event::Denoising(samples) => {
            state.notice = format!("Filtering {samples} spp");
        }
        Event::Denoised { pixels, samples } => {
            state.filtered_pixels = Some(pixels);
            state.filtered_samples = samples;
            state.notice = format!("Clean preview: {samples} spp");
        }
        Event::Finalizing => {
            state.status = "FINAL DENOISE".into();
        }
        Event::Warning(message) => {
            state.notice = message;
        }
        Event::Finished(result) => {
            state.finished = true;
            state.paused = false;
            state.pixels = result.pixels;
            state.seconds = state.seconds.max(result.stats.render_seconds);
            state.status = if cancelled { "STOPPED" } else { "COMPLETE" }.into();
            if !cancelled {
                state.progress = 1.0;
            }
            if result.stats.invalid_samples > 0 {
                state.status = "INVALID SAMPLES".into();
            }
        }
        Event::Failed(error) => {
            eprintln!("{error}");
            state.finished = true;
            state.status = "RENDER FAILED".into();
            state.notice = "See terminal for details".into();
        }
    }
}

fn update_cached(
    state: &mut ViewerState,
    event: Event,
    cancelled: bool,
    cache: &mut viewer_ui::FrameCache,
) {
    let filtered = state.show_filtered && state.filtered_pixels.is_some();
    let image_changed = match &event {
        Event::Preview { .. } => true,
        Event::Tile(_) | Event::Finished(_) => !filtered,
        Event::Denoised { .. } => state.show_filtered,
        _ => false,
    };
    if image_changed {
        cache.invalidate();
    }
    update(state, event, cancelled);
}
fn save_png(state: &ViewerState) -> Result<PathBuf> {
    ensure!(
        state.preview.is_none(),
        "Stop moving and let refinement start before saving a rendered image"
    );
    let path = PathBuf::from("renders").join(format!("{}-live.png", state.scene));
    std::fs::create_dir_all("renders")?;
    let bytes: Vec<u8> = state
        .display_pixels()
        .iter()
        .flat_map(|pixel| {
            let color = output::tonemap(*pixel, state.exposure);
            [
                (color.x * 255.0 + 0.5) as u8,
                (color.y * 255.0 + 0.5) as u8,
                (color.z * 255.0 + 0.5) as u8,
            ]
        })
        .collect();
    image::save_buffer_with_format(
        &path,
        &bytes,
        state.image_width,
        state.image_height,
        image::ColorType::Rgb8,
        image::ImageFormat::Png,
    )?;
    Ok(path)
}
fn reset_accumulation(state: &mut ViewerState, preview: bool) {
    state.navigating = preview;
    state.filtered_pixels = None;
    state.filtered_samples = 0;
    state.completed_samples = 0;
    state.pass_samples = 0;
    state.progress = 0.0;
    state.seconds = 0.0;
    state.last_tile = None;
    state.finished = false;
    state.paused = false;
    state.status = if preview {
        "NAVIGATION PREVIEW"
    } else {
        "RENDERING"
    }
    .into();
    if !preview {
        state.preview = None;
        state.pixels.fill(glam::Vec3::ZERO);
        state.notice.clear();
    }
}

fn main() -> Result<()> {
    let mut args = Args::parse();
    args.settings().validate()?;
    let initial_view = if let Some(path) = &args.viewpoint {
        let key = args.scene_file.as_ref().map_or_else(
            || args.scene.clone(),
            |path| path.to_string_lossy().into_owned(),
        );
        let view = Viewpoint::load(path, &key, args.width as f32 / args.height as f32)?;
        args.exposure = view.exposure;
        Some(view.camera)
    } else {
        None
    };
    ensure!(
        args.exposure.is_finite() && args.exposure.abs() <= 30.0,
        "Exposure must be between -30 and +30 EV"
    );
    if let Some(threads) = args.threads {
        ensure!(threads > 0, "Thread count must be positive");
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()?;
    }
    if let Some(path) = &args.headless_preview {
        let aspect = args.width as f32 / args.height as f32;
        let mut scene = if let Some(path) = &args.scene_file {
            scene::load(path, aspect)?
        } else {
            scene::preset(&args.scene, aspect)?
        };
        if let Some(camera) = initial_view {
            scene.camera = camera;
        }
        let mut state = ViewerState::new(
            args.width,
            args.height,
            args.samples,
            args.bounces,
            &args.scene,
            args.exposure,
        );
        state.backend = "CPU / Rayon".into();
        state.device = format!("{} CPU threads", rayon::current_num_threads());
        state.primitives = scene.primitives.len();
        state.bvh_nodes = scene.bvh.nodes.len();
        let control = RenderControl::default();
        let result = progressive::cpu(&scene, args.settings(), &control, |tile| {
            update(&mut state, Event::Tile(tile), false)
        })?;
        update(&mut state, Event::Finished(Box::new(result)), false);
        if !args.raw
            && let Ok(denoiser) = lumen_rs::denoise::Denoiser::new()
        {
            let guides = lumen_rs::denoise::Guides::new(&scene, args.width, args.height);
            state.filtered_pixels =
                Some(denoiser.filter(&state.pixels, &guides, args.width, args.height)?);
            state.filtered_samples = args.samples;
        }
        let (pixels, _) = viewer_ui::draw(&state, 1280, 820);
        let bytes: Vec<u8> = pixels
            .into_iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, p as u8])
            .collect();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        image::save_buffer(path, &bytes, 1280, 820, image::ColorType::Rgb8)?;
        println!("UI preview: {}", path.display());
        return Ok(());
    }
    let mut window = Window::new(
        "Lumen Studio | Progressive Path Tracing",
        1280,
        820,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    )
    .context("Cannot create window; launch viewer from a graphical desktop session")?;
    window.set_target_fps(0);
    let mut frame_pacer = lumen_rs::frame_pacing::FramePacer::new(60);
    let mut state = ViewerState::new(
        args.width,
        args.height,
        args.samples,
        args.bounces,
        &args.scene,
        args.exposure,
    );
    let mut job = start(&args);
    if let Some(camera) = initial_view {
        job.request(camera, false);
    }
    let mut restore_initial_view = initial_view;
    let mut frame_cache = viewer_ui::FrameCache::default();
    let mut mouse_was_down = false;
    let mut navigator: Option<Navigator> = None;
    let mut navigation_scene = None;
    let mut last_mouse = None;
    let mut last_tick = Instant::now();
    let mut last_motion: Option<Instant> = None;
    let mut latest_preview = 0;
    while window.is_open() {
        for message in job.receiver.try_iter().take(32) {
            if let Event::Ready(scene) = &message.event {
                let mut nav = Navigator::new(scene.camera);
                if let Some(camera) = restore_initial_view.take() {
                    nav.camera = camera;
                }
                navigator = Some(nav);
                navigation_scene = Some(Arc::clone(scene));
            }
            let accepted_preview = state.navigating
                && message.generation >= latest_preview
                && matches!(message.event, Event::Preview { .. });
            if message.generation == 0
                || (!job.control.cancelled.load(Ordering::Relaxed)
                    && (message.generation == job.generation || accepted_preview))
            {
                if matches!(message.event, Event::Preview { .. }) {
                    if !state.navigating {
                        continue;
                    }
                    latest_preview = message.generation;
                }
                update_cached(
                    &mut state,
                    message.event,
                    job.control.cancelled.load(Ordering::Relaxed),
                    &mut frame_cache,
                );
            }
        }
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f32().min(0.05);
        last_tick = now;
        let input = input::Input::read(&mut window);
        let looking = input.looking;
        let mouse = window.get_mouse_pos(MouseMode::Discard);
        let mut moved = false;
        if let (Some(nav), Some(scene)) = (&mut navigator, &navigation_scene) {
            if input.active {
                if looking && let (Some((x, y)), Some((px, py))) = (mouse, last_mouse) {
                    let delta = glam::Vec2::new(x - px, py - y) * 0.003;
                    if delta.length_squared() > 1e-8 {
                        nav.look(delta);
                        moved = true;
                    }
                }
                if input.toggle_flight {
                    nav.toggle_flight();
                    moved = true;
                }
                moved |= nav.translate(input.movement, dt * input.speed, scene);
                if window.is_key_pressed(Key::Home, KeyRepeat::No) {
                    nav.reset();
                    moved = true;
                }
                let name: String = args
                    .scene
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let view_path = PathBuf::from("renders/viewpoints").join(format!("{name}.json"));
                let scene_key = args.scene_file.as_ref().map_or_else(
                    || args.scene.clone(),
                    |path| path.to_string_lossy().into_owned(),
                );
                if window.is_key_pressed(Key::F5, KeyRepeat::No) {
                    match (Viewpoint {
                        scene: scene_key.clone(),
                        camera: nav.camera,
                        exposure: state.exposure,
                    })
                    .save(&view_path)
                    {
                        Ok(()) => {
                            state.notice = "Viewpoint saved [F9 restore]".into();
                            println!("Saved viewpoint {}", view_path.display());
                        }
                        Err(error) => {
                            state.notice = "Viewpoint save failed".into();
                            eprintln!("{error:#}");
                        }
                    }
                }
                if window.is_key_pressed(Key::F9, KeyRepeat::No) {
                    match Viewpoint::load(
                        &view_path,
                        &scene_key,
                        args.width as f32 / args.height as f32,
                    ) {
                        Ok(view) => {
                            nav.camera = view.camera;
                            state.exposure = view.exposure;
                            moved = true;
                        }
                        Err(error) => {
                            state.notice = "No valid saved viewpoint".into();
                            eprintln!("{error:#}");
                        }
                    }
                }
            }
            state.flying = nav.flying;
            if moved {
                job.request(nav.camera, true);
                reset_accumulation(&mut state, true);
                last_motion = Some(now);
                frame_cache.invalidate();
            } else if !state.paused
                && last_motion.is_some_and(|time| {
                    now.duration_since(time) >= Duration::from_millis(args.settle_ms)
                })
            {
                job.request(nav.camera, false);
                reset_accumulation(&mut state, false);
                last_motion = None;
                frame_cache.invalidate();
            }
        }
        last_mouse = if looking { mouse } else { None };
        if !state.finished {
            state.seconds = job.started.elapsed().as_secs_f64();
        }
        let (width, height) = window.get_size();
        if width == 0 || height == 0 {
            window.update();
            continue;
        }
        let (pixels, buttons) = viewer_ui::draw_cached(&state, width, height, &mut frame_cache);
        let mut action = None;
        let mouse_down = window.get_mouse_down(MouseButton::Left);
        if input.active
            && mouse_down
            && !mouse_was_down
            && let Some((x, y)) = window.get_mouse_pos(MouseMode::Discard)
        {
            action = buttons
                .iter()
                .find(|button| button.rect.contains(x, y))
                .map(|button| button.action);
        }
        mouse_was_down = mouse_down;
        if input.active && window.is_key_pressed(Key::Space, KeyRepeat::No) {
            action = Some(Action::TogglePause);
        }
        if input.toggle_denoise {
            action = Some(Action::ToggleDenoise);
        }
        if input.active && window.is_key_pressed(Key::R, KeyRepeat::No) {
            action = Some(Action::Restart);
        }
        if input.save_image {
            action = Some(Action::Save);
        }
        if input.active && window.is_key_pressed(Key::Equal, KeyRepeat::Yes) {
            state.exposure = (state.exposure + 0.25).min(30.0);
        }
        if input.active && window.is_key_pressed(Key::Minus, KeyRepeat::Yes) {
            state.exposure = (state.exposure - 0.25).max(-30.0);
        }
        if input.active && window.is_key_pressed(Key::Escape, KeyRepeat::No) && !state.finished {
            job.control.cancelled.store(true, Ordering::Relaxed);
            state.status = "STOPPED".into();
            state.finished = true;
            state.navigating = false;
            last_motion = None;
        }
        match action {
            Some(Action::ToggleDenoise) => {
                state.show_filtered = !state.show_filtered;
            }
            Some(Action::TogglePause) if !state.finished => {
                state.paused = !state.paused;
                job.control.paused.store(state.paused, Ordering::Relaxed);
                state.status = if state.paused { "PAUSED" } else { "RENDERING" }.into();
            }
            Some(Action::Restart) => {
                if let Some(nav) = &navigator {
                    job.request(nav.camera, false);
                    reset_accumulation(&mut state, false);
                    last_motion = None;
                    frame_cache.invalidate();
                }
            }
            Some(Action::Scene(_)) => {
                if let Some(Action::Scene(index)) = action {
                    args.scene = scene::PRESET_NAMES[index].into();
                    args.scene_file = None;
                    args.viewpoint = None;
                }
                args.exposure = state.exposure;
                job.control.cancelled.store(true, Ordering::Relaxed);
                job = start(&args);
                navigator = None;
                navigation_scene = None;
                last_motion = None;
                latest_preview = 0;
                state = ViewerState::new(
                    args.width,
                    args.height,
                    args.samples,
                    args.bounces,
                    &args.scene,
                    args.exposure,
                );
                frame_cache.invalidate();
            }
            Some(Action::Save) => match save_png(&state) {
                Ok(path) => {
                    state.notice = "PNG saved in renders/".into();
                    println!("Saved {}", path.display());
                }
                Err(error) => {
                    state.notice = "Save failed: see terminal".into();
                    eprintln!("{error}");
                }
            },
            _ => {}
        }
        window.update_with_buffer(&pixels, width, height)?;
        frame_pacer.wait();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumen_rs::progressive::TileUpdate;

    #[test]
    fn render_events_refresh_the_visible_frame() {
        let mut state = ViewerState::new(8, 8, 16, 50, "studio", 0.0);
        let mut cache = viewer_ui::FrameCache::default();
        let events = [
            Event::Tile(TileUpdate {
                tile: progressive::Tile {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
                pixels: vec![glam::Vec3::X; 64],
                completed_samples: 64,
                progress: 1.0 / 16.0,
                pass_samples: 1,
                pass_complete: true,
                elapsed: Duration::from_millis(10),
            }),
            Event::Denoised {
                pixels: vec![glam::Vec3::Y; 64],
                samples: 8,
            },
            Event::Denoised {
                pixels: vec![glam::Vec3::Z; 64],
                samples: 16,
            },
        ];
        for event in events {
            let before = viewer_ui::draw_cached(&state, 640, 480, &mut cache).0;
            update_cached(&mut state, event, false, &mut cache);
            let after = viewer_ui::draw_cached(&state, 640, 480, &mut cache).0;
            assert_ne!(before, after);
            assert_eq!(after, viewer_ui::draw(&state, 640, 480).0);
        }
    }

    #[test]
    fn camera_changes_discard_previous_accumulation() {
        let args = Args::parse_from([
            "viewer",
            "--backend",
            "cpu",
            "--scene",
            "studio",
            "--width",
            "24",
            "--height",
            "16",
            "--samples",
            "8",
            "--raw",
        ]);
        let mut job = start(&args);
        let scene = loop {
            let message = job.receiver.recv_timeout(Duration::from_secs(10)).unwrap();
            if let Event::Ready(scene) = message.event {
                break scene;
            }
        };
        let mut camera = scene.camera;
        for _ in 0..20 {
            camera.origin[0] += 0.01;
            job.request(camera, true);
        }
        job.request(camera, false);
        let result = loop {
            let message = job
                .receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("Navigation worker stalled");
            if message.generation != job.generation {
                continue;
            }
            match message.event {
                Event::Finished(result) => break result,
                Event::Failed(error) => panic!("{error}"),
                _ => {}
            }
        };
        let expected = progressive::cpu_with_camera(
            &scene,
            camera,
            args.settings(),
            &RenderControl::default(),
            |_| {},
        )
        .unwrap();
        assert_eq!(result.pixels, expected.pixels);
        assert_eq!(result.samples, expected.samples);
    }

    #[test]
    #[ignore = "requires the optional Open Image Denoise runtime"]
    fn worker_delivers_final_clean_image_before_completion() {
        let args = Args::parse_from([
            "viewer",
            "--backend",
            "cpu",
            "--scene",
            "studio",
            "--width",
            "24",
            "--height",
            "16",
            "--samples",
            "16",
            "--batch",
            "8",
        ]);
        let job = start(&args);
        let mut tiles = 0;
        let mut clean_samples = 0;
        loop {
            match job
                .receiver
                .recv_timeout(Duration::from_secs(30))
                .expect("Viewer worker stalled")
                .event
            {
                Event::Tile(_) => tiles += 1,
                Event::Denoised { pixels, samples } => {
                    assert!(pixels.iter().all(|p| p.is_finite()));
                    assert!(samples >= clean_samples);
                    clean_samples = samples;
                }
                Event::Finished(result) => {
                    assert!(tiles > 0);
                    assert_eq!(result.pixels.len(), 24 * 16);
                    assert_eq!(clean_samples, 16);
                    break;
                }
                Event::Failed(error) | Event::Warning(error) => panic!("{error}"),
                _ => {}
            }
        }
    }
}
