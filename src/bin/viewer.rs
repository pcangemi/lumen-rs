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
    #[arg(long, default_value_t = 960)]
    width: u32,
    #[arg(long, default_value_t = 640)]
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
    Denoising,
    Denoised(Vec<glam::Vec3>),
    Warning(String),
    Finished(Box<RenderOutput>),
    Failed(String),
}
struct Job {
    receiver: Receiver<Event>,
    control: Arc<RenderControl>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.control.cancelled.store(true, Ordering::Relaxed);
    }
}
fn start(args: &Args) -> Job {
    let (sender, receiver) = mpsc::sync_channel(4);
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
            let denoiser = if args.raw {
                None
            } else {
                match lumen_rs::denoise::Denoiser::new() {
                    Ok(denoiser) => Some(denoiser),
                    Err(error) => {
                        eprintln!("Denoising unavailable: {error:#}");
                        let _ = sender.send(Event::Warning("RAW: install denoiser".into()));
                        None
                    }
                }
            };
            let guides = denoiser
                .as_ref()
                .map(|_| lumen_rs::denoise::Guides::new(&scene, args.width, args.height));
            let mut live_pixels =
                vec![glam::Vec3::ZERO; args.width as usize * args.height as usize];
            let update = |tile: TileUpdate| {
                for (local, pixel) in tile.pixels.iter().enumerate() {
                    live_pixels[tile.tile.image_index(local, args.width)] = *pixel;
                }
                let filter_this_pass = tile.pass_complete
                    && tile.pass_samples >= 8
                    && ((tile.pass_samples >= 32 && tile.pass_samples.is_power_of_two())
                        || tile.pass_samples == args.samples);
                if sender.send(Event::Tile(tile)).is_err() {
                    worker_control.cancelled.store(true, Ordering::Relaxed);
                    return;
                }
                if filter_this_pass
                    && !worker_control.cancelled.load(Ordering::Relaxed)
                    && let (Some(denoiser), Some(guides)) = (&denoiser, &guides)
                {
                    let _ = sender.send(Event::Denoising);
                    match denoiser.filter(&live_pixels, guides, args.width, args.height) {
                        Ok(pixels) => {
                            let _ = sender.send(Event::Denoised(pixels));
                        }
                        Err(error) => {
                            eprintln!("{error:#}");
                            let _ = sender.send(Event::Warning("Denoising failed".into()));
                        }
                    }
                }
            };
            if metal {
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
            }
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
    Job { receiver, control }
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
        Event::Denoising => {
            state.status = "DENOISING".into();
        }
        Event::Denoised(pixels) => {
            state.filtered_pixels = Some(pixels);
            state.status = if state.paused { "PAUSED" } else { "RENDERING" }.into();
        }
        Event::Warning(message) => {
            state.notice = message;
        }
        Event::Finished(result) => {
            state.finished = true;
            state.paused = false;
            state.pixels = result.pixels;
            state.seconds = result.stats.render_seconds;
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
    let mut mouse_was_down = false;
    while window.is_open() {
        for event in job.receiver.try_iter().take(32) {
            update(
                &mut state,
                event,
                job.control.cancelled.load(Ordering::Relaxed),
            );
        }
        let (width, height) = window.get_size();
        if width == 0 || height == 0 {
            window.update();
            continue;
        }
        let (pixels, buttons) = viewer_ui::draw(&state, width, height);
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
                    args.scene = ["studio", "interior", "outdoor", "clouds"][index].into();
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
