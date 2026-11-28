# Metal performance work

The target is native Apple M1 Pro rendering with the complete scene, unchanged materials, full precision, and the requested sample and bounce counts. Navigation previews are a separate display mode and never enter the final accumulation.

## What the current implementation does

- A software binary SAH BVH runs inside a Metal compute kernel, with lossless 32-byte nodes and cached per-ray bounds calculations. Intersection geometry and shading attributes use separate streams. Each path keeps a traversal stack and a nested dielectric stack.
- One thread traces a camera path, including its subsequent surface and volume interactions. Materials share a large shader with dynamic branches.
- Each 128 × 128 tile is submitted separately, with at most two commands outstanding. Sample batches adapt to measured GPU duration, targeting 20 ms per command. The CPU reads only completed tile pixels and drains each batch before film reuse.
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

At the time of this earlier compiler experiment, the generic configuration remained the default. Specialization did not establish a useful gain here; the other compiler settings regressed. GPU execution occupied approximately 98% of measured tracing time, leaving little opportunity for a large speedup from submission overhead alone. GPU execution time includes memory stalls and does not establish high shader occupancy. [Raw compiler experiment results](results/m1-pro-metal-compiler-options.json).

A separate paired experiment stored intersection geometry in a 64-byte stream, retaining all original floating-point values and primitive order. Its generic/compact medians were 6.131/5.931 s, approximately 3.3% less time. That costs about 212 MB of extra GPU buffers in this scene, so it remains explicit experimental tuning. Compare variants within their paired run; the two experiments have different compiled shaders and operating conditions. [Raw geometry experiment results](results/m1-pro-metal-geometry-layout.json).

The later traversal pass below prototypes native Metal traversal and wider BVHs. Separate intersection/shading queues and spatial light sampling remain unimplemented; these earlier compiler experiments alone did not establish a large speedup.

## Point-cloud navigation measurement

The final navigation benchmark walked and looked around the neon apartment for 600 frames, with a native 1280 × 820 window and a 960 × 540 point viewport. It used up to 1.2 million area-sampled points, a depth layer, no textures, adaptive density driven by Metal GPU execution timestamps, and absolute 60 Hz frame deadlines.

Median preview work plus UI drawing was 5.46 ms; the 95th percentile was 12.57 ms. Median presentation/pacing interval was 16.664 ms. Seven of 600 work frames exceeded 16.67 ms, with a maximum of 24.26 ms. This establishes a 60 FPS target with observed misses, not a perfectly locked frame rate. These timings do not include the transition out of an in-flight final-render tile. [Recorded navigation measurements](results/m1-pro-point-navigation.json).

## Lossless traversal experiments (2026-11-28)

The new experiments keep the same primitive equations, surface attributes, texture data, random sequences, precision and light-transport code. `MetalTuning::default()` remains an explicit unoptimized reference for comparisons; production selection is made separately by `MetalRenderer::new`.

- **Split geometry:** positions, edges and primitive metadata occupy a 64-byte stream. Normals and UVs occupy a separate 80-byte stream. Together they retain the original 144 bytes per primitive, removing the extra geometry copy needed by the earlier experiment.
- **Compact binary nodes:** pack indices and tags into the unused fourth lanes of the bounds. Node storage falls from 48 to 32 bytes without quantizing the three-dimensional bounds.
- **Cached ray bounds:** calculate direction reciprocals and parallel-axis flags once per traversal. Conservative slab expansion accounts for reciprocal rounding; the original primitive intersection still determines the hit.
- **Four-way traversal:** collapse two binary levels into one node and test four child bounds together. All coordinates remain full precision. Child visitation follows the original binary split axes, preserving the ordering of coincident surfaces. At most three siblings are pending per two binary levels; 128 stack entries cover the builder's maximum depth of 60.
- **Bounded submissions:** allow at most two independent tile commands in flight. Read only completed tile pixels, drain all submitted work on error/cancellation, and finish the pass before reusing pixels for the next sample range. Individual commands retain the existing adaptive duration target. This bounds outstanding work; it does not promise a hard latency ceiling.

Native Metal acceleration structures were also implemented as an explicit experiment. Custom bounding-box intersections reuse the existing sphere/triangle and opacity equations. Shadow traversal remains software so the current thin-sheet transmission handling is retained. This prototype does **not** provide mesh instancing or a wavefront integrator. It changed raw pixels in the detailed apartment and therefore fails the exact-image gate; it must not be selected for production on the strength of its timing alone.

