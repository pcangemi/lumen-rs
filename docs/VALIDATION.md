# Validation record

Rust and Metal checks updated on 2026-11-28 with Apple M1 Pro and 10 CPU threads. Blender 5.3.0 checks below are retained from earlier validation runs.

## Coastal villa update

The new `coastal-villa` preset uses licensed source meshes and 2K material maps,
with 4 cm blue/gold mosaic geometry and a closed wind-displaced water volume.
The new gravity-wave surface is deterministic and its perimeter meets the
closed side faces. Water-grid vertices are evaluated once and shared during
assembly; existing standing-wave presets retain their surface function.

- Formatting, all-target Clippy with warnings denied and the standard release
  suite passed: 18 library, 5 viewer and 29 physics tests (52 total).
- Metal shader compilation and all 10 native GPU regressions passed, including
  the new scene, exact batch invariance, camera resets and queued cancellation.
- All 56 downloaded source files passed their pinned SHA-256 checks; converted
  material texture references resolve locally.
- Full HD wide and pool-detail renders use 50 bounces, zero adaptive stopping,
  original mesh detail and source texture resolution. Raw PNG/EXR files remain
  beside the denoised previews locally.
- Render records: [villa](../renders/coastal-villa.json) and
  [pool detail](../renders/coastal-pool.json). These are individual render timings,
  not a controlled benchmark or a comparison with Cycles.

The final wide view contains 6,148,756 primitives, 3,827,343 BVH nodes and 83
textures (5,420,733,904 packed scene bytes). At 1920 × 1080 and 256 samples,
transport took 123.42 seconds and denoising took 3.40 seconds. It completed with
zero invalid samples and a longest GPU command of 25.25 ms. These measurements
include the complete source meshes and mosaic geometry. The pool close-up uses
512 samples at the same resolution and bounce limit: 517.10 seconds of transport,
3.25 seconds of denoising, zero invalid samples and a longest GPU command of
28.60 ms. Neither final render reported a Metal command-buffer failure.

## Current automated checks

- Rust formatting and Clippy with warnings denied: passed.
- Release library suite: 18 passed; the optional OIDN test is excluded.
- Release viewer suite: five passed; the optional worker/denoiser test is excluded.
- Standard release physics suite: 29 passed; ten GPU tests are deliberately ignored in this suite and run separately below.
- Metal shader compilation: passed.
- Metal runtime regressions on the M1 Pro: ten passed, covering all seven presets, finite samples, exact batch invariance, partial film edges, adaptive stopping, micrometer-scale triangle visibility, fine smoke attenuation, point-preview depth/orientation, dense records, camera resets and cancellation.
- Viewer cache regressions: passed for changed pixels, exposure, resizing, raw/clean selection and live tile outlines. The optional worker test delivered the final denoised image before completion.
- Night lighting regression: all three lamp emitters are active, the pendant retains its 80 mm bulb envelope, and daylight emitters are absent.
- All seven public street lamps are active; any projected within the camera field of view are occluded from direct sight.
- Canvas alignment, architectural clearances and outward wavy-glass boundaries: passed.
- Downward ray probes beneath the coffee table verify raised wool between its supports, bounded pile height and clearance beneath the support rails.
- Thin diffuse transmission, analytic thin-glass shadow losses and MIS agreement with pure path sampling: passed.
- Cycles opaque, diffuse-transmission, glass, thin-glass and emission material graph construction in Blender 5.3: passed. A 465,600-sphere native point-cloud CPU smoke render also passed with finite output and 85.99% geometric coverage; this is not a full-scene quality benchmark.
- Latest published checks are available in the repository's [Actions history](https://github.com/pcangemi/lumen-rs/actions).

The standard suite also covers linear PBR maps, mirrored tangent frames, leaf opacity for camera and shadow rays, relative OBJ texture paths, curtain and radiator clearances, and viewer tile outlines over the clean preview. Optional OIDN and Cycles export checks from the preceding furniture update remain recorded below; they were not rerun for the day/night geometry update.

## Current CPU/Metal regression

73 × 47 pixels, 64 samples per pixel, eight bounces. Values below are sums of linear pixel luminance, not timing measurements.

