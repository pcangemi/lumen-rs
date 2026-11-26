# Validation record

Validated on 2026-11-26 with Apple M1 Pro, 10 CPU threads and Blender 5.3.0.

## Current automated checks

- Rust formatting and Clippy with warnings denied: passed.
- Standard release physics suite: 24 passed; two GPU tests are deliberately ignored in this suite.
- Metal shader compilation: passed.
- Metal runtime regressions on the M1 Pro: two passed, covering all five presets, finite samples, exact batch invariance and partial film edges.
- Night lighting regression: all three lamp emitters are active, the pendant retains its 80 mm bulb envelope, and daylight emitters are absent.
- All seven public street lamps are active; any projected within the camera field of view are occluded from direct sight.
- Canvas alignment, architectural clearances and outward wavy-glass boundaries: passed.
- Downward ray probes beneath the coffee table verify raised wool between its supports.
- Thin diffuse transmission, analytic thin-glass shadow losses and MIS agreement with pure path sampling: passed.
- Cycles diffuse-transmission and thin-glass material graph construction in Blender 5.3: passed; these are graph checks, not a new full-scene benchmark.
- Latest published checks are available in the repository's [Actions history](https://github.com/pcangemi/lumen-rs/actions).

The standard suite also covers linear PBR maps, mirrored tangent frames, leaf opacity for camera and shadow rays, relative OBJ texture paths, curtain and radiator clearances, and viewer tile outlines over the clean preview. Optional OIDN and Cycles export checks from the preceding furniture update remain recorded below; they were not rerun for the day/night geometry update.

## Current CPU/Metal regression

73 × 47 pixels, 64 samples per pixel, eight bounces. Values below are sums of linear pixel luminance, not timing measurements.

| Scene | CPU | Metal |
| --- | ---: | ---: |
| Studio | 777.843 | 777.853 |
| Interior | 641.695 | 638.566 |
| Interior night | 205.806 | 206.368 |
| Outdoor | 845.800 | 845.527 |
| Clouds | 1222.783 | 1222.789 |

Mean luminance differs by less than 0.5% in this run. GPU results are exactly equal across requested batch limits and offline/progressive rendering, including adaptive command sizing. A separate 277 × 71 render checks non-divisible film edges and a 17-sample final partial pass. Statistical CPU/GPU agreement does not establish equality of every path.

## Pendant light reaching the floor

`cargo run --release --example pendant_probe` measures the pendant's direct incident irradiance over 8 cm radius patches, using 4,096 samples per patch and the scene's actual shadow transmittance. The patches sit above the rug pile or parquet, so this isolates source geometry, glass transmission and occlusion from surface scattering, camera exposure and denoising. Values are relative scene units, not calibrated lux. The [recorded probe output](results/m1-pro-pendant-irradiance.json) contains RGB values and exact positions.

| Patch | Direct irradiance relative to the patch below the pendant | Fraction remaining after glass and occlusion |
| --- | ---: | ---: |
| Rug below pendant | 100.0% | 73.5% |
| Rug in front of table | 98.8% | 79.5% |
| Foreground rug | 19.6% | 32.9% |
| Under coffee table | 0.0% | 0.0% |
| Left parquet | 0.0% | 0.0% |
| Foreground parquet | 2.1% | 6.4% |
| Ceiling beside pendant | 55.2% | 23.9% |

The pendant does illuminate the rug below it. Direct light is much weaker in the foreground, and occluded at the sampled points under the table and on the left parquet. Zero direct light does not imply zero indirect light. These local measurements do not validate the complete image or its wool scattering. Increasing sample count reduces uncertainty; it does not remove geometric shadows or distance falloff.

The same probe traces paired normal-view surface paths with all lights at depth limits of 12 and 50, sharing random numbers until the shorter path ends. Across the six sampled floor patches, increasing the limit adds approximately 0.01–1.1% to estimated luminance. The larger differences are noisy: the patch under the table gains 0.000155 scene luminance with standard error 0.000143. These estimates provide no evidence that a severe loss of long paths explains the dark foreground. The report retains paired standard errors; neither depth is an infinite-depth reference, and these patch views do not replace a full camera-image comparison.

## Metal scheduling

The dense apartment exposed command-buffer failures, including a user-reported `Impacting Interactivity` diagnostic. Offline full-film dispatch was replaced by 64 × 64 tiles. Both modes now start at one sample per command and use the previous pass's slowest measured GPU command to size the next pass, targeting 20 ms and growing at most twofold up to 16 samples. Every requested sample is retained, and there are no artificial delays. CPU/GPU transfer and scene construction remain outside these command timings.

The target is a heuristic, not a promise that future commands cannot exceed it. Reports include command count, total GPU command time, longest command and maximum samples per command. Error reports retain the driver message, tile and sample range. Failed commands are not retried because their writes may be incomplete. Native desktop responsiveness and competing GPU workloads still need interactive assessment.

Ten [IORegistry readings](results/m1-pro-daynight-gpu-activity.json) sampled five seconds apart during an earlier daylight run at commit `2825875` reported 99–100% GPU device utilization. These are system-wide counters, not exclusive utilization by this process or proof of optimal kernel efficiency.

An interactive viewer was also running during that earlier daylight run. It completed 128 samples per pixel at Full HD with zero invalid samples in 718.65 seconds of transport and 3.51 seconds of filtering. Its 64,770 GPU commands took at most 32.33 ms each. This validates a shared-workload run of the scheduler; it is not an isolated throughput benchmark of the revised scene.

## Images

