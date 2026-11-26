# Development priorities

The target is photorealistic final images against Blender Cycles on Apple M1 Pro. Water scope is optical: waves, refraction, absorption and caustics. Fluid simulation is outside that scope.

The current renderer has a working CPU/Metal transport baseline, a progressive viewer, texture imports, guided denoising, architectural geometry, displaced water and bounded volume scattering. These features do not establish production parity or market-leading performance.

## Decisions to challenge

| Priority | Question | Next experiment | Acceptance criterion |
| --- | --- | --- | --- |
| 1 | Which image defects matter most? | Maintain raw and denoised references for the interior, glass product, water caustics and clouds. | Agreed local detail and noise thresholds, not only an attractive thumbnail. |
| 2 | Can it compete at equal final-image quality? | Run error-versus-time curves on M1 Pro against Cycles, including setup and denoising. | Repeated results at the same error threshold, with shader differences reconciled. |
| 3 | Are water and glass caustics central? | Prototype a dedicated estimator: specular manifold sampling or vertex connection/merging, chosen after representative path analysis. | Stable difficult light paths without clamping away their energy; compare bias and memory cost. |
| 4 | Do production assets determine realism? | Extend the existing normal/roughness/metalness and tangent-frame support with runtime glTF, mipmapping, texture caching and instancing. | Imported assets agree with reference renders across scale and UV seams. |
| 5 | Does indirect daylight dominate render time? | Add importance-sampled HDR lighting, then measure path guiding on interiors. | Better convergence at equal error with bounded preprocessing overhead. |
| 6 | Where does M1 Pro spend GPU time? | Profile traversal, occupancy, divergence and dispatch. Compare wider BVHs, native Metal intersectors and wavefront queues. | Measured improvement across the benchmark suite without quality regressions. |
| 7 | Are clouds a first-class workload? | Spatial majorant grids, density-grid/VDB import and better phase models. | Lower null-collision overhead and agreement with a volume reference. |
| 8 | Is realism limited by materials or lighting? | Add rough dielectric transmission, multiple-scattering GGX, layered fabric and measured material inputs. | Energy checks, reciprocity tests and side-by-side material studies. |
| 9 | Is an image pipeline part of the product? | Add OCIO color management, exposure metadata, resumable accumulation and camera controls. | Consistent output across viewer, EXR and reference applications. |

## Algorithm selection

There is no single best algorithm for every scene and device. ReSTIR methods address particular reuse and sampling problems; they are not a general replacement for physically correct transport or high-quality assets. Wavefront scheduling can reduce divergence but adds queues and memory traffic. A dedicated caustic estimator can improve difficult water lighting while increasing implementation and validation cost.

Use primary references when implementing these experiments: [PBRT transport](https://pbr-book.org/4ed/Light_Transport_I_Surface_Reflection/A_Better_Path_Tracer), [PBRT volume estimators](https://pbr-book.org/4ed/Light_Transport_II_Volume_Rendering/Volume_Scattering_Integrators), [ReSTIR GI](https://research.nvidia.com/publication/2021-06_restir-gi-path-resampling-real-time-path-tracing), and [Cycles light paths](https://docs.blender.org/manual/en/latest/render/cycles/render_settings/light_paths.html).

## Release gates

1. Preserve unclamped linear output and independently seeded references.
2. Validate transport before judging the denoised image.
3. Test CPU/Metal agreement after every transport, texture or layout change.
4. Check thin geometry, nested shells, absorption, grazing angles and malformed imports.
5. Inspect images at their delivery resolution. More samples cannot improve crude geometry or incorrect material scale.
6. Track peak memory and total time, not just sample throughput.
7. Publish measured limits alongside results; do not label a feature complete from shader compilation alone.