| Scene | CPU | Metal |
| --- | ---: | ---: |
| Studio | 777.850 | 777.856 |
| Interior | 168.296 | 168.253 |
| Interior night | 258.171 | 260.459 |
| Interior neon | 454.438 | 454.906 |
| Outdoor | 845.823 | 845.540 |
| Clouds | 1222.799 | 1222.799 |
| Coastal villa | 1461.806 | 1460.378 |

Mean luminance differs by less than 0.9% in this run (0.10% for the coastal villa). GPU results are exactly equal across requested batch limits and offline/progressive rendering, including adaptive command sizing. A separate 277 × 71 render checks non-divisible film edges and a 17-sample final partial pass. Statistical CPU/GPU agreement does not establish equality of every path.

## Perimeter lighting and script neon

The `interior-neon` preset adds wall-mounted LED cornices around all four walls, replacing the solid ceiling cornice only in this variant. The emitter channel is about 24 cm below the ceiling, with an opaque raised lip. Ray probes verify that every strip can illuminate the ceiling and is hidden from an eye position below it. No isolated central ceiling frame remains.

The pendant, sofa, table and rug use the large wall panel's centre line. The 2.4 m neon sign is centred above the sideboard. Original italic centre lines retain a level baseline and underline endpoints; crossovers lift toward the room. Separate emitting cores, thin-shell Fresnel glass, electrode fittings, clips and connected wiring form the sign. The glass jacket does not model cylindrical refraction or dispersion. Source intensities are scene-tuned RGB values, not measured lumens.

The library suite covers alignment, level lettering, finite neon geometry, glass transmission and real light visibility. In the current 73 × 47 / 64-sample / eight-bounce check, the neon scene's CPU and Metal luminance sums differ by about 0.10%; Metal images are exactly equal across batching and offline/progressive submission. This validates transport consistency, not photographic fidelity or manufacturing accuracy.

The antique panes have closed, nonparallel surfaces with submillimeter waviness. Tests verify outward entry/exit surfaces, approximately 6 mm thickness and small but nonzero changes in transmitted ray direction.

`cargo run --release --example render_neon_detail -- 128` renders a front-on Full HD detail using the same scene and materials, preserving raw and denoised output. The close-up camera sits beyond the pendant envelope; no scene geometry is hidden for that view.

The sign-detail image is a 1920 × 1080 preview at 64 samples per pixel and 24 maximum interactions. It predates the sculpture addition and uses 2,190,074 primitives and 43 textures. It completed with zero invalid samples, no Metal command failure and a longest GPU command of 107.77 ms. Open Image Denoise filters the displayed copy; raw PNG/EXR files are retained locally. The current room preview is recorded with the incense below.

## Glass guardian lion

The `interior-neon` scene imports the supplied SolidXperts guardian-lion scan as solid clear glass, including its pedestal. The [conversion report](../assets/models/guardian_lion/conversion.json) pins the original archive and output checksums. Local repairs remove detached scan fragments, level the underside and close scan openings. The final mesh has 986,376 triangles, one connected shell and no boundary or nonmanifold edges. The carved surface is not decimated or globally smoothed. Source attribution and its separate CC BY-NC 4.0 license are in the [asset notice](../assets/models/guardian_lion/NOTICE.md).

The sculpture now stands 1.90 m tall including its pedestal, centred at (1.20, 0, 2.20) m, farther into the room. Its separate rug is 1.76 m in diameter to fit the larger pedestal. Yarn spacing and thickness retain their original physical scale, while the compressed support region follows the enlarged base. About 31 cm of parquet separates the two rugs.

The scene regression verifies the full height, support level, IOR 1.52 and exact retention of every converted triangle. It checks all rug and sculpture bounds for at least 29 cm of clearance beyond the main rug and clearance from the wall. Across 243 ray probes on three axes, glass crossings alternate between entry and exit and finish in air. These probes supplement the conversion's topology checks; they do not prove the absence of every possible self-intersection in the scan.