The facade now reuses the apartment's plaster and linen texture handles. The [deterministic comparison](results/m1-pro-texture-reuse.json) records 45 → 43 atlas entries and 33,587,232 fewer packed scene bytes. Day and night linear EXR files are byte-for-byte identical before and after this change at 73 × 47, 64 samples and eight interactions on CPU. This checks pixel equivalence and storage, not a speedup. The Full HD exports below retain their measured settings and timings from `397a067`, before this allocation-only change.

The opposite facade contains stone courses, recessed glazed rooms, window profiles, sills, cornices, shutters, railings and downpipes. Seven street lamps sit on a street 7.3 m below the apartment floor. The night region study at 650 × 430 pixels and 128 samples completed with zero invalid samples in 143.69 seconds of transport, with a maximum GPU command time of 35.61 ms. An interactive viewer was also running, so this is a shared-workload observation.

`cargo run --release --example streetlight_probe` identifies reflected paths in two camera regions. The [recorded result](results/m1-pro-streetlight-reflections.json) includes a path at pixel (661, 533): camera → opposite glazing → lantern glass → public lamp at (−5.7, −0.1, −24.3) m. Fifteen of the 68,000 sampled camera rays found such a path. This establishes the geometric origin of a visible highlight; it is not a full-image convergence or radiometric accuracy measurement. The probe follows one reflection and thin-glass transmission, stopping at opaque or solid-refractive surfaces.

An earlier rug generator compressed the entire rectangular footprint of the coffee table. This exposed smoother backing between its legs and created a pale rectangular patch. Compression now follows the two measured support rails, with clearance for each yarn loop and a smooth transition to uncompressed pile. This is a geometry correction; additional samples or filtering could not repair the original patch.

`cargo run --release --example render_region -- interior 860 590 430 240 256 renders/rug-contact.png` traces a region of the Full HD camera at its original pixel scale, saving raw and filtered output for focused inspection. It shifts the camera frustum instead of resizing an image. On macOS it uses native Metal; other platforms use the CPU backend. OIDN is required for the filtered output.

The matching daylight and night apartment previews use 1920 × 1080, native Metal and Open Image Denoise. The daylight image uses 128 samples and the night image uses 256. Per-image JSON reports record actual settings, transport timings, denoising timings and dispatch measurements. The viewer snapshot reuses the finished daylight EXR through `examples/viewer_snapshot.rs`; it does not retrace the scene.

The Full HD exports at source commit `397a067` completed with zero invalid samples and no Metal command failure. Both use a 12-interaction limit. These are development runs, not an isolated Cycles comparison.

| Scene | Samples per pixel | Transport | Denoising | Longest GPU command |
| --- | ---: | ---: | ---: | ---: |
| interior | 128 | 617.47 s | 3.41 s | 35.11 ms |
| interior-night | 256 | 1729.23 s | 2.52 s | 73.20 ms |

The current apartment contains about 3.24 million primitives, including 58,200 curved rug loops, physical interlaced rattan, source lamp fittings, an 80 mm clear bulb envelope, transmitting shades, white timber joinery, connected cables, irregular curtains, balcony scrollwork and a centered public-domain Monet canvas. The lamps are generic existing source models; their emission is scene-tuned rather than calibrated in lumens. Rug loops use the opaque surface BRDF, not a dedicated fiber-scattering model. The source sofa uses an adapted diffuse/GGX fabric material rather than its original velvet sheen shader. These are development previews, not a claim of photographic equivalence or a controlled performance comparison.

Studio and courtyard previews retain their earlier CPU renders at 900 × 600 and 256 samples. The cloud study uses 720 × 480, 256 samples and 24 bounces. Raw PNG and EXR files remain local; large raw outputs are excluded from Git.

## Archived M1 Pro comparison (earlier scene)

This run used the earlier 55,438-primitive interior, before the furniture, foliage and rug update. Its timings do not describe the current scene. The run completed on Metal with Blender 5.3.0: 640 × 426, 128 samples, 12 bounces, three candidate runs per renderer and independent 2,048-sample references. Adaptive sampling, clamping and denoising were disabled.

| Renderer | Median process wall time | Median relative L2 error against its own reference |
| --- | ---: | ---: |
| Lumen / Metal | 26.58 s | 6.91% |
| Cycles / Metal | 20.15 s | 6.04% |

Cycles was faster and had lower measured error at this setting. This is not an equal-quality speed ratio: materials are mapped approximately and references retain noise. The full [machine-readable report](results/m1-pro-interior-raw.json) includes every run, settings, image errors, device and timing scopes.

Lumen's reference took 430.50 seconds of transport. Cycles' first reference render invocation took 598.44 seconds, including its initial Metal shader compilation. That cold reference invocation must not be used as evidence that Cycles renders more slowly. Candidate timings were measured afterward. Subsequent first-time shader compilation may still vary with Blender, driver and material configurations.

## Earlier integration checks

The previous furniture update passed optional OIDN noise-reduction and asynchronous viewer-worker tests. The worker delivered the final clean image before its completion event. Water and homogeneous-fog JSON smoke renders produced finite output. Cycles CPU smoke renders exercised the exported normal, roughness and opacity maps. These historical checks are not new performance measurements of the day/night scene.

Both rendering executables default to 1920 × 1080. The viewer filters on a separate worker, uses a bounded queue, retains tile activity in clean mode and shows the clean snapshot's sample count. It waits for the final clean image before reporting completion.

## Remaining validation

A controlled error-versus-time curve at delivery resolution remains necessary. The archived comparison establishes a baseline, not performance leadership. Repeat across sample counts and scene families, and evaluate denoised output separately.

Native window event handling and long-session GPU memory behavior require interactive desktop testing. Headless screenshots do not establish those properties. Specialized caustic convergence, production asset compatibility and full color-management accuracy remain separate development work.
