# Imported balcony ironwork

The source models **Juliet Balcony** and **Wrought Iron Balcony** are by
[vinish.dev](https://vinish.dev/free-3d-models/balconies-and-railings), licensed
[Creative Commons Attribution 4.0](https://creativecommons.org/licenses/by/4.0/).
Their source GLB files contain the same attribution in `asset.copyright`.

Modifications: converted to OBJ with original split normals, centered and grounded,
assigned the apartment's dark painted-iron material, and fitted to window openings.
The continuous balcony uses the front iron panel extracted from Wrought Iron
Balcony; its source slab and returns are replaced with scene-sized stonework and
end returns. No triangle decimation or subdivision was performed. These adaptations
remain subject to the source attribution requirements, separate from the engine's MIT license.

`sources.json` records download URLs, SHA-256 hashes, selected triangle counts and
output dimensions. Reproduce with:

```bash
/Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/prepare-exterior-assets.py
```
