//! Native framebuffer window with rendering on a worker thread and bounded tile updates.
use anyhow::{Context, Result, ensure};
use clap::{Parser, ValueEnum};
use lumen_rs::{
    output,
    progressive::{self, RenderControl, TileUpdate},
    render::{RenderOutput, RenderSettings},
    scene,
    viewer_ui::{self, Action, ViewerState},
};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::Ordering,
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Backend {
    Auto,
    Cpu,
    Metal,
}
#[derive(Parser, Clone)]
#[command(about = "Lumen Studio — progressive native Rust renderer")]
struct Args {
    #[arg(long, default_value = "studio")]
    scene: String,
    #[arg(long)]
    scene_file: Option<PathBuf>,
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
enum Event {
    Started {
        backend: String,
        device: String,
        primitives: usize,
        nodes: usize,
    },
    Tile(TileUpdate),
    Denoising(u32),
    Denoised {
        pixels: Vec<glam::Vec3>,
        samples: u32,
    },
    Finalizing,
    Warning(String),
    Finished(Box<RenderOutput>),
    Failed(String),
}
struct Job {
    receiver: Receiver<Event>,
    control: Arc<RenderControl>,
    started: Instant,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.control.cancelled.store(true, Ordering::Relaxed);
    }
}
fn start(args: &Args) -> Job {
    let (sender, receiver) = mpsc::sync_channel(64);
    let control = Arc::new(RenderControl::default());
    let worker_control = Arc::clone(&control);
    let args = args.clone();
    thread::spawn(move || {
        let run = || -> Result<RenderOutput> {
            let aspect = args.width as f32 / args.height as f32;
            let scene = if let Some(path) = &args.scene_file {
                scene::load(path, aspect)?
            } else {
                scene::preset(&args.scene, aspect)?
            };
            #[cfg(target_os = "macos")]
            let device = lumen_rs::metal_backend::available();
            #[cfg(not(target_os = "macos"))]
            let device: Option<String> = None;
            let metal = matches!(args.backend, Backend::Metal)
                || matches!(args.backend, Backend::Auto) && device.is_some();
            let _ = sender.send(Event::Started {
                backend: if metal { "Metal GPU" } else { "CPU / Rayon" }.into(),
                device: if metal {
                    #[cfg(target_os = "macos")]
                    {
                        device.unwrap_or_else(|| "Metal unavailable".into())
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        "Metal unavailable".into()
                    }
                } else {
                    format!("{} CPU threads", rayon::current_num_threads())
                },
                primitives: scene.primitives.len(),
                nodes: scene.bvh.nodes.len(),
            });
            let scene = Arc::new(scene);
            // OIDN owns its handle on a separate thread; filtering never stalls GPU dispatch.
            let filter = if args.raw {
                None
            } else {
                let (requests, incoming) = mpsc::sync_channel::<(Vec<glam::Vec3>, u32)>(1);
                let scene = Arc::clone(&scene);
                let events = sender.clone();
                let control = Arc::clone(&worker_control);
                let width = args.width;
                let height = args.height;
                let handle = thread::spawn(move || {
                    let denoiser = match lumen_rs::denoise::Denoiser::new() {
                        Ok(denoiser) => denoiser,
                        Err(error) => {
                            eprintln!("Denoising unavailable: {error:#}");
                            let _ = events.send(Event::Warning("RAW: install denoiser".into()));
                            return;
                        }
                    };
                    let guides = lumen_rs::denoise::Guides::new(&scene, width, height);
                    while let Ok((pixels, samples)) = incoming.recv() {
                        if control.cancelled.load(Ordering::Relaxed) {
                            break;
                        }
                        let _ = events.send(Event::Denoising(samples));
                        match denoiser.filter(&pixels, &guides, width, height) {
                            Ok(pixels) => {
                                let _ = events.send(Event::Denoised { pixels, samples });
                            }
                            Err(error) => {
                                eprintln!("{error:#}");
                                let _ = events.send(Event::Warning("Denoising failed".into()));
                            }
                        }
                    }
                });
                Some((requests, handle))
            };
            let mut last_filter = Instant::now();
            let mut first_filter = true;
            let mut live_pixels =
                vec![glam::Vec3::ZERO; args.width as usize * args.height as usize];
            let update = |tile: TileUpdate| {
                for (local, pixel) in tile.pixels.iter().enumerate() {
                    live_pixels[tile.tile.image_index(local, args.width)] = *pixel;
                }
                let pass_samples = tile.pass_samples;
                let filter_this_pass = tile.pass_complete
                    && pass_samples >= 8
                    && pass_samples < args.samples
                    && (first_filter || last_filter.elapsed() >= Duration::from_secs(3));
                if sender.send(Event::Tile(tile)).is_err() {
                    worker_control.cancelled.store(true, Ordering::Relaxed);
                    return;
                }
                if filter_this_pass
                    && !worker_control.cancelled.load(Ordering::Relaxed)
                    && let Some((requests, _)) = &filter
                    && requests
                        .try_send((live_pixels.clone(), pass_samples))
                        .is_ok()
                {
                    last_filter = Instant::now();
                    first_filter = false;
                }
            };
            let result = if metal {
                #[cfg(target_os = "macos")]
                {
                    lumen_rs::metal_backend::render_progressive(
                        &scene,
                        args.settings(),
                        &worker_control,
                        update,
                    )
                }
                #[cfg(not(target_os = "macos"))]
                {
                    anyhow::bail!("Metal requires macOS")
                }
            } else {
                progressive::cpu(&scene, args.settings(), &worker_control, update)
            };
            if let Some((requests, handle)) = filter {
                if let Ok(result) = &result
                    && !worker_control.cancelled.load(Ordering::Relaxed)
                {
                    let _ = sender.send(Event::Finalizing);
                    let _ = requests.send((result.pixels.clone(), args.samples));
                }
                drop(requests);
                let _ = handle.join();
            }
            result
        };
        match run() {
            Ok(result) => {
                let _ = sender.send(Event::Finished(Box::new(result)));
            }
            Err(error) => {
                let _ = sender.send(Event::Failed(format!("{error:#}")));
            }
        }
    });
    Job {
        receiver,
        control,
        started: Instant::now(),
    }
}
fn update(state: &mut ViewerState, event: Event, cancelled: bool) {
    match event {
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
fn main() -> Result<()> {
    let mut args = Args::parse();
    args.settings().validate()?;
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
        let scene = if let Some(path) = &args.scene_file {
            scene::load(path, aspect)?
        } else {
            scene::preset(&args.scene, aspect)?
        };
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
    window.set_target_fps(30);
    let mut state = ViewerState::new(
        args.width,
        args.height,
        args.samples,
        args.bounces,
        &args.scene,
        args.exposure,
    );
    let mut job = start(&args);
    let mut frame_cache = viewer_ui::FrameCache::default();
    let mut mouse_was_down = false;
    while window.is_open() {
        for event in job.receiver.try_iter().take(32) {
            update_cached(
                &mut state,
                event,
                job.control.cancelled.load(Ordering::Relaxed),
                &mut frame_cache,
            );
        }
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
        if mouse_down
            && !mouse_was_down
            && let Some((x, y)) = window.get_mouse_pos(MouseMode::Discard)
        {
            action = buttons
                .iter()
                .find(|button| button.rect.contains(x, y))
                .map(|button| button.action);
        }
        mouse_was_down = mouse_down;
        if window.is_key_pressed(Key::Space, KeyRepeat::No) {
            action = Some(Action::TogglePause);
        }
        if window.is_key_pressed(Key::D, KeyRepeat::No) {
            action = Some(Action::ToggleDenoise);
        }
        if window.is_key_pressed(Key::R, KeyRepeat::No) {
            action = Some(Action::Restart);
        }
        if window.is_key_pressed(Key::S, KeyRepeat::No) {
            action = Some(Action::Save);
        }
        if window.is_key_pressed(Key::Equal, KeyRepeat::Yes) {
            state.exposure = (state.exposure + 0.25).min(30.0);
        }
        if window.is_key_pressed(Key::Minus, KeyRepeat::Yes) {
            state.exposure = (state.exposure - 0.25).max(-30.0);
        }
        if window.is_key_pressed(Key::Escape, KeyRepeat::No) && !state.finished {
            job.control.cancelled.store(true, Ordering::Relaxed);
            state.status = "STOPPING".into();
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
            Some(Action::Restart | Action::Scene(_)) => {
                if let Some(Action::Scene(index)) = action {
                    args.scene = scene::PRESET_NAMES[index].into();
                    args.scene_file = None;
                }
                args.exposure = state.exposure;
                job.control.cancelled.store(true, Ordering::Relaxed);
                job = start(&args);
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
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
