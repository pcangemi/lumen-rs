# Architecture

## Data flow

A built-in preset or validated JSON file produces geometry and materials. OBJ meshes are triangulated once at load time. BVH construction reorders primitives into contiguous leaf ranges; only then are emissive primitives indexed and their sampling probabilities normalized.

The renderer owns this immutable scene. Each camera sample has a deterministic random stream based on pixel index, sample index, and seed. Path state stays on the stack. The CPU backend distributes independent pixels through Rayon. The progressive backend schedules 64-pixel tiles in center-first passes. Metal consumes the same packed geometry, material, camera, light, and BVH records. Both Metal modes dispatch 64 × 64 tiles, starting at one sample and adapting later batches using completed GPU command timings. The slowest tile in each pass guides the next batch toward 20 ms, with at most twofold growth and a 16-sample ceiling. The requested batch size remains an upper limit. Offline rendering reads the film once at completion; the viewer reads completed regions. GPU regressions check exact pixel equality across these scheduling modes and different batch limits.

The native window owns a separate linear image. A worker sends completed tile copies through a channel bounded to four events. Pause and cancellation use atomics checked at tile boundaries. Changing exposure only changes the display transform. Restarting or switching scenes cancels the old worker, drops its receiver, and starts a new job; stale events cannot overwrite the new image.

## Modules

| Module | Responsibility |
| --- | --- |
| `geometry` | Rays, primitive intersections, normals, surface sampling, origin offsets |
| `bvh` | SAH construction, flat nodes, closest-hit and shadow traversal |
| `material` | Procedural colors, GGX, Lambert, dielectric Fresnel |
| `integrator` | Path throughput, light sampling, MIS, absorption, medium stack, Russian roulette |
| `camera` | Perspective projection and thin-lens sampling |
| `rng` | Reproducible random streams and directional sampling |
| `scene` | Scene assembly and OBJ/MTL import |
| `scene/file` | JSON schema and validation |
| `scene/presets`, `scene/apartment` | Studio, day/night apartment, courtyard and cloud demonstrations |
| `scene/apartment/atmosphere` | Sourced lighting, rattan weave, exterior, curtains and canvas |
| `scene/water` | Closed displaced meshes with time-dependent waves |
| `texture` | Linear atlas and bilinear texture sampling |
| `medium` | Delta tracking, ratio tracking, cloud density and phase sampling |
| `denoise` | Optional OIDN C API, surface guides and separate filtered output |
| `comparison`, `bin/compare` | Scene snapshots and linear image-error measurements |
| `render` | Offline CPU scheduling, settings, statistics |
| `progressive` | Tiled CPU rendering and shared job controls |
| `metal_backend` | Metal device, pipeline, shared buffers, dispatch, readback |
| `pathtrace.metal` | GPU transport and traversal |
| `output` | Exposure, display transform, PNG, OpenEXR, metadata |
| `viewer_ui` | Window-independent interface drawing |
| `bin/viewer` | Native window, controls, worker lifecycle, event handling |

## Conventions

- All identifiers, comments, diagnostics, UI labels, documentation, and commit messages are English.
- Use `snake_case` for functions and fields, `UpperCamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants.
- Run `cargo fmt` and keep `cargo clippy --all-targets -- -D warnings` clean.
- Prefer explicit names for application state. Conventional symbols such as `wi`, `wo`, `pdf`, and `eta` are appropriate within rendering equations and are documented at the API boundary.
- Use `Result` for loading, validation, device, and output failures. Do not silently replace a requested Metal render with a CPU render.
- Keep changes to CPU transport and Metal transport synchronized. Validate their statistics on an accessible GPU after changing either implementation.
- Do not add `unsafe` to geometry or transport code. Unsafe operations are confined to shared Metal buffers after command completion, documented Metal Objective-C selectors and the OIDN C ABI. Each has explicit ownership and safety comments.
- Keep GPU layouts explicit with `repr(C)` and `Pod`; test record sizes. Packed arrays exist for the GPU ABI, not as a general application coding style.
- Avoid allocating in the path-bounce loop. Allocate scene buffers once, and reuse image accumulators across passes.
- Add regression tests for numerical behavior, concurrency boundaries, malformed input, and layout contracts rather than tests that simply restate implementation details.

## Numerical scope

Distances are in meters. Rendering uses single-precision, linear RGB rather than a spectral representation. BRDF directions point away from the surface; ray directions are normalized. Geometric normals determine medium transitions and ray offsets; smooth normals determine opaque shading.

Light sampling uses area-weighted emitted luminance and the power heuristic. Smooth specular paths use delta events and cannot be sampled by direct-light connections through glass. Caustics may appear through path sampling, but can be very noisy. The bounce cap truncates transport. Adaptive stopping is optional and introduces additional estimator bias.

One optional bounded medium occupies air outside closed dielectrics. Homogeneous extinction has analytic shadow transmittance; procedural clouds use delta tracking for collisions and ratio tracking for shadows. The global majorant is conservative because density remains in [0, 1]. The implementation does not support overlapping participating media or spatial majorant grids.

The viewer runs OIDN on a separate worker with a bounded request queue. Albedo and normal guides are sampled with the rendering camera. Filtering operates on a separate image after complete passes, and completion waits for the final filtered image. Offline rendering filters after transport. Neither mode feeds filtered pixels back into accumulation. The raw image remains available for checking detail, noise and bias.

Image textures include base color, tangent-space normals, roughness, metalness and binary opacity. Rough transmission, spectral dispersion, motion blur, subsurface transport, native Metal acceleration structures and a dedicated caustic estimator remain unimplemented.

## Validation status

The standard suite checks BVH traversal against brute force, parallel slab boundaries, surface interpolation, dielectric Fresnel and total internal reflection, approximate BRDF energy and reciprocity, constant-radiance transmission through clear glass, outward box winding, OBJ rendering, thread determinism, GPU record sizes, progressive/offline equivalence, cancellation, texture filtering, MTL-relative texture paths, water boundary orientation, Beer–Lambert slab transmission, phase-function sampling and volume transmittance.

Native-window input requires a desktop session, and Metal execution requires an accessible GPU. The headless UI preview and shader compilation do not establish those runtime properties. The optional OIDN test checks noise reduction against a known constant field. The two ignored GPU tests cover all five presets, batch invariance and partial tiles on an accessible device.

A dependency of the Metal binding (`block` 0.1.6) emits an upstream future-compatibility warning on the current Rust toolchain; project code passes Clippy without warnings.
