"""Restore pinned coastal sources and export their full meshes with Blender.

Run: Blender --background --python-exit-code 1 --python scripts/prepare-coastal-assets.py
The renderer uses only the bundled OBJ/MTL files. Source downloads and licenses
are recorded in assets/models/coastal-sources.json; no geometry is decimated.
"""
import hashlib
import json
import math
import shutil
import struct
import urllib.request
from pathlib import Path

import bpy
import numpy as np
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / '.runtime/source-assets'
MANIFEST = ROOT / 'assets/models/coastal-sources.json'


def restore(record):
    folder = CACHE / record['name']
    for item in record['files']:
        dest = folder / item['path']
        if not dest.exists():
            request = urllib.request.Request(item['url'], headers={'User-Agent': 'Lumen asset preparation'})
            with urllib.request.urlopen(request, timeout=120) as response:
                contents = response.read()
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(contents)
        if hashlib.sha256(dest.read_bytes()).hexdigest() != item['sha256']:
            raise ValueError(f'Source checksum mismatch: {dest}')
    return folder


def unpack(folder, name):
    """Expose embedded GLB images without altering their pixels."""
    glb = folder / (name + '.glb')
    if not glb.exists():
        return folder / (name + '.gltf')
    blob = glb.read_bytes()
    size = struct.unpack_from('<I', blob, 12)[0]
    data = json.loads(blob[20:20 + size])
    binary = blob[28 + size:]
    (folder / 'mesh.bin').write_bytes(binary)
    data['buffers'][0]['uri'] = 'mesh.bin'
    for index, image in enumerate(data.get('images', [])):
        view = data['bufferViews'][image.pop('bufferView')]
        extension = {'image/png': '.png', 'image/jpeg': '.jpg'}[image.pop('mimeType')]
        dest = folder / f'image-{index}{extension}'
        start = view.get('byteOffset', 0)
        dest.write_bytes(binary[start:start + view['byteLength']])
        image['uri'] = dest.name
    dest = folder / (name + '.gltf')
    dest.write_text(json.dumps(data))
    return dest


def convert(record):
    name = record['name']
    folder = restore(record)
    if record.get('kind') == 'texture':
        output = ROOT / 'assets/textures/coastal' / name
        output.mkdir(parents=True, exist_ok=True)
        for item in record['files']:
            shutil.copyfile(folder / item['path'], output / item['path'])
        return
    source = unpack(folder, name)
    data = json.loads(source.read_text())
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(source))
    output = ROOT / 'assets/models' / name
    output.mkdir(parents=True, exist_ok=True)
    texture_cache = {}

    def texture(index, channel=None):
        key = (index, channel)
        if key in texture_cache:
            return texture_cache[key]
        source_image = folder / data['images'][data['textures'][index]['source']]['uri']
        dest = output / (f'texture-{index}' + (source_image.suffix if channel is None else f'-{channel}.png'))
        if channel is None:
            shutil.copyfile(source_image, dest)
        else:
            image = bpy.data.images.load(str(source_image), check_existing=False)
            image.colorspace_settings.name = 'Non-Color'
            pixels = np.empty(len(image.pixels), dtype=np.float32)
            image.pixels.foreach_get(pixels)
            pixels = pixels.reshape((-1, 4))
            if np.ptp(pixels[:, channel]) == 0:
                value = float(pixels[0, channel])
                bpy.data.images.remove(image)
                texture_cache[key] = value
                return value
            pixels[:, :3] = pixels[:, channel:channel + 1].copy()
            pixels[:, 3] = 1
            image.pixels.foreach_set(pixels.ravel())
            image.file_format = 'PNG'
            image.filepath_raw = str(dest)
            image.save()
            bpy.data.images.remove(image)
        texture_cache[key] = dest.name
        return dest.name

    lines = []
    for index, material in enumerate(data['materials']):
        pbr = material.get('pbrMetallicRoughness', {})
        color = pbr.get('baseColorFactor', [1, 1, 1, 1])
        lines += [f'newmtl material_{index}', 'Kd ' + ' '.join(map(str, color[:3])),
                  f"Pr {pbr.get('roughnessFactor', 1)}", f"Pm {pbr.get('metallicFactor', 1)}"]
        extensions = material.get('extensions', {})
        if extensions.get('KHR_materials_transmission', {}).get('transmissionFactor', 0) > 0.5:
            lines += ['d 0', f"Ni {extensions.get('KHR_materials_ior', {}).get('ior', 1.5)}"]
        elif name != 'coastal_villa' or index not in [0, 1, 2, 5, 7, 9, 10]:
            if 'baseColorTexture' in pbr:
                lines.append('map_Kd ' + texture(pbr['baseColorTexture']['index']))
            if 'normalTexture' in material:
                lines.append('norm ' + texture(material['normalTexture']['index']))
            if 'metallicRoughnessTexture' in pbr:
                index_map = pbr['metallicRoughnessTexture']['index']
                for channel, key, factor in [(1, 'Pr', pbr.get('roughnessFactor', 1)), (2, 'Pm', pbr.get('metallicFactor', 1))]:
                    value = texture(index_map, channel)
                    if isinstance(value, float):
                        lines.append(f'{key} {value * factor}')
                    else:
                        lines.append(f'map_{key} {value}')
        if (folder / 'alpha.png').exists():
            shutil.copyfile(folder / 'alpha.png', output / 'alpha.png')
            lines.append('map_d alpha.png')
        lines.append('')
    (output / (name + '.mtl')).write_text('\n'.join(lines))
    used_textures = {line.split(' ', 1)[1] for line in lines if line.startswith(('map_', 'norm '))}
    for image in output.iterdir():
        if image.suffix.lower() in ['.png', '.jpg'] and image.name not in used_textures:
            image.unlink()
    objects = [obj for obj in bpy.context.scene.objects if obj.type == 'MESH']
    if name == 'coastal_villa':
        objects = [obj for obj in objects if not obj.name.startswith('Pool ') and obj.name != 'Timber deck']
    slots = {material.get('name', f'material_{i}'): i for i, material in enumerate(data['materials'])}
    # Millimeter edge radii supply real architectural highlights, retaining all
    # original openings, joinery, hardware and UVs from the downloaded building.
    if name == 'coastal_villa':
        for obj in objects:
            if any(any(word in slot.material.name.lower() for word in ['glazing', 'glass', 'water']) for slot in obj.material_slots if slot.material):
                continue
            bevel = obj.modifiers.new('Construction edge radius', 'BEVEL')
            bevel.width = 0.006
            bevel.segments = 3
            bevel.limit_method = 'ANGLE'
    if name == 'grass_bermuda_01':
        for variant in ['small_a', 'small_c', 'medium_a']:
            selection = [obj for obj in objects if variant in obj.name]
            if not selection:
                raise ValueError(f'Missing grass variant {variant}: {[obj.name for obj in objects]}')
            export(selection, data, slots, output, name, variant)
    else:
        export(objects, data, slots, output, name, name)


