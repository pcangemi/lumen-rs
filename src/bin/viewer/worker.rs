//! Persistent scene ownership, coalesced camera requests and generation-tagged results.
use super::{Args, Backend, PreviewStyle};
use anyhow::Result;
use glam::Vec3;
use lumen_rs::{
    camera::Camera,
    denoise::{Denoiser, Guides},
    navigation,
    progressive::{self, RenderControl, TileUpdate},
    render::RenderOutput,
    scene::{self, Scene},
};
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

pub enum Event {
    Ready(Arc<Scene>),
    Started {
        backend: String,
        device: String,
        primitives: usize,
        nodes: usize,
    },
    Preview {
        pixels: Vec<Vec3>,
        packed: Option<Vec<u32>>,
        width: u32,
        height: u32,
        seconds: f64,
    },
    Tile(TileUpdate),
    Denoising(u32),
    Denoised {
        pixels: Vec<Vec3>,
        samples: u32,
    },
    Finalizing,
    Warning(String),
    Finished(Box<RenderOutput>),
    Failed(String),
}

pub struct Message {
    pub generation: u64,
    pub event: Event,
}

/// A single replaceable slot bounds memory when input outruns rendering.
struct Latest<T> {
    value: Mutex<Option<T>>,
    ready: Condvar,
    closed: AtomicBool,
}
impl<T> Default for Latest<T> {
    fn default() -> Self {
        Self {
            value: Mutex::new(None),
            ready: Condvar::new(),
            closed: AtomicBool::new(false),
        }
    }
}
impl<T> Latest<T> {
    fn send(&self, value: T) {
        *self.value.lock().unwrap() = Some(value);
        self.ready.notify_one();
    }
    fn receive(&self) -> Option<T> {
        let mut slot = self.value.lock().unwrap();
        loop {
            if self.closed.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(value) = slot.take() {
                return Some(value);
            }
            slot = self.ready.wait(slot).unwrap();
        }
    }
    fn close(&self) {
        let _slot = self.value.lock().unwrap();
        self.closed.store(true, Ordering::Relaxed);
        self.ready.notify_all();
    }
}
struct Request {
    camera: Option<Camera>,
    preview: bool,
    generation: u64,
    control: Arc<RenderControl>,
}
pub struct Job {
    pub receiver: Receiver<Message>,
    pub control: Arc<RenderControl>,
    pub started: Instant,
    pub generation: u64,
    requests: Arc<Latest<Request>>,
    shutdown: Arc<RenderControl>,
}
impl Job {
    pub fn request(&mut self, camera: Camera, preview: bool) {
        self.control.cancelled.store(true, Ordering::Relaxed);
        self.control = Arc::new(RenderControl {
            discard_cancelled: true,
            ..Default::default()
        });
        self.generation += 1;
        self.started = Instant::now();
        self.requests.send(Request {
            camera: Some(camera),
            preview,
            generation: self.generation,
            control: Arc::clone(&self.control),
        });
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.control.cancelled.store(true, Ordering::Relaxed);
        self.shutdown.cancelled.store(true, Ordering::Relaxed);
        self.requests.close();
    }
}
fn emit(sender: &SyncSender<Message>, generation: u64, event: Event) {
    let _ = sender.send(Message { generation, event });
}

struct FilterRequest {
    camera: Camera,
    pixels: Vec<Vec3>,
    samples: u32,
    generation: u64,
    control: Arc<RenderControl>,
    result: Option<Box<RenderOutput>>,
}
fn filter_worker(
    scene: Arc<Scene>,
    args: Args,
    requests: Arc<Latest<FilterRequest>>,
    events: SyncSender<Message>,
) {
    let denoiser = Denoiser::new();
    let mut guides: Option<(u64, Guides)> = None;
    while let Some(request) = requests.receive() {
        let FilterRequest {
            camera,
            pixels,
            samples,
            generation,
            control,
            result,
        } = request;
        if control.cancelled.load(Ordering::Relaxed) {
            continue;
        }
        if let Ok(denoiser) = &denoiser {
            emit(&events, generation, Event::Denoising(samples));
            if guides.as_ref().is_none_or(|(id, _)| *id != generation) {
                guides = None;
                let Some(value) =
                    Guides::for_camera(&scene, camera, args.width, args.height, &control)
                else {
                    continue;
                };
                guides = Some((generation, value));
            }
            match denoiser.filter(
                &pixels,
                &guides.as_ref().unwrap().1,
                args.width,
                args.height,
            ) {
                Ok(pixels) if !control.cancelled.load(Ordering::Relaxed) => {
                    emit(&events, generation, Event::Denoised { pixels, samples })
                }
                Err(error) => emit(
                    &events,
                    generation,
                    Event::Warning(format!("Denoising failed: {error}")),
                ),
                _ => {}
            }
        } else {
            emit(
                &events,
                generation,
                Event::Warning("RAW: install denoiser".into()),
            );
        }
        if !control.cancelled.load(Ordering::Relaxed)
            && let Some(result) = result
        {
            emit(&events, generation, Event::Finished(result));
        }
    }
}

