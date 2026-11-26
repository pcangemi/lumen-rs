# Architecture

## Data flow

A built-in preset or validated JSON file produces geometry and materials. OBJ meshes are triangulated once at load time. BVH construction reorders primitives into contiguous leaf ranges; only then are emissive primitives indexed and their sampling probabilities normalized.

OBJ import uses the same minimum triangle area as scene validation so that scaling detailed glass models down does not silently remove valid faces. CPU and Metal triangle intersection use a relative determinant cancellation tolerance instead of a fixed world-area cutoff. The guardian-lion conversion retains the scanned carving, repairs its shell and levels its pedestal before import; it is a solid dielectric, not an alpha-blended or thin-glass surface.

The renderer owns this immutable scene. Each camera sample has a deterministic random stream based on pixel index, sample index, and seed. Path state stays on the stack. The CPU backend distributes independent pixels through Rayon. The progressive backend schedules 64-pixel tiles in center-first passes. Metal consumes the same packed geometry, material, camera, light, and BVH records. Both Metal modes dispatch 128 × 128 tiles, starting at one sample and adapting later batches using completed GPU command timings. The slowest tile in each pass guides the next batch toward 20 ms, with at most twofold growth and a 16-sample ceiling. The requested batch size remains an upper limit. Offline rendering reads the film once at completion; the viewer reads completed regions. GPU regressions check exact pixel equality across these scheduling modes and different batch limits. Metal defers normals and UVs until the nearest accepted intersection, except when opacity or thin-glass transmission requires them during traversal. Opaque shadow rays return after the geometric intersection. Fixed-sample rendering skips unused variance updates; radiance accumulation and random streams remain unchanged.

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
| `scene/presets`, `scene/apartment` | Studio, day/night/neon apartment, courtyard and cloud demonstrations |
| `scene/apartment/atmosphere` | Sourced lighting, rattan weave, curtains and canvas |
| `scene/apartment/exterior` | Recessed masonry facade, glazed rooms and public street lighting |
| `scene/apartment/sculpture` | Licensed glass guardian lion, physical scale and separate round wool rug |
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

The native viewer retains the scaled display image across frames. Render events invalidate it when visible pixels change; exposure, resize and raw/clean selection are part of the cache key. Controls and tile outlines are drawn every frame. The worker event queue is bounded to 64 entries to absorb bursts of completed tiles.

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

Sphere intersections use the distance to the ray's closest approach. This avoids cancellation between scene-scale squared distances when intersecting millimeter-sized wool beads. Both backends normalize the resulting surface normal. Regression rays check hits, silhouette misses and exits at distances up to 30 m.

The apartment's boucle yarn uses eight overlapping analytic beads per loop. Short, thick cords provide rounded relief with fewer primitives than the earlier thin polygonal strands. The backing and beads share the wool maps, with attenuated normal-map slopes. Geometry still uses the opaque surface model, not fiber scattering. Dense world-textured opaque sphere groups export to native Cycles point clouds, one object per material, through Blender's [Mesh to Points node](https://docs.blender.org/manual/en/4.0/modeling/geometry_nodes/mesh/operations/mesh_to_points.html). The importer reconstructs projected tangent frames for their normal maps; sparse and emitting spheres retain tessellated geometry. `scripts/check-cycles-spheres.py` exercises a 465,600-sphere cloud and material graphs in Blender without generating one object per bead.

Light sampling uses area-weighted emitted luminance and the power heuristic. Smooth specular paths use delta events and cannot be sampled by direct-light connections through solid refractive glass. Caustics may appear through path sampling, but can be very noisy. The bounce cap truncates transport. Adaptive stopping is optional and introduces additional estimator bias.

Thin glass is a separate model for locally parallel interfaces in air. It sums internal Fresnel reflections and transmits in the incident direction, approximating a thin envelope without tracing its wall thickness. Shadow traversal accumulates these transmission factors while retaining early-out occlusion for opaque objects. Straight transmission preserves the preceding sampling vertex and updates its probability for MIS; reflected paths remain specular. This exception does not make direct-light connections through solid refractive glass valid. Thin fabric/paper instead mixes the opaque BRDF with a cosine-weighted diffuse transmission lobe; neither kind requires a new GPU record layout.

The envelope model follows the [thin dielectric formulation in PBRT](https://pbr-book.org/4ed/Reflection_Models/Dielectric_BSDF#ThinDielectricBSDF). Regression tests compare accumulated shadow losses with the analytic result and next-event estimation with pure path sampling.

One optional bounded medium occupies air outside closed dielectrics. Homogeneous extinction has analytic shadow transmittance; procedural clouds use delta tracking for collisions and ratio tracking for shadows. The global majorant is conservative because density remains in [0, 1]. The implementation does not support overlapping participating media or spatial majorant grids.

The viewer runs OIDN on a separate worker with a bounded request queue. Albedo and normal guides are sampled with the rendering camera. Filtering operates on a separate image after complete passes, and completion waits for the final filtered image. Offline rendering filters after transport. Neither mode feeds filtered pixels back into accumulation. The raw image remains available for checking detail, noise and bias.

Image textures include base color, tangent-space normals, roughness, metalness and binary opacity. Rough transmission, spectral dispersion, motion blur, subsurface transport, native Metal acceleration structures and a dedicated caustic estimator remain unimplemented.

## Validation status

The standard suite checks BVH traversal against brute force, parallel slab boundaries, surface interpolation, dielectric Fresnel and total internal reflection, approximate BRDF energy and reciprocity, constant-radiance transmission through clear glass, outward box winding, OBJ rendering, thread determinism, GPU record sizes, progressive/offline equivalence, cancellation, texture filtering, MTL-relative texture paths, water boundary orientation, Beer–Lambert slab transmission, phase-function sampling and volume transmittance.

Native-window input requires a desktop session, and Metal execution requires an accessible GPU. The headless UI preview and shader compilation do not establish those runtime properties. The optional OIDN test checks noise reduction against a known constant field. The three ignored GPU tests cover all six presets, batch invariance, partial tiles and adaptive stopping on an accessible device.

A dependency of the Metal binding (`block` 0.1.6) emits an upstream future-compatibility warning on the current Rust toolchain; project code passes Clippy without warnings.
