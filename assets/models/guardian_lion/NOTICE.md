# Chinese Guardian Lions

**Creator:** SolidXperts

**Source:** https://sketchfab.com/3d-models/chinese-guardian-lions-c981456846cc49f59cfa92553b7a7506

**License:** Creative Commons Attribution-NonCommercial 4.0 International
(CC BY-NC 4.0), https://creativecommons.org/licenses/by-nc/4.0/

This mesh is separate from Lumen's MIT-licensed code. Commercial use of this
asset requires separate permission from the rights holder. Attribution and
this license notice must accompany redistributed copies.

The source is an Artec Leo scan of a marble guardian lion at the Arah W.
Prescott Historical Library in Hooksett, New Hampshire. Its weathering belongs
to the scanned object. The archive was supplied by the user from Sketchfab.

## Modifications

- Retained the main connected sculpture; removed detached scan fragments.
- Leveled the pedestal using a robust fit to the scanned underside, then
  resurfaced one source millimeter above that plane to provide a flat support.
- Closed the remaining small scan openings locally and oriented the shell
  outward. Collapsed only micrometer-scale edges below the numerical tolerance.
- Converted the coordinate system to Y-up, centered the horizontal bounds,
  grounded the pedestal and normalized the total height to one unit.
- Replaced the source marble texture with clear solid glass in the scene.
  The sculpture and its pedestal together measure 1.40 m in the apartment.

The carved surface is not decimated or globally smoothed. Source and output
checksums, mesh counts and repair statistics are recorded in `conversion.json`.
The source textures are not redistributed.

To reproduce the conversion:

```bash
/Applications/Blender.app/Contents/MacOS/Blender --background \
  --python scripts/prepare-guardian-lion.py -- /path/to/chinese-guardian-lions.zip
```

The round wool rug is original project geometry and is not part of the scan.
