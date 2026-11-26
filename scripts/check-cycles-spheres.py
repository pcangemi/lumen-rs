"""Run with Blender --background --python scripts/check-cycles-spheres.py."""
import json
import math
import sys
import time
from pathlib import Path

import bpy
from mathutils import Vector

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cycles_scene import material, sphere_cloud


def main():
    started = time.perf_counter()
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    scene = bpy.context.scene
    image = bpy.data.images.new('Linear wool fixture', width=2, height=2)
    image.colorspace_settings.name = 'Non-Color'
    image.pixels = [0.7, 0.5, 0.3, 1] * 4
    normal = bpy.data.images.new('Flat normal fixture', width=2, height=2)
    normal.colorspace_settings.name = 'Non-Color'
    normal.pixels = [0.5, 0.5, 1, 1] * 4
    record = {
        'base': [0.8, 0.8, 0.8, 0.3], 'surface': [0.98, 0, 0, 1.5],
        'emission': [0, 0, 0, 0], 'absorption': [0, 0, 0, 0],
        'detail': [1, 0, 1, 7], 'maps': [2, 1, 0, 0], 'world_projection': True,
    }
    surface = material(record, [image, normal], False, point_cloud=True)
    for vertex_colors in (False, True):
        for kind in (0, 1, 2):
            material({**record, 'surface': [0.7, 0, kind, 1.5]},
                     [image, normal], vertex_colors)
        material({**record, 'absorption': [0, 0, 0, 0.6]},
                 [image, normal], vertex_colors)
        material({**record, 'emission': [3, 2, 1, 0]},
                 [image, normal], vertex_colors)
    spheres = []
    for column in range(300):
        for row in range(194):
            for segment in range(8):
                theta = segment * math.tau / 8
                spheres.append({
                    'center': [column * 0.018 + 0.006 * math.cos(theta),
                               0.006 * (1 + math.sin(theta)), row * 0.018],
                    'radius': 0.004,
                })
    cloud = sphere_cloud(scene, spheres, surface)
    assert len(cloud.data.vertices) == len(spheres)
    assert len(bpy.data.objects) == 1
    assert all(abs(point.value - 0.004) < 1e-8
               for point in cloud.data.attributes['lumen_radius'].data)
    built = time.perf_counter()

    camera_data = bpy.data.cameras.new('Camera')
    camera = bpy.data.objects.new('Camera', camera_data)
    scene.collection.objects.link(camera)
    camera.location = (2.7, -1.6, 0.24)
    target = Vector((2.7, -1.8, 0.006))
    camera.rotation_euler = (target - camera.location).to_track_quat('-Z', 'Y').to_euler()
    camera_data.clip_start = 0.001
    scene.camera = camera
    scene.world = bpy.data.worlds.new('White environment')
    scene.world.use_nodes = True
    scene.world.node_tree.nodes['Background'].inputs[0].default_value = (1, 1, 1, 1)
    scene.render.film_transparent = True
    scene.render.engine = 'CYCLES'
    scene.cycles.device = 'CPU'
    scene.cycles.samples = 16
    scene.cycles.max_bounces = 4
    scene.render.resolution_x = 128
    scene.render.resolution_y = 96
    scene.render.resolution_percentage = 100
    folder = Path(__file__).resolve().parent.parent / 'renders'
    folder.mkdir(exist_ok=True)
    scene.render.filepath = str(folder / 'cycles-wool-smoke.exr')
    scene.render.image_settings.file_format = 'OPEN_EXR'
    scene.render.image_settings.color_mode = 'RGBA'
    bpy.ops.render.render(write_still=True)
    rendered = bpy.data.images.load(scene.render.filepath)
    pixels = list(rendered.pixels)
    assert all(math.isfinite(value) for value in pixels)
    coverage = sum(pixels[3::4]) / (128 * 96)
    assert coverage > 0.2, f'Point radii produced insufficient wool coverage: {coverage}'
    assert max(pixels[::4]) > 0.1, 'Point-cloud material rendered black'
    report = {
        'blender_version': bpy.app.version_string,
        'sphere_count': len(spheres), 'sphere_objects': 1,
        'build_seconds': built - started,
        'render_and_validation_seconds': time.perf_counter() - built,
        'coverage': coverage, 'finite': True,
        'scope': 'Synthetic dense-sphere Cycles CPU smoke; not a renderer quality comparison',
    }
    (folder / 'cycles-wool-smoke.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
