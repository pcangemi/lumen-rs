# Metal performance work

The target is native Apple M1 Pro rendering with the complete scene, unchanged materials, full precision, and the requested sample and bounce counts. Navigation previews are a separate display mode and never enter the final accumulation.

## What the current implementation does

- A software binary SAH BVH runs inside a Metal compute kernel. Each path keeps a traversal stack and a nested dielectric stack.
- One thread traces a camera path, including its subsequent surface and volume interactions. Materials share a large shader with dynamic branches.
- Each 128 × 128 tile is submitted separately. Sample batches adapt to measured GPU duration, targeting 20 ms per command. The CPU waits for completion before reading shared film buffers.
- The viewer now owns a resident Metal renderer. Moving the camera reuses scene buffers and pipelines; only film state and camera parameters change. The same path-tracing kernel renders the final view.

These observations identify candidates. They do not establish which hardware resource currently limits the shader; that requires profiling and controlled timings.

## Priorities and quality gates

| Candidate | Expected benefit to investigate | Quality requirement |
| --- | --- | --- |
| Resident scene and pipeline objects | Avoid repeated setup and scene upload between viewpoints | Identical raw pixels after camera changes and film reset |
| Compiler threadgroup limits | Improve register allocation and GPU occupancy | Identical geometry, sampling and linear pixels; no fast math |
| Scene feature specialization | Remove code for absent volumes or opacity maps | Derive features from the entire scene, including off-camera objects |
| Traversal stack sized from actual BVH depth | Reduce per-thread storage | Keep enough entries for every pending sibling; never truncate traversal |
| Bounded overlapping submissions | Reduce gaps between GPU commands | Synchronize film reads and writes; preserve sample order and cancellation responsiveness |
| Native Metal acceleration structures | Compare Apple's traversal with the current software BVH | Preserve tiny triangles, cutouts, transmitting sheets, spheres and primitive identities |
| Separate traversal and shading passes | Reduce divergent work and register lifetimes | Preserve complete path state, nested dielectrics, MIS weights and random sequences |
| Spatial light selection / light tree | Reduce variance with many emissive triangles | Unbiased PDFs and MIS; compare error against a high-sample reference |

Apple recommends using function constants to eliminate unused shader features and tuning compute compilation for complex renderers. This motivates the specialization experiments; Apple's demonstration speedups are not predictions for this engine. [Optimize GPU renderers with Metal](https://developer.apple.com/videos/play/wwdc2023/10127/).

Apple explains that register and threadgroup memory pressure can limit occupancy, and that providing a maximum threadgroup size at pipeline creation can help the compiler. More threads alone are not necessarily faster. [Metal Compute on MacBook Pro](https://developer.apple.com/videos/play/tech-talks/10580/).

Immutable resources should be created outside the frame loop, with CPU/GPU access explicitly synchronized. A future submission pipeline must preserve those ownership rules rather than merely remove completion waits. [Setting up a command structure](https://developer.apple.com/documentation/metal/setting-up-a-command-structure), [Synchronizing CPU and GPU work](https://developer.apple.com/documentation/metal/synchronizing-cpu-and-gpu-work).

Apple's ray-tracing guidance discusses acceleration structures, intersection payloads and reducing memory indirections. M1 supports Metal ray tracing, but dedicated hardware-accelerated ray tracing arrived on Mac with M3. Native Metal traversal on M1 therefore needs a direct benchmark against this software traversal. [Maximize your Metal ray tracing performance](https://developer.apple.com/videos/play/wwdc2022/10105/), [Metal ray-tracing sample](https://developer.apple.com/documentation/metal/accelerating-ray-tracing-using-metal), [M3 hardware announcement](https://www.apple.com/newsroom/2023/10/apple-unveils-m3-m3-pro-and-m3-max-the-most-advanced-chips-for-a-personal-computer/).

## Reproducible measurements

Pause other renders first. Keep resolution, scene, sample count, bounce limit, seed, exposure and adaptive settings identical. Compare raw linear pixels before denoising or tone mapping. Reverse variant order on alternating repetitions to reduce ordering effects; report setup separately from tracing.

```bash
cargo run --release --example benchmark_metal_tuning -- \
  --scene interior-neon --width 640 --height 360 --samples 16 --bounces 50 --repeats 3
cargo run --release --example benchmark_navigation -- --scene interior-neon
```

`benchmark_metal_tuning` records exact pixel equality, maximum linear difference, GPU duration, longest command, wall-clock tracing duration and setup duration. Experimental tuning is explicit; merely adding a benchmark does not enable a variant in production.

`benchmark_navigation` measures point-cloud projection, GPU completion, readback, collision probes and interface drawing. `--window` also measures presentation and frame pacing separately. Its frame rate is not final-render throughput and cannot be compared to a 50-bounce path-traced frame.

A change to light selection or path guiding can improve noise at fixed time while producing different individual samples. Such an algorithm needs statistical image-quality tests against a reference, in addition to transport regression tests. Reducing samples, bounce limits, resolution, geometry detail or floating-point precision is not an accepted final-render optimisation here.

## M1 Pro diagnostic results (2026-11-28)

The 3,313,317-primitive neon apartment was measured at 640 × 360, 16 samples and 50 bounces, with three repetitions and reversed order. All tested variants produced identical raw pixels. These diagnostic timings are not Full HD or 2,048-sample estimates.

| Compiler experiment | Median tracing time |
| --- | ---: |
| Generic pipeline | 6.921 s |
| Remove unused scene features | 6.884 s |
| Also reduce traversal stack capacity | 7.451 s |
| Stack variant with 128-thread compiler limit | 9.752 s |
| Stack variant with 64-thread compiler limit | 7.823 s |

The generic configuration remains the default. Specialization did not establish a useful gain here; the other compiler settings regressed. GPU execution occupied approximately 98% of measured tracing time, leaving little opportunity for a large speedup from submission overhead alone. GPU execution time includes memory stalls and does not establish high shader occupancy. [Raw compiler experiment results](results/m1-pro-metal-compiler-options.json).

A separate paired experiment stored intersection geometry in a 64-byte stream, retaining all original floating-point values and primitive order. Its generic/compact medians were 6.131/5.931 s, approximately 3.3% less time. That costs about 212 MB of extra GPU buffers in this scene, so it remains explicit experimental tuning. Compare variants within their paired run; the two experiments have different compiled shaders and operating conditions. [Raw geometry experiment results](results/m1-pro-metal-geometry-layout.json).

The larger opportunities still requiring prototypes and profiling are native Metal traversal, restructuring the large path kernel to reduce divergent work, and spatial light sampling. No claim of a large final-render speedup is made by these experiments.

## Point-cloud navigation measurement

The final navigation benchmark walked and looked around the neon apartment for 600 frames, with a native 1280 × 820 window and a 960 × 540 point viewport. It used up to 1.2 million area-sampled points, a depth layer, no textures, adaptive density driven by Metal GPU execution timestamps, and absolute 60 Hz frame deadlines.

Median preview work plus UI drawing was 5.46 ms; the 95th percentile was 12.57 ms. Median presentation/pacing interval was 16.664 ms. Seven of 600 work frames exceeded 16.67 ms, with a maximum of 24.26 ms. This establishes a 60 FPS target with observed misses, not a perfectly locked frame rate. These timings do not include the transition out of an in-flight final-render tile. [Recorded navigation measurements](results/m1-pro-point-navigation.json).