The exploratory [layout sweep](results/m1-pro-metal-layout-sweep.json) and [traversal experiments](results/m1-pro-metal-traversal-experiments.json) record every variant and raw-pixel result. They are diagnostic screening runs, not delivery-resolution performance claims. Native traversal differed consistently across repetitions; the lossless software variants matched exactly.

### Profiling limits and reproducibility

A programmatic capture of the neon path-tracing dispatch was successfully replayed using Apple's `gpudebug`. Counter collection returned `GPU profiling requires Apple M3 / A17 Pro or later` on this M1 Pro. Consequently this report makes no measured register-occupancy or bandwidth claim. GPU command timestamps and controlled wall-clock comparisons remain available. Apple's guidance explains why reducing memory traffic and register lifetimes can matter, but it does not establish this shader's bottleneck. [GPU performance counters](https://developer.apple.com/documentation/xcode/analyzing-apple-gpu-performance-using-counter-statistics).

For a small diagnostic capture, use an output path that does not already exist:

```bash
MTL_CAPTURE_ENABLED=1 cargo run --release --example benchmark_metal_tuning -- \
  --scene interior-neon --width 128 --height 128 --samples 1 --repeats 1 \
  --variants generic --capture .runtime/performance/neon.gputrace
xcrun gpudebug --gputrace .runtime/performance/neon.gputrace
```

The benchmark fails on any changed raw pixel unless `--allow-differences` is explicitly supplied for experiments. Scene buffer bytes exclude CPU scene storage, temporary acceleration-build scratch, driver/compiler allocations and film buffers; they are not total process memory.

Further work remains for mesh instancing, separate intersection/shading queues, spatial light selection and HDR importance sampling. These are not enabled or claimed as completed by this optimization pass. Sampling changes require statistical error-versus-time validation rather than an exact-pixel comparison.

### Full HD variant selection

The complete 4,705,126-primitive neon apartment was measured at **1920 × 1080, 16 spp, 50 bounces**, with three repetitions and reversed ordering. No other render or build ran during this comparison. Each of the nine images matched the reference's raw linear pixels exactly. [All runs and memory records](results/m1-pro-metal-fullhd-variants.json).

| Configuration | Median tracing time | Scene buffer bytes |
| --- | ---: | ---: |
| Original reference | 52.941 s | 1,681,795,680 |
| Compact binary BVH, split geometry, cached bounds, two commands | 31.501 s | 1,635,822,304 |
| Four-way BVH, split geometry, two commands | 31.097 s | 1,647,354,704 |

The compact binary configuration is selected for production: **1.68× throughput / 40.5% less tracing time** in this comparison, and about 46 MB less scene buffer storage. The four-way configuration's roughly 1.3% median advantage over compact binary is smaller than the observed run-to-run variation; it retains a larger traversal stack and remains an explicit experiment. This is not a completed 2,048-spp timing, nor a claim about every scene or another GPU.

```bash
cargo run --release --example benchmark_metal_tuning -- \
  --scene interior-neon --width 1920 --height 1080 --samples 16 --bounces 50 \
  --repeats 3 --variants generic,production,wide-queued
```

### Rebuilt executable versus the previous release

The final application was compared directly against the saved `ce68a04` executable, using the same assets, seed, fixed sampling and bounce limit. Three pairs alternated execution order. No other rendering or compilation ran during measurement. This is the release speedup result; the internal variant timings above explain configuration selection.

| Pair | Previous executable | Rebuilt executable | Maximum linear pixel error |
| --- | ---: | ---: | ---: |
| 1 | 49.872 s | 30.898 s | 0 |
| 2 | 47.703 s | 31.025 s | 0 |
| 3 | 49.942 s | 30.972 s | 0 |
| Median | **49.872 s** | **30.972 s** | **0** |

This establishes **1.61× tracing throughput / 37.9% less tracing time** on the M1 Pro for the Full HD, 16-spp, 50-bounce neon apartment. Every requested sample completed, with zero invalid samples. Scene loading, compilation, denoising and file output are outside the transport timer. No completed 2,048-spp duration or speed ratio against Cycles is inferred. [Complete paired reports, image errors and binary hashes](results/m1-pro-metal-lossless-fullhd.json).