Import formerly discarded small but valid triangles, and a fixed ray/triangle determinant threshold could miss them. Import now uses the existing scene-area tolerance, while CPU and Metal use a relative cancellation threshold for intersection. A CPU regression retains and intersects all faces of tetrahedra at four scales down to 10 micrometers. A separate Metal regression sees an 8-micrometer emissive quad identically to the CPU, with zero invalid samples.

The room preview at `f437881`, before the incense addition, used native Metal at 1920 × 1080, 64 samples per pixel and 50 maximum interactions. It completed with zero invalid samples and no command failure: 384.64 s transport, 2.14 s denoising and 114.91 ms longest GPU command. That scene contained 3,311,866 primitives, 2,061,337 BVH nodes and 43 textures, using 1,307,970,192 packed scene bytes. These timings overlapped an active interactive viewer and are not a controlled performance benchmark. The current room image is recorded with the incense below.

`cargo run --release --example render_sculpture_detail -- 256` renders a Full HD close-up in the same apartment lighting without removing scene geometry. The material supports Fresnel reflection, refraction, total internal reflection and weak volume absorption. It has no opacity trick, dedicated caustic solver or spectral dispersion. The source scan's weathering remains in the surface geometry; assigning clear glass does not turn its carved surface into optically flat glass.

The published [sculpture close-up](../renders/sculpture-detail.png), rendered before the incense addition, uses 1920 × 1080, 64 samples per pixel and 50 maximum interactions. It completed with zero invalid samples and no command failure: 371.56 s transport, 1.99 s denoising and 111.14 ms longest GPU command. It uses that revision's complete geometry and textures. An interactive viewer was running during this export as well, so the timing is not a controlled benchmark. These denoised 64-sample images are previews, not convergence references; raw PNG and EXR files are retained locally.

## Incense volume

The `interior-neon` preset adds an original glazed dish, brass support, coated bamboo stick, ash and a small emitting ember beside the sideboard lamp. A 78 cm plume begins just above its tip. The density field uses a narrow stem that rolls into twisting, widening wisps and fades at the top. Its shape is static and procedural. Scattering albedo, phase anisotropy and extinction are scene-tuned values, not measurements of a particular incense product.

