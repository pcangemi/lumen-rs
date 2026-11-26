"""Load a Lumen snapshot in Blender and render with explicit Cycles settings."""
import argparse
import json
import math
import sys
import time
from pathlib import Path

import bpy
from mathutils import Matrix, Vector


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--snapshot', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--backend', choices=['cpu', 'metal'], default='metal')
    parser.add_argument('--samples', type=int)
    parser.add_argument('--seed', type=int)
    parser.add_argument('--denoise', action='store_true')
    parser.add_argument('--save-blend', action='store_true')
    return parser.parse_args(sys.argv[sys.argv.index('--') + 1:])


def vector(value):
    """Map Lumen's Y-up coordinates to Blender's Z-up coordinates, preserving handedness."""
    return Vector((value[0], -value[2], value[1]))


def material(record, textures, vertex_colors):
    result = bpy.data.materials.new('Surface')
    result.use_nodes = True
    nodes, links = result.node_tree.nodes, result.node_tree.links
    nodes.clear()
    output = nodes.new('ShaderNodeOutputMaterial')
    base, surface = record['base'], record['surface']
    emission, absorption, detail = record['emission'], record['absorption'], record['detail']
    if max(emission[:3]) > 0:
        shader = nodes.new('ShaderNodeEmission')
        shader.inputs['Color'].default_value = (*emission[:3], 1)
        links.new(shader.outputs[0], output.inputs['Surface'])
        return result
    if surface[2] > 0.5:
        shader = nodes.new('ShaderNodeBsdfGlass')
        shader.distribution = 'BECKMANN'
        shader.inputs['Color'].default_value = (1, 1, 1, 1)
        shader.inputs['Roughness'].default_value = 0
        shader.inputs['IOR'].default_value = surface[3]
        if max(absorption[:3]) > 0:
            volume = nodes.new('ShaderNodeVolumeAbsorption')
            density = max(absorption[:3])
            volume.inputs['Density'].default_value = density
            volume.inputs['Color'].default_value = (*(1 - x / density for x in absorption[:3]), 1)
            links.new(volume.outputs[0], output.inputs['Volume'])
    else:
        shader = nodes.new('ShaderNodeBsdfPrincipled')
        shader.inputs['Base Color'].default_value = (*base[:3], 1)
        shader.inputs['Metallic'].default_value = surface[1]
        shader.inputs['Roughness'].default_value = surface[0]
        shader.inputs['IOR'].default_value = 1.5
        if vertex_colors:
            color = nodes.new('ShaderNodeVertexColor')
            color.layer_name = 'lumen_color'
            color_output = color.outputs['Color']
            if detail[2] >= 1 and detail[3] >= 0:
                uv = nodes.new('ShaderNodeTexCoord')
                scale = nodes.new('ShaderNodeVectorMath')
                scale.operation = 'SCALE'
                scale.inputs['Scale'].default_value = detail[3]
                links.new(uv.outputs['UV'], scale.inputs[0])
                texture = nodes.new('ShaderNodeTexImage')
                texture.image = textures[int(detail[2]) - 1]
                texture.interpolation = 'Linear'
                texture.extension = 'REPEAT'
                links.new(scale.outputs[0], texture.inputs['Vector'])
                strength = nodes.new('ShaderNodeMixRGB')
                strength.blend_type = 'MIX'
                strength.inputs[0].default_value = base[3]
                strength.inputs[1].default_value = (1, 1, 1, 1)
                links.new(texture.outputs['Color'], strength.inputs[2])
                multiply = nodes.new('ShaderNodeMixRGB')
                multiply.blend_type = 'MULTIPLY'
                multiply.inputs[0].default_value = 1
                links.new(color_output, multiply.inputs[1])
                links.new(strength.outputs[0], multiply.inputs[2])
                color_output = multiply.outputs[0]
            links.new(color_output, shader.inputs['Base Color'])
    links.new(shader.outputs[0], output.inputs['Surface'])
    return result