## Further lossless experiments

The next pass freezes `8743910` as the `previous` benchmark configuration and preserves its executable for application-level comparisons. Merely increasing GPU launch width did not establish a further gain. The benchmark now accepts `--reference previous` so that changes to production defaults cannot silently redefine its reference image.

Two smaller records were evaluated. Geometry can occupy 48 bytes by encoding the primitive kind, material index and light index in unused vector lanes. All positions, edges and sphere radii retain their original FP32 bits. Normals and all six UV coordinates can separately occupy 64 bytes, again retaining every used FP32 bit. The geometry layout validates its metadata range and the ordinary renderer can fall back to the full record if needed. These are storage changes, not mesh simplification or reduced texture precision.

The following Full HD comparison used the complete 4,705,126-primitive neon apartment, 16 samples, 50 bounces and three repetitions with reversed order. All twelve raw images matched exactly. No other render or build ran during these measurements. The first reference run took 44.705 seconds, while the subsequent reference runs took 32.786 and 32.090; all runs are retained in the report rather than selecting the slow first run as a baseline. [Raw measurements](results/m1-pro-metal-records-fullhd.json).

| Configuration | Median tracing time | Scene buffer bytes |
| --- | ---: | ---: |
| Previous production settings | 32.786 s | 1,635,822,304 |
| 48-byte geometry records | 33.702 s | 1,560,540,288 |
| 48-byte geometry and 64-byte shading records | 34.711 s | 1,485,258,272 |
| 256 threads per group instead of 128 | 34.680 s | 1,635,822,304 |

Packing both records saves 150,564,032 bytes of GPU scene buffers, but it did not improve the median tracing time here. Neither packed records nor 256-thread launches are enabled by default. Spatial Morton ordering and a smaller four-way traversal stack also remain explicit experiments after inconclusive screening. A parent-pointer traversal prototype preserved the pixels but regressed in every screening repetition; its implementation was discarded. [Screening records](results/m1-pro-metal-followup-screening.json).

Runtime threadgroup width is distinct from the compiler's maximum-threadgroup setting. Launches remain multiples of the pipeline's execution width and are capped by its supported maximum. Apple's guidance motivates measuring these configurations rather than assuming a larger launch must be faster. [Threadgroup and grid sizes](https://developer.apple.com/documentation/metal/calculating-threadgroup-and-grid-sizes).

One further experiment caches immutable cutout and thin-glass classification in unused primitive metadata. Ordinary visibility checks can then avoid reading a material record. Cutouts still sample their opacity map; thin sheets still evaluate Fresnel transmission; final surface shading and solid dielectric transport are unchanged. It adds no scene buffer storage and is evaluated against the saved executable separately from the smaller-record experiments.

### Direct executable check for visibility classification

The saved `8743910` executable and an executable with visibility classification enabled were measured in three alternating Full HD pairs at 16 samples and 50 bounces. No other render or build ran during this comparison. All requested samples completed, with zero invalid samples and zero maximum EXR error, RMSE and relative L2 error. [Reports and executable hashes](results/m1-pro-metal-visibility-fullhd.json).

| Pair | Previous executable | Visibility classification | Maximum linear pixel error |
| --- | ---: | ---: | ---: |
| 1 | 31.453 s | 31.547 s | 0 |
| 2 | 33.323 s | 32.871 s | 0 |
| 3 | 31.089 s | 32.222 s | 0 |
| Median | **31.453 s** | **32.222 s** | **0** |

The candidate took 2.45% longer at the median and remains disabled. Its small screening advantage did not carry over to the application comparison. **This follow-up pass establishes no additional production speedup.** The previous validated production configuration is retained; smaller records, Morton ordering, visibility flags and alternative launch widths are explicit experiments only.

```bash
cargo run --release --example benchmark_metal_tuning -- \
  --scene interior-neon --width 1920 --height 1080 --samples 16 --bounces 50 \
  --repeats 3 --reference previous \
  --variants previous,visibility,dense-geometry,dense,group-256
```

The new layout tests check coordinate, normal, UV and identifier bits, including signed zero and boundary identifiers. GPU tests check exact final and preview pixels on odd image dimensions, alternate launch widths, tiled/offline agreement, and visibility classification across all six presets. Fewer scene-buffer bytes are useful information, but are not evidence of faster rendering or lower total process memory by themselves.
