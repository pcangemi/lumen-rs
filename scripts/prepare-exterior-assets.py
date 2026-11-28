"""Convert licensed balcony meshes without decimation; run through Blender."""
import hashlib
import json
from pathlib import Path
import subprocess

import bpy

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / '.runtime/source-assets/haussmann'
OUTPUT = ROOT / 'assets/models/haussmann'
OUTPUT.mkdir(parents=True, exist_ok=True)
SOURCE.mkdir(parents=True, exist_ok=True)
records = []
for name in ['juliet-balcony', 'wrought-iron-balcony']:
    url = f'https://vinish-models.pages.dev/balconies-and-railings/{name}.glb'
    path = SOURCE / f'{name}.glb'
    if not path.exists():
        subprocess.run(['curl', '-fLsS', url, '-o', str(path)], check=True)
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    manifest = OUTPUT / 'sources.json'
    if manifest.exists():
        previous = next(x for x in json.loads(manifest.read_text()) if x['name'] == name)
        if digest != previous['sha256']:
            raise ValueError(f'Source checksum mismatch: {name}')
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(path))
    triangles = []
    for obj in bpy.context.scene.objects:
        if obj.type != 'MESH':
            continue
        mesh = obj.data
        mesh.calc_loop_triangles()
        normals = obj.matrix_world.to_3x3().inverted().transposed()
        for triangle in mesh.loop_triangles:
            mat = obj.material_slots[triangle.material_index].material.name
            if 'Iron' not in mat:
                continue
            corners = []
            for i in triangle.loops:
                p = obj.matrix_world @ mesh.vertices[mesh.loops[i].vertex_index].co
                n = (normals @ mesh.corner_normals[i].vector).normalized()
                corners.append((p.x, p.z, -p.y, n.x, n.z, -n.y))
            triangles.append(corners)
    # The long balcony contributes its detailed front panel to continuous runs.
    # Keep complete triangles, then ground and center the selected ironwork.
    if name == 'wrought-iron-balcony':
        front = max(p[2] for tri in triangles for p in tri)
        triangles = [tri for tri in triangles if min(p[2] for p in tri) > front - 0.10]
    lo = [min(p[k] for tri in triangles for p in tri) for k in range(3)]
    hi = [max(p[k] for tri in triangles for p in tri) for k in range(3)]
    origin = [(lo[0] + hi[0]) / 2, lo[1], lo[2]]
    vertices = {}
    with (OUTPUT / f'{name}.obj').open('w') as output:
        output.write('# Adapted CC BY 4.0 ironwork by vinish.dev. See NOTICE.md.\n')
        for triangle in triangles:
            face = []
            for p in triangle:
                key = tuple(round(p[k] - origin[k], 7) for k in range(3)) + tuple(round(x, 7) for x in p[3:])
                if key not in vertices:
                    vertices[key] = len(vertices) + 1
                    output.write('v ' + ' '.join(map(str, key[:3])) + '\n')
                    output.write('vn ' + ' '.join(map(str, key[3:])) + '\n')
                face.append(vertices[key])
            output.write('f ' + ' '.join(f'{i}//{i}' for i in face) + '\n')
    records.append(dict(name=name, url=url, source='https://vinish.dev/free-3d-models/balconies-and-railings',
                        author='vinish.dev', license='CC-BY-4.0', sha256=digest,
                        triangles=len(triangles), dimensions=[hi[k]-lo[k] for k in range(3)]))
print(json.dumps(records, indent=2))
(OUTPUT / 'sources.json').write_text(json.dumps(records, indent=2) + '\n')
