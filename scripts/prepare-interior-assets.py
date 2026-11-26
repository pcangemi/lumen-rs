"""Convert downloaded licensed glTF assets to meters, Y-up OBJ/MTL with PBR maps.

Run with Blender --background --python scripts/prepare-interior-assets.py.
Sources and checksums live in assets/models/sources.json. Downloads are cached
under .runtime/source-assets; the renderer needs only the converted assets.
"""
import hashlib
import json
import shutil
from pathlib import Path

import bpy
import numpy as np
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
NAMES = ['modern_arm_chair_01', 'modern_coffee_table_01', 'modern_wooden_cabinet', 'tea_set_01', 'potted_plant_02', 'glam_velvet_sofa']


def convert(name):
    source = ROOT / '.runtime/source-assets' / name
    output = ROOT / 'assets/models' / name
    output.mkdir(parents=True, exist_ok=True)
    data = json.loads((source / (name + '.gltf')).read_text())
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(source / (name + '.gltf')))
    materials = []
    texture_cache = {}
    used_materials = {primitive.get("material", 0) for mesh in data["meshes"] for primitive in mesh["primitives"]}

    def texture(index, label, channel=None):
        if (index, channel) in texture_cache:
            return texture_cache[(index, channel)]
        image_index = data['textures'][index]['source']
        path = source / data['images'][image_index]['uri']
        destination = output / (label + ('.png' if channel is not None else path.suffix))
        if channel is None:
            shutil.copyfile(path, destination)
        else:
            image = bpy.data.images.load(str(path), check_existing=True)
            image.colorspace_settings.name = 'Non-Color'
            pixels = np.empty(len(image.pixels), dtype=np.float32)
            image.pixels.foreach_get(pixels)
            pixels = pixels.reshape((-1, 4))
            values = pixels[:, channel].copy()
            pixels[:, :3] = values[:, None]
            pixels[:, 3] = 1
            result = bpy.data.images.new(label, width=image.size[0], height=image.size[1])
            result.colorspace_settings.name = 'Non-Color'
            result.pixels.foreach_set(pixels.ravel())
            result.filepath_raw = str(destination)
            result.file_format = 'PNG'
            result.save()
            bpy.data.images.remove(result)
        texture_cache[(index, channel)] = destination.name
        return destination.name

    for index, material in enumerate(data.get('materials', [])):
        if index not in used_materials:
            continue
        pbr = material.get('pbrMetallicRoughness', {})
        color = pbr.get('baseColorFactor', [1, 1, 1, 1])
        # The source velvet relies on an unsupported sheen lobe; use a documented
        # cream woven-fabric adaptation, retaining the real sofa geometry and UVs.
        if name == 'glam_velvet_sofa' and 'fabric' in material.get('name', ''):
            color = [0.67, 0.60, 0.48, 1]

        lines = [f'newmtl material_{index}', 'Kd ' + ' '.join(map(str, color[:3])),
                 f"Pr {pbr.get('roughnessFactor', 1)}", f"Pm {pbr.get('metallicFactor', 1)}"]
        if 'baseColorTexture' in pbr:
            lines.append('map_Kd ' + texture(pbr['baseColorTexture']['index'], f'{index}-color'))
        if 'normalTexture' in material:
            lines.append('norm ' + texture(material['normalTexture']['index'], f'{index}-normal'))
        if 'metallicRoughnessTexture' in pbr:
            tex = pbr['metallicRoughnessTexture']['index']
            lines.append('map_Pr ' + texture(tex, f'{index}-roughness', 1))
            lines.append('map_Pm ' + texture(tex, f'{index}-metallic', 2))
        if name == 'potted_plant_02' and 'leaves' in material.get('name', ''):
            shutil.copyfile(source / 'textures/leaves-alpha.png', output / 'leaves-alpha.png')
            lines.append('map_d leaves-alpha.png')
        materials.extend(lines + [''])
    (output / (name + '.mtl')).write_text('\n'.join(materials))
    objects = [obj for obj in bpy.context.scene.objects if obj.type == 'MESH']
    points = [obj.matrix_world @ vertex.co for obj in objects for vertex in obj.data.vertices]
    lo = Vector(tuple(min(p[k] for p in points) for k in range(3)))
    hi = Vector(tuple(max(p[k] for p in points) for k in range(3)))
    center = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z))
    slots = {mat.get('name', f'material_{i}'): i for i, mat in enumerate(data['materials'])}
    vertex_index = 1
    vertices = {}
    triangle_count = 0
    with (output / (name + '.obj')).open('w') as file:
        file.write(f'# Licensed source asset; see ../sources.json and ../../CREDITS.md\nmtllib {name}.mtl\n')
        for obj in objects:
            mesh = obj.data
            mesh.calc_loop_triangles()
            normal_matrix = obj.matrix_world.to_3x3().inverted().transposed()
            current = None
            for triangle in mesh.loop_triangles:
                material = obj.material_slots[triangle.material_index].material
                slot = slots.get(material.name, 0)
                if slot != current:
                    file.write(f'usemtl material_{slot}\n')
                    current = slot
                face = []
                for loop_index in triangle.loops:
                    loop = mesh.loops[loop_index]
                    p = obj.matrix_world @ mesh.vertices[loop.vertex_index].co - center
                    n = (normal_matrix @ mesh.corner_normals[loop_index].vector).normalized()
                    uv = mesh.uv_layers.active.data[loop_index].uv if mesh.uv_layers.active else (0, 0)
                    if name == 'glam_velvet_sofa':
                        mapping = data['materials'][slot].get('normalTexture', {}).get('extensions', {}).get('KHR_texture_transform', {})
                        repeat = mapping.get('scale', [1, 1])
                        uv = (uv[0] * repeat[0], uv[1] * repeat[1])
                    key = tuple(round(v, 7) for v in (p.x, p.z, -p.y, n.x, n.z, -n.y, uv[0], uv[1]))
                    index = vertices.get(key)
                    if index is None:
                        index = vertex_index
                        vertex_index += 1
                        vertices[key] = index
                        file.write(f'v {key[0]} {key[1]} {key[2]}\n')
                        file.write(f'vn {key[3]} {key[4]} {key[5]}\n')
                        file.write(f'vt {key[6]} {key[7]}\n')
                    face.append(index)
                file.write('f ' + ' '.join(f'{v}/{v}/{v}' for v in face) + '\n')
                triangle_count += 1
    print(name, 'triangles', triangle_count, 'dimensions XYZ', list(hi - lo), flush=True)


if __name__ == '__main__':
    manifest = json.loads((ROOT / '.runtime/source-assets/manifest.json').read_text())
    for item in manifest:
        path = ROOT / item['file']
        if hashlib.md5(path.read_bytes()).hexdigest() != item['md5']:
            raise ValueError(f'Source checksum mismatch: {path}')
    for name in NAMES:
        convert(name)
    records = []
    for name in NAMES:
        sofa = name == 'glam_velvet_sofa'
        records.append(dict(asset=name,
            source='https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/GlamVelvetSofa' if sofa else f'https://polyhaven.com/a/{name}',
            license='CC-BY-4.0' if sofa else 'CC0-1.0',
            resolution='1K',
            files=[dict(path=str(Path(i['file']).relative_to(Path('.runtime/source-assets') / name)), url=i['url'], md5=i['md5']) for i in manifest if i['asset'] == name]))
    (ROOT / 'assets/models/sources.json').write_text(json.dumps(records, indent=2) + '\n')
