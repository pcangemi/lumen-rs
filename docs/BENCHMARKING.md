# Comparing final-image quality on M1 Pro

The chosen target is clean final images against Blender Cycles on Apple M1 Pro, not interactive frame rate. A useful result measures time to an agreed error level and retains the images used to calculate it.

## Reproducible runs

Build release binaries, close competing render jobs, connect power and use the same power mode. Run `scripts/compare-cycles.py` from a normal Terminal session so both engines can enumerate Metal. The script fails rather than silently substituting CPU for a requested GPU.

For a first validation, use `--quick`: 320 × 213, 32 candidate samples, 256 reference samples and one repeat. The first Cycles Metal run may still spend several minutes compiling shaders. Each subprocess reports its stage, log path and elapsed time every 30 seconds.

The default full run uses 640 × 426, 128 candidate samples, 2048 reference samples, 12 bounces and three measured candidate runs. Reference seed 1701 is independent of candidate seeds. Each engine is compared against its own reference because the shader models are not identical. Increase reference samples until independent reference images agree substantially better than the candidate error target. A high sample count alone does not establish a ground truth.

```bash
python3 scripts/compare-cycles.py --backend metal --scene interior \
  --width 1920 --height 1080 --samples 256 --reference-samples 4096 --repeats 3 \
  --output renders/comparison-interior-256
```

Repeat at several candidate sample counts to produce an error-versus-time curve. Rank results only after selecting a common error threshold and checking materials and difficult image regions. Record raw convergence first, then compare denoised final images separately, including denoising time. Inspect parquet detail, edges, glass, water and caustics; a low average error can conceal a local failure.

## Timing scopes

`process_wall_seconds` includes process startup, scene assembly, BVH construction, tracing and image output. Export preparation is excluded. It is measured the same way for both executables, although Lumen also writes a PNG. Record that extra output cost when interpreting short runs.

Lumen's `render_seconds` measures transport after scene and pipeline construction. Cycles' `render_wall_seconds` covers the render invocation, including renderer setup and EXR output. These internal fields have different scopes: do not divide them to claim a speedup. On tiny validation images startup dominates process time.

## Mapping limitations

- Geometry is exported after procedural assembly, with triangle normals, UVs and material indices. Analytic Lumen spheres are tessellated in Cycles.
- Cycles Principled BSDF approximates Lumen's Lambert plus single-scattering GGX mixture. Multiple scattering, diffuse response and importance sampling differ.
- UV textures are exported as linear EXR and sampled in both engines. World-projected textures and procedural colors are baked at triangle vertices in this comparison export; interpolation can visibly change large faces.
- Camera orientation, vertical field of view, focus distance and aperture are mapped. Environment radiance is exported as a linear equirectangular map.
- Smooth glass and RGB absorption are mapped to Cycles glass and absorption nodes. Surface smoothing and volume bookkeeping can differ.
- All triangle materials are separated into objects so a glass volume cannot make opaque geometry part of its absorption object.
- Participating media are currently rejected by the exporter. Do not claim cloud parity from a surface-only benchmark.
- Adaptive sampling, sample clamping and denoising are disabled for the raw comparison. Different termination rules and bounce semantics can still affect reference images.

The snapshot records these limitations. An automated report is evidence of a reproducible run, not evidence of equal appearance or production readiness.

## Inspect images

```bash
./target/release/compare renders/cycles-comparison/lumen-reference.exr \
  renders/cycles-comparison/lumen-1.exr --output renders/error.json
./target/release/compare renders/cycles-comparison/cycles-reference.exr \
  renders/cycles-comparison/cycles-1.exr --preview renders/cycles-preview.png
```

Metrics operate on linear pixels without fitting exposure. The preview applies Lumen's display curve, making visual checks consistent. Use EXR inputs; display PNG error is not radiometric error.

Blender's [light-path documentation](https://docs.blender.org/manual/en/latest/render/cycles/render_settings/light_paths.html) describes why ordinary path tracing can struggle with caustics. This is one of the cases to test explicitly before choosing a specialized integrator.

## Recorded baseline

The archived [M1 Pro interior run](VALIDATION.md), using the earlier 55,438-primitive scene, completed with three measured candidates per engine. Cycles had both lower total time and lower raw error at that single setting. The report is retained to guide optimization, not as an equal-quality ranking.

The current interior uses different geometry, materials and framing, including imported furniture, masked foliage and a displaced rug. The archived comparison does not describe its performance. Rebuild and regenerate both snapshots and references before measuring the updated scene.