The smoke uses the existing participating-medium integrator for camera paths, indirect paths and shadow attenuation. The implementation retains the 64-byte CPU/Metal medium layout and adds no voxel allocation. Its majorant is 75 inverse meters; density stays in [0, 1]. The [transmittance methods](https://pbr-book.org/4ed/Volume_Scattering/Transmittance) are delta tracking for collision sampling and ratio tracking for shadow connections. The medium's bounding box is 43.68 cm wide and deep, localized above the stick.

The CPU regression checks bounds and density range, then compares 20,000 transmission and escape estimates at three plume heights with independent 2,048-step extinction integrals. Each estimate is within 0.012 absolute transmittance of its numerical reference. The focused Metal regression uses an absorbing plume in front of an emissive background at 48 × 32 and 512 samples. CPU and Metal attenuation sums both report 13.670 and their pixel mean absolute difference reports 0.000000, with zero invalid samples. Whole-scene batch invariance and offline/progressive equivalence also pass.

`cargo run --release --example render_incense_detail -- 256` renders the same apartment lighting at Full HD from a camera facing the stick and lamp. Raw PNG and linear EXR output are preserved alongside the denoised image. Because the comparison exporter does not support participating media, it explicitly rejects this preset instead of dropping the smoke; the other apartment presets are still exportable.

The [incense close-up](../renders/incense-detail.png) completed on native Metal at 1920 × 1080, 128 samples per pixel and 50 maximum interactions, with zero invalid samples and no command failure. Transport took 405.75 s, denoising 2.25 s and the longest GPU command 46.52 ms. The scene has 3,313,317 primitives, 2,062,183 BVH nodes and 43 textures. The addition uses 1,451 extra primitives and 250,144 additional packed scene bytes; the density field requires no voxel storage. This is a preview run, not an isolated throughput or convergence benchmark.

The updated [room preview](../renders/interior-neon.png) uses the same complete scene at 1920 × 1080, 64 samples per pixel and 50 maximum interactions. It completed with zero invalid samples and no command failure: 221.46 s transport, 7.04 s denoising and 66.46 ms longest GPU command. The displayed image is denoised, with the raw PNG and linear EXR retained locally. This single run does not establish a performance change relative to older exports made under different concurrent workloads.

## Metal throughput with identical pixels

The [paired performance report](results/m1-pro-metal-throughput.json) compares the executable from `9e40a6f` with the optimized renderer on the same M1 Pro. Both use the complete night apartment at **1920 × 1080, 16 samples per pixel, 50 bounces**, seed 42, with adaptive sampling and denoising disabled. Three pairs alternate execution order. The viewer was paused; exploratory runs overlapping an active viewer are excluded.

| Transport time | Previous executable | Optimized executable |
| --- | ---: | ---: |
| Pair 1 | 68.631 s | 36.599 s |
| Pair 2 | 77.382 s | 36.286 s |
| Pair 3 | 77.009 s | 39.757 s |
| Median | **77.009 s** | **36.599 s** |

Measured throughput improved **2.10×**, or **52.5% less transport time**. All six runs retained 1,818,546 primitives, 1,108,345 BVH nodes, 43 textures and every requested sample. Each pair's raw linear EXR comparison has zero maximum absolute error and zero RMSE. No invalid samples or command failures occurred. The longest optimized GPU command across the three runs was 43.08 ms.

Metal now dispatches up to 16,384 independent pixels per tile instead of 4,096, while retaining 128 threads per threadgroup on this device and timing-based sample batches. Intersection traversal defers shading attributes until needed, opaque visibility avoids them entirely, and fixed-sample renders omit unused variance calculations. The scene, sampling sequence, radiance accumulation, materials and bounce limit remain unchanged.

These timings cover transport and its submission/readback overhead, excluding scene loading, shader compilation, file output and denoising. This is a 16-sample throughput benchmark, not a completed 2,048-sample render or a final-image convergence claim. It does not establish a 2.10× gain on every scene, machine or concurrent workload.

The viewer additionally caches unchanged display pixels and buffers up to 64 worker events. `cargo run --release --example benchmark_viewer` compares cached and uncached drawing with identical output. This CPU drawing measurement is separate from the transport ratio above; native interactive performance still depends on the window and other applications.

## Small emitter visibility

The earlier sphere quadratic subtracted two scene-scale squared distances. Its cancellation error could falsely classify a ray ending just before a small bulb as intersecting that bulb, suppressing a valid direct-light connection. The CPU and Metal implementations now use the perpendicular distance to the ray's closest approach and normalize the surface normal.

An isolated check sampled a sphere of radius 5.5 mm centered at (0.2, 2.533, -1.35) m from two floor locations. Of 4,082 visible connections, the previous formula falsely reported 1,470 occlusions; the corrected intersection reported zero. This checks a numerical error at the emitter endpoint, independently of the room geometry or sample convergence. The permanent regression asserts that all those visible connections remain clear. Another regression checks millimeter-scale hits, misses and inside exits up to 30 m away.

## Pendant light reaching the floor

`cargo run --release --example pendant_probe` measures the pendant's direct incident irradiance over 8 cm radius patches, using 4,096 samples per patch and the scene's actual shadow transmittance. The patches sit above the rug pile or parquet, so this isolates source geometry, glass transmission and occlusion from surface scattering, camera exposure and denoising. Values are relative scene units, not calibrated lux. The [recorded probe output](results/m1-pro-pendant-irradiance.json) contains RGB values and exact positions.

| Patch | Direct irradiance relative to the patch below the pendant | Fraction remaining after glass and occlusion |
| --- | ---: | ---: |
| Rug below pendant | 100.0% | 84.9% |
| Rug in front of table | 98.7% | 91.8% |
| Foreground rug | 26.1% | 50.6% |
| Under coffee table | 0.0% | 0.0% |
| Left parquet | 0.0% | 0.0% |
| Foreground parquet | 3.4% | 11.8% |
| Ceiling beside pendant | 49.1% | 24.5% |

The pendant does illuminate the rug below it. Direct light is much weaker in the foreground, and occluded at the sampled points under the table and on the left parquet. Zero direct light does not imply zero indirect light. These local measurements do not validate the complete image or its wool scattering. Increasing sample count reduces uncertainty; it does not remove geometric shadows or distance falloff.

The same probe traces paired normal-view surface paths with all lights at depth limits of 12 and 50, sharing random numbers until the shorter path ends. Across the six sampled floor patches, increasing the limit adds approximately 0.01–0.13% to estimated luminance. The larger differences are noisy: the patch in front of the table gains 0.0000854 scene luminance with standard error 0.0000778. These estimates provide no evidence that a severe loss of long paths explains the dark foreground. The report retains paired standard errors; neither depth is an infinite-depth reference, and these patch views do not replace a full camera-image comparison.

## Metal scheduling

The dense apartment exposed command-buffer failures, including a user-reported `Impacting Interactivity` diagnostic. Offline full-film dispatch was initially replaced by 64 × 64 tiles. The current Metal tile is 128 × 128, providing four times as many independent pixel paths per full dispatch. Both modes start at one sample per command and use the previous pass's slowest measured GPU command to size the next pass, targeting 20 ms and growing at most twofold up to 16 samples. Every requested sample is retained, and there are no artificial delays. CPU/GPU transfer and scene construction remain outside these command timings.

The target is a heuristic, not a promise that future commands cannot exceed it; a one-sample tile can exceed the target. Reports include command count, total GPU command time, longest command, maximum samples per command, tile size and threads per threadgroup. Error reports retain the driver message, tile and sample range. Failed commands are not retried because their writes may be incomplete. Native desktop responsiveness and competing GPU workloads still need interactive assessment.

Six [IORegistry readings](results/m1-pro-boucle-gpu-activity.json) taken five seconds apart during the revised Full HD night render reported 92–97% GPU device utilization. These are system-wide counters, not exclusive process occupancy or proof of optimal kernel efficiency.

Ten [IORegistry readings](results/m1-pro-daynight-gpu-activity.json) sampled five seconds apart during an earlier daylight run at commit `2825875` reported 99–100% GPU device utilization. These are system-wide counters, not exclusive utilization by this process or proof of optimal kernel efficiency.

An interactive viewer was also running during that earlier daylight run. It completed 128 samples per pixel at Full HD with zero invalid samples in 718.65 seconds of transport and 3.51 seconds of filtering. Its 64,770 GPU commands took at most 32.33 ms each. This validates a shared-workload run of the scheduler; it is not an isolated throughput benchmark of the revised scene.

## Images

The facade now reuses the apartment's plaster and linen texture handles. The [deterministic comparison](results/m1-pro-texture-reuse.json) records 45 → 43 atlas entries and 33,587,232 fewer packed scene bytes. Day and night linear EXR files are byte-for-byte identical before and after this change at 73 × 47, 64 samples and eight interactions on CPU. This checks pixel equivalence and storage, not a speedup. The current Full HD exports include both the shared atlas handles and the revised boucle geometry.

The opposite facade contains stone courses, recessed glazed rooms, window profiles, sills, cornices, shutters, railings and downpipes. Seven street lamps sit on a street 7.3 m below the apartment floor. The night region study at 650 × 430 pixels and 128 samples completed with zero invalid samples in 143.69 seconds of transport, with a maximum GPU command time of 35.61 ms. An interactive viewer was also running, so this is a shared-workload observation.

`cargo run --release --example streetlight_probe` identifies reflected paths in two camera regions. The [recorded result](results/m1-pro-streetlight-reflections.json) includes a path at pixel (661, 533): camera → opposite glazing → lantern glass → public lamp at (−5.7, −0.1, −24.3) m. Fifteen of the 68,000 sampled camera rays found such a path. This establishes the geometric origin of a visible highlight; it is not a full-image convergence or radiometric accuracy measurement. The probe follows one reflection and thin-glass transmission, stopping at opaque or solid-refractive surfaces.

An earlier rug generator compressed the entire rectangular footprint of the coffee table. This exposed smoother backing between its legs and created a pale rectangular patch. Compression now follows the two measured support rails, with clearance for each yarn loop and a smooth transition to uncompressed pile. This is a geometry correction; additional samples or filtering could not repair the original patch.

The revised rug replaces the tall, narrow polygonal strands with 58,200 short boucle loops, each formed from eight overlapping rounded beads. Cord radii range from 3.2 to 4.4 mm, and free loop heights from 9 to 16 mm. Wool color, roughness and attenuated normal maps add surface variation. The long fringe is removed. The scene now contains 1,818,548 primitives by day, 43.9% fewer than the preceding version, and uses 239,351,136 fewer packed scene bytes (228.26 MiB). This is a storage and geometry reduction; it is not an equal-quality speed measurement. The [recorded report](results/m1-pro-boucle-rug.json) includes the focused render and Cycles import check.

`/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/check-cycles-spheres.py` checks the dense-sphere import without generating one object per bead. Native point-cloud spheres keep world-projected color, roughness and normal maps. Shading-normal correction and the BSDF still differ between renderers.

`cargo run --release --example render_region -- interior 860 590 430 240 256 renders/rug-contact.png` traces a region of the Full HD camera at its original pixel scale, saving raw and filtered output for focused inspection. It shifts the camera frustum instead of resizing an image. On macOS it uses native Metal; other platforms use the CPU backend. OIDN is required for the filtered output.

The matching daylight and night apartment previews use 1920 × 1080, native Metal and Open Image Denoise. The daylight image uses 128 samples and the night image uses 256. Per-image JSON reports record actual settings, transport timings, denoising timings and dispatch measurements. The viewer snapshot reuses the finished daylight EXR through `examples/viewer_snapshot.rs`; it does not retrace the scene.

The Full HD exports at source commit `8c21a72` completed with zero invalid samples and no Metal command failure. Both use a 12-interaction limit. These are development runs, not an isolated Cycles comparison.

| Scene | Samples per pixel | Transport | Denoising | Longest GPU command |
| --- | ---: | ---: | ---: | ---: |
| interior | 128 | 457.23 s | 2.10 s | 35.06 ms |
| interior-night | 256 | 954.42 s | 2.21 s | 29.61 ms |

The previous Full HD previews at `397a067` took 617.47 s by day and 1729.23 s by night. Geometry, materials and numerical behavior changed in the current revision; these observations are not a controlled equal-quality speed comparison.

The day and original night apartment variants contain about 1.82 million primitives, including 58,200 rounded rug loops, physical interlaced rattan, source lamp fittings, an 80 mm clear bulb envelope, transmitting shades, white timber joinery, connected cables, irregular curtains, balcony scrollwork and a centered public-domain Monet canvas. The lamps are generic existing source models; their emission is scene-tuned rather than calibrated in lumens. Rug loops use the opaque surface BRDF, not a dedicated fiber-scattering model. The source sofa uses an adapted diffuse/GGX fabric material rather than its original velvet sheen shader. These are development previews, not a claim of photographic equivalence or a controlled performance comparison.

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


## Interactive viewpoints and Metal tuning

The viewer now retains the scene and Metal buffers across camera changes. Camera requests coalesce, old results carry generation tags, and cancelled interactive films can be discarded without a full-resolution readback. Point previews and final film buffers remain separate. Saved cameras round-trip through JSON and can be loaded by either executable.

Native point projection is checked for correct depth ordering, clipping of points behind the camera, odd image widths, and repeatability after camera movement. Resident path tracing is compared against a fresh film after previews and camera changes. Compiler and geometry-layout experiments check exact raw linear pixel equality.

The release validation passed 53 library, viewer and physics tests, including the optional OIDN checks and seven native Metal tests. GPU tests ran serially. Formatting, Clippy with warnings denied, compilation of both Metal shaders, and release executable builds passed. Saved-viewpoint smoke renders completed through both the Metal command-line renderer and the headless viewer.

[Navigation measurements](results/m1-pro-point-navigation.json) include 600 native-window frames with walking, looking, GPU readback and interface drawing. The median work time was 5.46 ms; seven frames exceeded 16.67 ms. The software pacing median was 16.664 ms. This is not a guarantee of locked 60 FPS, and it does not describe final-render speed.

The [Metal performance report](metal-performance.md) records the official Apple references, measured compiler regressions and an experimental compact-geometry layout. Those earlier compiler/duplicated-geometry variants were not enabled by default. The later lossless layout pass is documented in the same report. Final resolution, physical materials, geometry, sampling and bounce settings remain unchanged.

### Native navigation input regression

The initial navigation benchmark drove the camera directly and did not test window focus or keyboard input. Native testing subsequently reproduced an inverted macOS focus result in minifb 0.28: its `is_active()` returned false for a focused window. The viewer now checks AppKit's [key window](https://developer.apple.com/documentation/appkit/nswindow/iskeywindow) and [active application](https://developer.apple.com/documentation/appkit/nsapplication/isactive) directly on the UI thread.

`cargo run --example check_viewer_input` opens a temporary macOS window and dispatches local AppKit key events. It verifies WASD and arrow movement without a mouse button, F and Tab handling, the native right-mouse handlers, and rejection of movement after losing focus. It does not send global input or require accessibility permissions. Unit tests also cover AZERTY aliases and Cmd/Ctrl+S without camera movement. F enters the navigation preview; S and D are exclusively movement keys, with image saving on Cmd/Ctrl+S and Raw/Clean on Tab.

## Lossless Metal traversal and scheduling

The release suite passed **49 standard tests and all 9 native Metal tests**, with GPU tests run serially. Formatting, Clippy with warnings denied, shader compilation and release executable builds passed. The new checks verify compact-node packing, all eight direction-octant traversal orders, exact production/reference/wide-BVH pixels across all six presets, tiny geometry through native custom intersections, and film recovery after cancellation with two submitted commands.

The production configuration uses lossless split geometry/shading buffers, compact binary BVH nodes, cached ray/bounds calculations and at most two tile commands in flight. Queued work is drained before film reuse even on errors, and CPU readback accesses only the completed tile's pixels. Geometry, texture values, random samples, bounce settings and radiance calculations retain the reference representation.

The [Full HD variant comparison](results/m1-pro-metal-fullhd-variants.json) measured the 4,705,126-primitive neon scene at 16 samples and 50 bounces over three repetitions. Reference/selected medians were 52.941/31.501 seconds, with exact raw-pixel equality and zero invalid samples. It measures transport, excluding setup and denoising, and is not a prediction of a completed 2,048-sample render. The four-way and native Metal prototypes remain explicit experiments; native traversal failed the apartment's exact-pixel gate.

The subsequent [direct executable comparison](results/m1-pro-metal-lossless-fullhd.json) used the saved `ce68a04` binary and the rebuilt release with identical assets. Three alternating pairs measured **49.872 → 30.972 seconds** at the median: **1.61× throughput, 37.9% less tracing time**. All six renders completed every requested sample with zero invalid samples; every paired linear EXR comparison had zero maximum error, RMSE and relative L2 error. This is the release comparison; the variant comparison above was used to select the implementation.

## Follow-up Metal experiment gates

The follow-up suite passes **50 standard tests and 10 native Metal tests**. The added layout checks preserve every used coordinate, normal, UV and metadata bit; GPU checks exercise odd tiles, preview/final agreement, alternate launch widths and exact visibility-classification output across all six presets. Formatting, release Clippy with warnings denied, shader compilation and release executable builds pass.

The [Full HD record-layout comparison](results/m1-pro-metal-records-fullhd.json) retained every pixel in twelve renders, but packing both records was slower at the median despite saving 150,564,032 bytes of GPU scene buffers. A separate [direct executable comparison](results/m1-pro-metal-visibility-fullhd.json) measured 31.453 seconds for the saved release versus 32.222 seconds with cached visibility classification. All six renders completed with zero invalid samples, and all three EXR comparisons had zero error. These experiments remain disabled by default; the follow-up does not establish another production speedup.

The daylight exposure diagnostic completed at 1920 × 1080, 32 samples and 50 bounces, with zero invalid samples and `--exposure 1.5`. This changes the PNG/display transform, not the lighting, geometry or linear EXR. Its denoised image is a local development preview, not a converged final image or an isolated performance measurement. HDR environment importance sampling remains unimplemented.
