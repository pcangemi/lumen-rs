# Validation record

Validated on 2026-11-26 with Apple M1 Pro, 10 CPU threads and Blender 5.3.0.

## Automated checks

- Rust formatting and Clippy with warnings denied: passed.
- [GitHub CI on Ubuntu and Apple Silicon macOS](https://github.com/pcangemi/lumen-rs/actions/runs/45799818520): passed, including release builds and smoke renders.
- Standard release physics suite: 18 passed; the GPU test is deliberately ignored in this suite.
- Optional Open Image Denoise noise-reduction test: passed with the installed 2.5.1 CPU runtime.
- Metal shader compilation: passed.
- Metal runtime regression from a normal macOS Terminal session: passed across studio, interior, outdoor and clouds.
- OBJ with a nested MTL directory and a relative diffuse texture: passed.
- Water and homogeneous-fog JSON smoke renders: passed with finite output.
- Progressive viewer drawing: rendered and visually inspected without opening a native window.
- Cycles scene export: studio and textured interior rendered successfully on CPU. The comparison script completed a raw reference/candidate run and wrote image-error metrics.

## CPU/Metal regression

73 × 47 pixels, 64 samples per pixel, eight bounces. Values below are sums of linear pixel luminance, not timing measurements.

| Scene | CPU | Metal |
| --- | ---: | ---: |
| Studio | 777.843 | 777.853 |
| Interior | 1785.528 | 1785.938 |
| Outdoor | 845.800 | 845.527 |
| Clouds | 1222.783 | 1222.789 |

Mean luminance differs by less than 0.04% in this run. Different GPU batch sizes and progressive versus full-image dispatch produced exactly equal pixels. This statistical check does not prove equality of every CPU and GPU path.

## Images

The repository previews were rendered by Lumen's CPU backend and filtered with Open Image Denoise. Studio and courtyard use 900 × 600 at 256 samples; the interior uses 900 × 600 at 192 samples; the cloud study uses 720 × 480 at 256 samples and 24 bounces. The UI preview uses 750 × 500 at 128 samples. Per-image JSON records contain the actual settings and timings. Raw PNG and EXR counterparts are retained locally; large raw outputs are excluded from Git.

The interior has been visually checked for parquet direction, windows, mouldings and furniture. The clouds scene is a transport study with a simple procedural density field. These are development previews, not a claim of photographic equivalence.

## Measured M1 Pro comparison

The interior run completed on Metal with Blender 5.3.0: 640 × 426, 128 samples, 12 bounces, three candidate runs per renderer and independent 2,048-sample references. Adaptive sampling, clamping and denoising were disabled.

| Renderer | Median process wall time | Median relative L2 error against its own reference |
| --- | ---: | ---: |
| Lumen / Metal | 26.58 s | 6.91% |
| Cycles / Metal | 20.15 s | 6.04% |

Cycles was faster and had lower measured error at this setting. This is not an equal-quality speed ratio: materials are mapped approximately and references retain noise. The full [machine-readable report](results/m1-pro-interior-raw.json) includes every run, settings, image errors, device and timing scopes.

Lumen's reference took 430.50 seconds of transport. Cycles' first reference render invocation took 598.44 seconds, including its initial Metal shader compilation. That cold reference invocation must not be used as evidence that Cycles renders more slowly. Candidate timings were measured afterward. Subsequent first-time shader compilation may still vary with Blender, driver and material configurations.

## Remaining validation

A controlled error-versus-time curve at delivery resolution remains necessary. The current run establishes a baseline, not performance leadership. Repeat across sample counts and scene families, and evaluate denoised output separately.

Native window event handling and long-session GPU memory behavior require interactive desktop testing. Headless screenshots do not establish those properties. Specialized caustic convergence, production asset compatibility and full color-management accuracy remain separate development work.