def export(objects, data, slots, output, library, name):
    points = [obj.matrix_world @ Vector(corner) for obj in objects for corner in obj.bound_box]
    lo = Vector(tuple(min(p[k] for p in points) for k in range(3)))
    hi = Vector(tuple(max(p[k] for p in points) for k in range(3)))
    center = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, 0 if library == 'coastal_villa' else lo.z))
    vertices = {}
    triangle_count = 0
    graph = bpy.context.evaluated_depsgraph_get()
    with (output / (name + '.obj')).open('w') as file:
        file.write(f'# Source and modifications: ../../CREDITS.md\nmtllib {library}.mtl\n')
        for obj in objects:
            evaluated = obj.evaluated_get(graph)
            mesh = evaluated.to_mesh()
            mesh.calc_loop_triangles()
            normal_matrix = obj.matrix_world.to_3x3().inverted().transposed()
            current = None
            for triangle in mesh.loop_triangles:
                material = obj.material_slots[triangle.material_index].material
                slot = slots.get(material.name, 0)
                if current != slot:
                    file.write(f'usemtl material_{slot}\n')
                    current = slot
                mapping = data['materials'][slot].get('pbrMetallicRoughness', {}).get('baseColorTexture', {}).get('extensions', {}).get('KHR_texture_transform', {})
                scale = mapping.get('scale', [1, 1])
                offset = mapping.get('offset', [0, 0])
                angle = mapping.get('rotation', 0)
                face = []
                for loop_index in triangle.loops:
                    loop = mesh.loops[loop_index]
                    p = obj.matrix_world @ mesh.vertices[loop.vertex_index].co - center
                    n = (normal_matrix @ mesh.corner_normals[loop_index].vector).normalized()
                    uv = mesh.uv_layers.active.data[loop_index].uv if mesh.uv_layers.active else (0, 0)
                    # glTF transforms use a top-left UV origin; OBJ uses bottom-left.
                    u, v = uv[0] * scale[0], (1 - uv[1]) * scale[1]
                    uv = (math.cos(angle) * u - math.sin(angle) * v + offset[0],
                          1 - (math.sin(angle) * u + math.cos(angle) * v + offset[1]))
                    key = tuple(round(x, 7) for x in (p.x, p.z, -p.y, n.x, n.z, -n.y, *uv))
                    index = vertices.get(key)
                    if index is None:
                        index = len(vertices) + 1
                        vertices[key] = index
                        file.write(f'v {key[0]} {key[1]} {key[2]}\nvn {key[3]} {key[4]} {key[5]}\nvt {key[6]} {key[7]}\n')
                    face.append(index)
                file.write('f ' + ' '.join(f'{i}/{i}/{i}' for i in face) + '\n')
                triangle_count += 1
            evaluated.to_mesh_clear()
    record = dict(triangles=triangle_count, dimensions=[hi.x-lo.x, hi.z-lo.z, hi.y-lo.y])
    (output / (name + '-geometry.json')).write_text(json.dumps(record, indent=2) + '\n')
    print(name, record, flush=True)


if __name__ == '__main__':
    for record in json.loads(MANIFEST.read_text()):
        convert(record)