def build(data, folder):
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    scene = bpy.context.scene
    textures = []
    for filename in data['textures']:
        image = bpy.data.images.load(str(folder / filename))
        image.colorspace_settings.name = 'Non-Color'
        textures.append(image)
    materials = [material(record, textures, True) for record in data['materials']]
    groups = {}
    for triangle in data['triangles']:
        groups.setdefault(triangle['material'], []).append(triangle)
    # Separate material volumes: opaque walls must not share an absorption object with glass.
    for material_index, triangles in groups.items():
        positions = [vector(p) for triangle in triangles for p in triangle['positions']]
        faces = [(i, i + 1, i + 2) for i in range(0, len(positions), 3)]
        mesh = bpy.data.meshes.new('Assembled geometry')
        mesh.from_pydata(positions, [], faces)
        mesh.update()
        obj = bpy.data.objects.new('Assembled geometry', mesh)
        scene.collection.objects.link(obj)
        mesh.materials.append(materials[material_index])
        uv_layer = mesh.uv_layers.new(name='UVMap')
        color_layer = mesh.color_attributes.new(name='lumen_color', type='FLOAT_COLOR', domain='CORNER')
        normals = []
        for face, triangle in zip(mesh.polygons, triangles):
            face.material_index = 0
            glass = data['materials'][triangle['material']]['surface'][2] > 0.5
            face.use_smooth = not glass
            for corner, loop in enumerate(face.loop_indices):
                uv_layer.data[loop].uv = triangle['uvs'][corner]
                color_layer.data[loop].color = (*triangle['colors'][corner], 1)
                normals.append(vector(triangle['normals'][corner]))
        mesh.normals_split_custom_set(normals)
    for sphere in data['spheres']:
        bpy.ops.mesh.primitive_uv_sphere_add(segments=128, ring_count=64,
            radius=sphere['radius'], location=vector(sphere['center']))
        obj = bpy.context.object
        obj.name = 'Analytic sphere approximation'
        obj.data.materials.append(material(data['materials'][sphere['material']], textures, False))
        for face in obj.data.polygons:
            face.use_smooth = True
    camera = data['camera']
    camera_data = bpy.data.cameras.new('Camera')
    camera_obj = bpy.data.objects.new('Camera', camera_data)
    scene.collection.objects.link(camera_obj)
    camera_obj.location = vector(camera['origin'])
    camera_obj.rotation_euler = Matrix((vector(camera['right']), vector(camera['up']),
                                       -vector(camera['forward']))).transposed().to_euler()
    camera_data.sensor_fit = 'VERTICAL'
    camera_data.sensor_height = 24
    camera_data.lens = 12 / camera['up'][3]
    camera_data.clip_start = 0.001
    camera_data.clip_end = 10000
    if camera['origin'][3] > 0:
        camera_data.dof.use_dof = True
        camera_data.dof.focus_distance = camera['forward'][3]
        camera_data.dof.aperture_fstop = camera_data.lens / (2000 * camera['origin'][3])
    scene.camera = camera_obj
    world = bpy.data.worlds.new('Linear environment')
    scene.world = world
    world.use_nodes = True
    environment = world.node_tree.nodes.new('ShaderNodeTexEnvironment')
    environment.image = bpy.data.images.load(str(folder / 'environment.exr'))
    environment.image.colorspace_settings.name = 'Non-Color'
    world.node_tree.links.new(environment.outputs['Color'], world.node_tree.nodes['Background'].inputs['Color'])
    scene.render.resolution_x, scene.render.resolution_y = data['width'], data['height']
    scene.render.resolution_percentage = 100
    scene.render.engine = 'CYCLES'
    scene.cycles.use_adaptive_sampling = False
    scene.cycles.max_bounces = data['max_depth']
    scene.cycles.diffuse_bounces = data['max_depth']
    scene.cycles.glossy_bounces = data['max_depth']
    scene.cycles.transmission_bounces = data['max_depth']
    scene.cycles.volume_bounces = data['max_depth']
    scene.cycles.sample_clamp_direct = 0
    scene.cycles.sample_clamp_indirect = 0
    scene.cycles.blur_glossy = 0
    scene.cycles.caustics_reflective = True
    scene.cycles.caustics_refractive = True
    scene.render.image_settings.file_format = 'OPEN_EXR'
    scene.render.image_settings.color_depth = '32'
    scene.render.image_settings.color_mode = 'RGB'
    return scene


def main():
    args = arguments()
    started = time.perf_counter()
    data = json.loads(args.snapshot.read_text())
    if data['schema'] != 1:
        raise ValueError('Unsupported snapshot schema')
    print("Building exported scene", flush=True)
    scene = build(data, args.snapshot.parent)
    scene.cycles.samples = args.samples or data['samples']
    scene.cycles.seed = args.seed if args.seed is not None else data['seed']
    scene.cycles.use_denoising = args.denoise
    devices = []
    if args.backend == 'metal':
        preferences = bpy.context.preferences.addons['cycles'].preferences
        preferences.compute_device_type = 'METAL'
        preferences.refresh_devices()
        for device in preferences.devices:
            device.use = device.type == 'METAL'
            if device.use:
                devices.append(device.name)
        if not devices:
            raise RuntimeError('No Cycles Metal device accessible; launch this command from a normal Terminal session')
        scene.cycles.device = 'GPU'
    else:
        scene.cycles.device = 'CPU'
        devices = ['CPU']
    args.output.parent.mkdir(parents=True, exist_ok=True)
    scene.render.filepath = str(args.output.resolve())
    if args.save_blend:
        bpy.ops.wm.save_as_mainfile(filepath=str(args.output.with_suffix('.blend').resolve()))
    setup_seconds = time.perf_counter() - started
    started = time.perf_counter()
    print(f"Starting Cycles on {devices}: {scene.cycles.samples} samples; first-run Metal shader compilation may take several minutes", flush=True)
    bpy.ops.render.render(write_still=True)
    report = {
        'engine': 'Cycles', 'blender_version': bpy.app.version_string, 'backend': args.backend,
        'devices': devices, 'samples': scene.cycles.samples, 'seed': scene.cycles.seed,
        'max_depth': scene.cycles.max_bounces, 'denoised': args.denoise,
        'scene_setup_seconds': setup_seconds, 'render_wall_seconds': time.perf_counter() - started,
        'width': data['width'], 'height': data['height'], 'mapping_differences': data['differences'],
        'timing_scope': 'Render invocation includes Cycles setup, tracing, optional denoising and EXR output; process startup and scene assembly excluded.'
    }
    args.output.with_suffix('.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
