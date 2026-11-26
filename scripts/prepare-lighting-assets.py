"""Restore licensed lamp sources and convert them with Blender and Khronos ktx.

Run with Blender --background --python scripts/prepare-lighting-assets.py.
Set KTX_TOOL to the ktx executable when restoring compressed source textures.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess

import bpy
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
SOURCES = [
    ('rattan_pendant', 'https://vinish-models.pages.dev/ceiling-lights-and-chandeliers/rattan-pendant-light.glb', 'CC-BY-4.0', 'https://vinish.dev/free-3d-models/ceiling-lights-and-chandeliers'),
    ('arc_floor_lamp', 'https://tevy.dev/models/floor-lamp/v1/lod0.glb', 'CC0-1.0', 'https://tevy.dev/models/floor-lamp'),
]


def unpack(name, url):
    folder = ROOT / '.runtime/source-assets' / name
    folder.mkdir(parents=True, exist_ok=True)
    source = folder / (name + '.glb')
    if not source.exists():
        subprocess.run(['curl', '--fail', '--location', '--silent', '--show-error', url, '--output', str(source)], check=True)
    blob = source.read_bytes()
    manifest = ROOT / 'assets/models/lighting-sources.json'
    if manifest.exists():
        expected = next((item['sha256'] for item in json.loads(manifest.read_text()) if item['name'] == name), None)
        if expected and hashlib.sha256(blob).hexdigest() != expected:
            raise ValueError(f'Source checksum mismatch: {name}')
    json_size = struct.unpack_from('<I', blob, 12)[0]
    data = json.loads(blob[20:20 + json_size])
    binary = blob[28 + json_size:]
    (folder / 'mesh.bin').write_bytes(binary)
    data['buffers'][0]['uri'] = 'mesh.bin'
    for index, image in enumerate(data.get('images', [])):
        view = data['bufferViews'][image['bufferView']]
        start = view.get('byteOffset', 0)
        extension = '.ktx2' if image['mimeType'] == 'image/ktx2' else '.png'
        path = folder / f'image-{index}{extension}'
        path.write_bytes(binary[start:start + view['byteLength']])
        if extension == '.ktx2':
            target = path.with_suffix('.png')
            if not target.exists():
                subprocess.run([os.environ.get('KTX_TOOL', 'ktx'), 'extract', '--transcode', 'rgba8', str(path), str(target)], check=True)
            path = target
        image.clear()
        image['uri'] = path.name
    for texture in data.get('textures', []):
        basis = texture.get('extensions', {}).pop('KHR_texture_basisu', None)
        if basis:
            texture['source'] = basis['source']
    for key in ('extensionsUsed', 'extensionsRequired'):
        if key in data:
            data[key] = [value for value in data[key] if value != 'KHR_texture_basisu']
    (folder / (name + '.gltf')).write_text(json.dumps(data))
    return hashlib.sha256(blob).hexdigest()


def main():
    spec = importlib.util.spec_from_file_location('interior_assets', ROOT / 'scripts/prepare-interior-assets.py')
    converter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(converter)
    records = []
    for name, url, license_name, page in SOURCES:
        digest = unpack(name, url)
        converter.convert(name)
        output = ROOT / 'assets/models' / name
        if name == 'rattan_pendant':
            # The source RGBA weave provides genuine holes to camera and shadow rays.
            image = bpy.data.images.load(str(output / '4-color.png'), check_existing=False)
            image.colorspace_settings.name = 'Non-Color'
            pixels = np.empty(len(image.pixels), dtype=np.float32)
            image.pixels.foreach_get(pixels)
            pixels = pixels.reshape((-1, 4))
            pixels[:, :3] = pixels[:, 3:4]
            pixels[:, 3] = 1
            image.pixels.foreach_set(pixels.ravel())
            image.filepath_raw = str(output / 'weave-alpha.png')
            image.file_format = 'PNG'
            image.save()
            mtl = output / (name + '.mtl')
            mtl.write_text(mtl.read_text() + 'map_d weave-alpha.png\n')
        records.append(dict(name=name, source=page, url=url, license=license_name, sha256=digest))
    (ROOT / 'assets/models/lighting-sources.json').write_text(json.dumps(records, indent=2) + '\n')


if __name__ == '__main__':
    main()
