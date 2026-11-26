# Validation record

Validated on 2026-11-26 with Apple M1 Pro, 10 CPU threads and Blender 5.3.0.

## Automated checks

- Rust formatting and Clippy with warnings denied: passed.
- [Earlier baseline CI on Ubuntu and Apple Silicon macOS](https://github.com/pcangemi/lumen-rs/actions/runs/45799818520): passed, including release builds and smoke renders. See the repository Actions tab for the checks on the latest commit.
- Standard release physics suite: 20 passed; the GPU test is deliberately ignored in this suite.
- Optional Open Image Denoise noise-reduction test: passed with the installed 2.5.1 CPU runtime.
- Metal shader compilation: passed.
- Metal runtime regression from a normal macOS Terminal session: passed across studio, interior, outdoor and clouds.
- OBJ with a nested MTL directory and a relative diffuse texture: passed.
- Water and homogeneous-fog JSON smoke renders: passed with finite output.
- Progressive viewer drawing: rendered and visually inspected without opening a native window.
- Cycles scene export: studio and textured interior rendered successfully on CPU. The comparison script completed a raw reference/candidate run and wrote image-error metrics.

## Earlier CPU/Metal regression

73 × 47 pixels, 64 samples per pixel, eight bounces. Values below are sums of linear pixel luminance, not timing measurements.

| Scene | CPU | Metal |
| --- | ---: | ---: |
| Studio | 777.843 | 777.853 |
| Interior | 1785.528 | 1785.938 |
| Outdoor | 845.800 | 845.527 |
| Clouds | 1222.783 | 1222.789 |

Mean luminance differs by less than 0.04% in this run. Different GPU batch sizes and progressive versus full-image dispatch produced exactly equal pixels. This statistical check does not prove equality of every CPU and GPU path.

## Images

The repository previews were rendered by Lumen's CPU backend and filtered with Open Image Denoise. Studio and courtyard use 900 × 600 at 256 samples; the updated interior uses 1920 × 1080 at 128 samples; the cloud study uses 720 × 480 at 256 samples and 24 bounces. The UI preview reuses the finished interior EXR and its JSON report through `examples/viewer_snapshot.rs`; it does not retrace the scene. Per-image JSON records contain the actual settings and timings. Raw PNG and EXR counterparts are retained locally; large raw outputs are excluded from Git.

The interior now includes source furniture meshes, mapped foliage, displaced rug pile, cast sections and pipe fittings. Its completed Full HD image was visually inspected: 762,612 primitives, 452,703 BVH nodes, 36 textures and zero invalid samples. Packed scene data occupies 734,836,176 bytes, excluding render buffers, GPU copies and allocator overhead. The CPU render took 746.04 seconds and denoising took 4.20 seconds; this development run overlapped other work and is not a controlled performance measurement. The source sofa uses an adapted diffuse/GGX fabric material rather than its original velvet sheen shader. The clouds scene is a transport study with a simple procedural density field. These are development previews, not a claim of photographic equivalence.

## Archived M1 Pro comparison (earlier scene)

This run used the earlier 55,438-primitive interior, before the furniture, foliage and rug update. Its timings do not describe the current scene. The run completed on Metal with Blender 5.3.0: 640 × 426, 128 samples, 12 bounces, three candidate runs per renderer and independent 2,048-sample references. Adaptive sampling, clamping and denoising were disabled.

| Renderer | Median process wall time | Median relative L2 error against its own reference |
| --- | ---: | ---: |
| Lumen / Metal | 26.58 s | 6.91% |
| Cycles / Metal | 20.15 s | 6.04% |

Cycles was faster and had lower measured error at this setting. This is not an equal-quality speed ratio: materials are mapped approximately and references retain noise. The full [machine-readable report](results/m1-pro-interior-raw.json) includes every run, settings, image errors, device and timing scopes.

Lumen's reference took 430.50 seconds of transport. Cycles' first reference render invocation took 598.44 seconds, including its initial Metal shader compilation. That cold reference invocation must not be used as evidence that Cycles renders more slowly. Candidate timings were measured afterward. Subsequent first-time shader compilation may still vary with Blender, driver and material configurations.

## Remaining validation

A controlled error-versus-time curve at delivery resolution remains necessary. The current run establishes a baseline, not performance leadership. Repeat across sample counts and scene families, and evaluate denoised output separately.

Native window event handling and long-session GPU memory behavior require interactive desktop testing. Headless screenshots do not establish those properties. Specialized caustic convergence, production asset compatibility and full color-management accuracy remain separate development work.

## Material and viewer update

The standard suite additionally checks linear PBR data maps, mirrored tangent frames,
alpha rejection for camera/shadow rays, curtain clearance, radiator/sill clearance,
and tile outlines over a clean preview. Formatting, Clippy and Metal shader
compilation are checked after the changes. The final Metal runtime check, including leaf opacity, passed on the M1 Pro:
interior luminance was 2008.782 on CPU and 2007.724 on Metal (about 0.053% difference).
All four scenes passed, and different GPU batch sizes and tiled/full-image dispatch
produced identical pixels. This remains a statistical check rather than proof of
per-path equality. The asynchronous viewer worker also passed its optional OIDN
integration test, delivering the final clean image before the completion event.
The OIDN noise-reduction regression passed separately.
An additional Cycles CPU smoke render exercised exported normal, roughness and
opacity maps successfully.

Both rendering executables now default to 1920 × 1080. The viewer denoises on a
separate worker, uses a bounded queue, retains tile activity in clean mode, shows
the clean snapshot's sample count, and waits for the final clean image before
reporting completion. Native window interaction still requires a desktop test.
