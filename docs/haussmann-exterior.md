# Haussmann exterior

The opposite building now has six occupied levels, continuous iron balconies on
the second and fifth floors, deep stone consoles, layered cornices, pilasters,
pediments and a mansard roof. Its slate lower slope contains recessed dormers;
the shallow zinc upper roof has raised seams, a ridge and chimney stacks with
separate terracotta pots.

The architectural reference is the [UNESCO Paris heritage atlas](https://whc.unesco.org/en/urban-heritage-atlas/Paris),
especially its residential building and roof sections. The roof proportions and
horizontal hierarchy are original scene geometry, not an imported survey of a
specific address.

Window and balcony guards use the existing [Juliet Balcony and Wrought Iron Balcony models](https://vinish.dev/free-3d-models/balconies-and-railings)
by vinish.dev, with source normals retained and no decimation. Meshes are loaded
once per scene assembly, then transformed into the scene. Their scale follows
the window openings, with wall plates and end returns. The source and conversion
licenses are recorded in [asset credits](../assets/CREDITS.md).

Neighbouring windows are grouped into apartments with deterministic variations:
wall and curtain palettes, curtain openings, shutters and roller blinds, four
furnishing themes, and different light positions and warm/cool light colors.
Some apartments remain unlit. The interior furnishings across the street are
purpose-built background geometry, not full furnished apartment assets.

Day and night use separate photographic 4K HDR skies from Poly Haven: a partly
cloudy daytime sky and an urban evening sky with thin clouds. Texture
values stay linear and can exceed one. CPU and Metal sample the same spherical
mapping, with pole clamping, longitude wrapping, exposure and horizon blending.
The sky affects reflection and indirect-light rays as well as the background.
The cloud appearance is photographed, not generated or ray-marched; the incense
volume remains independent. Environment importance sampling is not implemented,
so small bright sun/moon features can converge slowly.

The environment exporter uses the same longitude orientation as Cycles after
the scene's Y-up to Z-up coordinate conversion. CPU and native Metal tests check
HDR values, pole clamping, longitude direction, rotation and horizon blending.
The night-scene regression also casts rays beneath the opposite apartments'
ceiling emitters to catch lights accidentally enclosed in ceiling geometry.

The retained architectural fill lights are scene-tuned artistic lighting, not a
calibrated atmospheric or astronomical model. The sky map's exposure and horizon
fade are also artistic settings. No photographic-equivalence claim is made from
a development preview.
The former nearby rectangular daylight emitter was removed because it appeared
as a white panel when navigating outside. The photographic daytime environment
and the overhead architectural fill now supply the daylight.

```bash
cargo run --release --example render_exterior -- night 128
cargo run --release --example render_exterior -- day 128
cargo run --release --example render_exterior -- railing 128
```

Each produces a 1920 × 1080 PNG, raw image, linear EXR, statistics and saved
viewpoint in `renders/`. Load the exterior viewpoint in the live viewer with:

```bash
./target/release/viewer --backend metal --scene interior-neon \
  --viewpoint renders/haussmann-night-view.json --samples 2048 --bounces 50
```

Press `F` for free flight when inspecting the exterior and roof.

The saved viewpoints change only camera and exposure. They do not lower geometry
detail, texture resolution or the final-render sample and bounce settings. The
preview example uses 16 bounces, independent of the viewer command above.

Validation on Apple M1 Pro: all 57 release tests passed, including native Metal
tests, plus `cargo fmt --check` and Clippy with warnings denied. This covers the
environment mapping and the ceiling-emitter clearance regression. Image preview
timings are recorded for traceability, not as controlled performance benchmarks.