pub fn start(args: &Args) -> Job {
    let (sender, receiver) = mpsc::sync_channel(64);
    let requests = Arc::new(Latest::default());
    let control = Arc::new(RenderControl {
        discard_cancelled: true,
        ..Default::default()
    });
    requests.send(Request {
        camera: None,
        preview: false,
        generation: 1,
        control: Arc::clone(&control),
    });
    let shutdown = Arc::new(RenderControl {
        discard_cancelled: true,
        ..Default::default()
    });
    let job = Job {
        receiver,
        control,
        started: Instant::now(),
        generation: 1,
        requests: Arc::clone(&requests),
        shutdown: Arc::clone(&shutdown),
    };
    let args = args.clone();
    thread::spawn(move || {
        let run = || -> Result<()> {
            let aspect = args.width as f32 / args.height as f32;
            let scene = Arc::new(if let Some(path) = &args.scene_file {
                scene::load(path, aspect)?
            } else {
                scene::preset(&args.scene, aspect)?
            });
            #[cfg(target_os = "macos")]
            let device = lumen_rs::metal_backend::available();
            #[cfg(not(target_os = "macos"))]
            let device: Option<String> = None;
            let use_metal = matches!(args.backend, Backend::Metal)
                || matches!(args.backend, Backend::Auto) && device.is_some();
            emit(
                &sender,
                0,
                Event::Started {
                    backend: if use_metal {
                        "Metal GPU"
                    } else {
                        "CPU / Rayon"
                    }
                    .into(),
                    device: if use_metal {
                        device.unwrap_or_else(|| "Metal unavailable".into())
                    } else {
                        format!("{} CPU threads", rayon::current_num_threads())
                    },
                    primitives: scene.primitives.len(),
                    nodes: scene.bvh.nodes.len(),
                },
            );
            #[cfg(target_os = "macos")]
            let mut metal = if use_metal {
                Some(lumen_rs::metal_backend::MetalRenderer::new(&scene)?)
            } else {
                None
            };
            #[cfg(not(target_os = "macos"))]
            anyhow::ensure!(!use_metal, "Metal requires macOS");
            let point_cloud = matches!(args.preview_style, PreviewStyle::Points)
                .then(|| lumen_rs::point_cloud::PointCloud::new(&scene, 1_200_000));
            #[cfg(target_os = "macos")]
            let mut point_renderer = if use_metal {
                point_cloud
                    .as_ref()
                    .map(lumen_rs::metal_points::PointRenderer::new)
                    .transpose()?
            } else {
                None
            };
            let mut point_budget = 1_200_000usize;
            emit(&sender, 0, Event::Ready(Arc::clone(&scene)));
            let filters = Arc::new(Latest::default());
            let filter_thread = if args.raw {
                None
            } else {
                let filters = Arc::clone(&filters);
                let scene = Arc::clone(&scene);
                let args = args.clone();
                let sender = sender.clone();
                Some(thread::spawn(move || {
                    filter_worker(scene, args, filters, sender)
                }))
            };
            while let Some(request) = requests.receive() {
                let Request {
                    camera,
                    preview,
                    generation,
                    control,
                } = request;
                if control.cancelled.load(Ordering::Relaxed) {
                    continue;
                }
                let camera = camera.unwrap_or(scene.camera);
                if preview {
                    let width = args.preview_width.min(args.width);
                    let height = ((width as f64 * f64::from(args.height) / f64::from(args.width))
                        .round() as u32)
                        .max(1);
                    let started = Instant::now();
                    if let Some(cloud) = &point_cloud {
                        #[cfg(target_os = "macos")]
                        let pixels = if let Some(renderer) = &mut point_renderer {
                            renderer.render(camera, width, height, point_budget)
                        } else {
                            Ok(cloud.render_cpu(camera, width, height, point_budget))
                        };
                        #[cfg(not(target_os = "macos"))]
                        let pixels: Result<_> =
                            Ok(cloud.render_cpu(camera, width, height, point_budget));
                        let seconds = started.elapsed().as_secs_f64();
                        // Leave time in the 16.7 ms display budget for input and presentation.
                        #[cfg(target_os = "macos")]
                        let budget_seconds = point_renderer
                            .as_ref()
                            .map_or(seconds, |renderer| renderer.last_gpu_seconds);
                        #[cfg(not(target_os = "macos"))]
                        let budget_seconds = seconds;
                        point_budget = lumen_rs::point_cloud::adjust_budget(
                            point_budget,
                            cloud.points.len(),
                            budget_seconds,
                        );
                        match pixels {
                            Ok(packed) => emit(
                                &sender,
                                generation,
                                Event::Preview {
                                    pixels: Vec::new(),
                                    packed: Some(packed),
                                    width,
                                    height,
                                    seconds,
                                },
                            ),
                            Err(error) => {
                                emit(&sender, generation, Event::Failed(format!("{error:#}")))
                            }
                        }
                        continue;
                    }
                    // Finish one preview even if another pose arrives, then consume the latest
                    // request. Cancelling on every mouse event would starve slow preview frames.
                    #[cfg(target_os = "macos")]
                    let pixels = if let Some(metal) = &mut metal {
                        metal.preview(camera, width, height, &shutdown)
                    } else {
                        Ok(navigation::preview_cpu(
                            &scene, camera, width, height, &shutdown,
                        ))
                    };
                    #[cfg(not(target_os = "macos"))]
                    let pixels: Result<_> = Ok(navigation::preview_cpu(
                        &scene, camera, width, height, &shutdown,
                    ));
                    match pixels {
                        Ok(Some(pixels)) => emit(
                            &sender,
                            generation,
                            Event::Preview {
                                pixels,
                                packed: None,
                                width,
                                height,
                                seconds: started.elapsed().as_secs_f64(),
                            },
                        ),
                        Err(error) => {
                            emit(&sender, generation, Event::Failed(format!("{error:#}")))
                        }
                        _ => {}
                    }
                    continue;
                }
                let mut live_pixels = vec![Vec3::ZERO; args.width as usize * args.height as usize];
                let mut last_filter = Instant::now();
                let mut first_filter = true;
                let update = |tile: TileUpdate| {
                    for (local, pixel) in tile.pixels.iter().enumerate() {
                        live_pixels[tile.tile.image_index(local, args.width)] = *pixel;
                    }
                    let filter_pass = tile.pass_complete
                        && tile.pass_samples >= 8
                        && tile.pass_samples < args.samples
                        && (first_filter || last_filter.elapsed() >= Duration::from_secs(3));
                    let samples = tile.pass_samples;
                    emit(&sender, generation, Event::Tile(tile));
                    if filter_pass && !args.raw && !control.cancelled.load(Ordering::Relaxed) {
                        filters.send(FilterRequest {
                            camera,
                            pixels: live_pixels.clone(),
                            samples,
                            generation,
                            control: Arc::clone(&control),
                            result: None,
                        });
                        last_filter = Instant::now();
                        first_filter = false;
                    }
                };
                #[cfg(target_os = "macos")]
                let result = if let Some(metal) = &mut metal {
                    metal.render(camera, args.settings(), &control, true, update)
                } else {
                    progressive::cpu_with_camera(&scene, camera, args.settings(), &control, update)
                };
                #[cfg(not(target_os = "macos"))]
                let result =
                    progressive::cpu_with_camera(&scene, camera, args.settings(), &control, update);
                if control.cancelled.load(Ordering::Relaxed) {
                    continue;
                }
                match result {
                    Ok(result) if args.raw => {
                        emit(&sender, generation, Event::Finished(Box::new(result)))
                    }
                    Ok(result) => {
                        emit(&sender, generation, Event::Finalizing);
                        filters.send(FilterRequest {
                            camera,
                            pixels: result.pixels.clone(),
                            samples: args.samples,
                            generation,
                            control: Arc::clone(&control),
                            result: Some(Box::new(result)),
                        });
                    }
                    Err(error) => emit(&sender, generation, Event::Failed(format!("{error:#}"))),
                }
            }
            filters.close();
            if let Some(handle) = filter_thread {
                let _ = handle.join();
            }
            Ok(())
        };
        if let Err(error) = run() {
            emit(&sender, 0, Event::Failed(format!("{error:#}")));
        }
    });
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_mailbox_keeps_only_latest_request_and_wakes_on_close() {
        let slot = Arc::new(Latest::default());
        for value in 0..1000 {
            slot.send(value);
        }
        assert_eq!(slot.receive(), Some(999));
        let waiting = Arc::clone(&slot);
        let worker = thread::spawn(move || waiting.receive());
        slot.close();
        assert_eq!(worker.join().unwrap(), None);
    }
}
