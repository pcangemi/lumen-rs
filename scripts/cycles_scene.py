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


def thin_glass_shader(nodes, links, ior):
    """Lossless parallel interfaces: straight transmission plus 2F/(1+F) reflection."""
    geometry = nodes.new('ShaderNodeNewGeometry')
    sided_ior = nodes.new('ShaderNodeMath')
    sided_ior.operation = 'MULTIPLY_ADD'
    links.new(geometry.outputs['Backfacing'], sided_ior.inputs[0])
    sided_ior.inputs[1].default_value = 1 / ior - ior
    sided_ior.inputs[2].default_value = ior
    fresnel = nodes.new('ShaderNodeFresnel')
    links.new(sided_ior.outputs[0], fresnel.inputs['IOR'])
    numerator = nodes.new('ShaderNodeMath')
    numerator.operation = 'MULTIPLY'
    links.new(fresnel.outputs[0], numerator.inputs[0])
    numerator.inputs[1].default_value = 2
    denominator = nodes.new('ShaderNodeMath')
    denominator.operation = 'ADD'
    links.new(fresnel.outputs[0], denominator.inputs[0])
    denominator.inputs[1].default_value = 1
    reflection = nodes.new('ShaderNodeMath')
    reflection.operation = 'DIVIDE'
    links.new(numerator.outputs[0], reflection.inputs[0])
    links.new(denominator.outputs[0], reflection.inputs[1])
    transparent = nodes.new('ShaderNodeBsdfTransparent')
    mirror = nodes.new('ShaderNodeBsdfAnisotropic')
    mirror.inputs['Color'].default_value = (1, 1, 1, 1)
    mirror.inputs['Roughness'].default_value = 0
    mixture = nodes.new('ShaderNodeMixShader')
    links.new(reflection.outputs[0], mixture.inputs[0])
    links.new(transparent.outputs[0], mixture.inputs[1])
    links.new(mirror.outputs[0], mixture.inputs[2])
    return mixture


def sphere_cloud(scene, spheres, surface):
    mesh = bpy.data.meshes.new('Sphere centers')
    mesh.from_pydata([vector(sphere['center']) for sphere in spheres], [], [])
    radius = mesh.attributes.new('lumen_radius', 'FLOAT', 'POINT')
    radius.data.foreach_set('value', [sphere['radius'] for sphere in spheres])
    obj = bpy.data.objects.new('Analytic sphere cloud', mesh)
    scene.collection.objects.link(obj)
    tree = bpy.data.node_groups.new('Analytic spheres', 'GeometryNodeTree')
    tree.interface.new_socket(name='Geometry', in_out='INPUT', socket_type='NodeSocketGeometry')
    tree.interface.new_socket(name='Geometry', in_out='OUTPUT', socket_type='NodeSocketGeometry')
    source = tree.nodes.new('NodeGroupInput')
    output = tree.nodes.new('NodeGroupOutput')
    points = tree.nodes.new('GeometryNodeMeshToPoints')
    points.mode = 'VERTICES'
    attribute = tree.nodes.new('GeometryNodeInputNamedAttribute')
    attribute.data_type = 'FLOAT'
    attribute.inputs['Name'].default_value = 'lumen_radius'
    shader = tree.nodes.new('GeometryNodeSetMaterial')
    shader.inputs['Material'].default_value = surface
    tree.links.new(source.outputs['Geometry'], points.inputs['Mesh'])
    tree.links.new(attribute.outputs['Attribute'], points.inputs['Radius'])
    tree.links.new(points.outputs['Points'], shader.inputs['Geometry'])
    tree.links.new(shader.outputs['Geometry'], output.inputs['Geometry'])
    obj.modifiers.new('Analytic spheres', 'NODES').node_group = tree
    return obj


def world_projection(nodes, links):
    geometry = nodes.new('ShaderNodeNewGeometry')
    position = nodes.new('ShaderNodeSeparateXYZ')
    links.new(geometry.outputs['Position'], position.inputs[0])
    normal = nodes.new('ShaderNodeVectorMath')
    normal.operation = 'ABSOLUTE'
    links.new(geometry.outputs['Normal'], normal.inputs[0])
    axes = nodes.new('ShaderNodeSeparateXYZ')
    links.new(normal.outputs[0], axes.inputs[0])

    def math_node(operation, first, second):
        node = nodes.new('ShaderNodeMath')
        node.operation = operation
        for index, value in enumerate((first, second)):
            if isinstance(value, (int, float)):
                node.inputs[index].default_value = value
            else:
                links.new(value, node.inputs[index])
        return node.outputs[0]

    def plane(u, v):
        node = nodes.new('ShaderNodeCombineXYZ')
        links.new(u, node.inputs[0])
        links.new(v, node.inputs[1])
        return node.outputs[0]

    def blend(factor, first, second):
        node = nodes.new('ShaderNodeMixRGB')
        links.new(factor, node.inputs[0])
        links.new(first, node.inputs[1])
        links.new(second, node.inputs[2])
        return node.outputs[0]

    def axis(value):
        node = nodes.new('ShaderNodeCombineXYZ')
        for socket, component in zip(node.inputs, value):
            socket.default_value = component
        return node.outputs[0]

    x, y, z = position.outputs
    lumen_z = math_node('MULTIPLY', y, -1)
    x_dominant = math_node('GREATER_THAN', axes.outputs['X'], axes.outputs['Y'])
    horizontal_max = math_node('MAXIMUM', axes.outputs['X'], axes.outputs['Y'])
    vertical_dominant = math_node('SUBTRACT', 1,
        math_node('GREATER_THAN', horizontal_max, axes.outputs['Z']))
    walls = blend(x_dominant, plane(x, z), plane(lumen_z, z))
    wall_tangent = blend(x_dominant, axis((1, 0, 0)), axis((0, -1, 0)))
    tangent = blend(vertical_dominant, wall_tangent, axis((1, 0, 0)))
    bitangent = blend(vertical_dominant, axis((0, 0, 1)), axis((0, -1, 0)))
    return blend(vertical_dominant, walls, plane(x, lumen_z)), tangent, bitangent


def projected_normal(nodes, links, color, tangent, bitangent):
    def vector_math(operation, *values):
        node = nodes.new('ShaderNodeVectorMath')
        node.operation = operation
        for index, value in enumerate(values):
            socket = node.inputs['Scale'] if operation == 'SCALE' and index == 1 else node.inputs[index]
            if isinstance(value, (int, float, tuple)):
                socket.default_value = value
            else:
                links.new(value, socket)
        return node.outputs['Value' if operation == 'DOT_PRODUCT' else 'Vector']

    normal = nodes.new('ShaderNodeNewGeometry').outputs['Normal']
    normal_component = vector_math('SCALE', normal, vector_math('DOT_PRODUCT', normal, tangent))
    tangent = vector_math('NORMALIZE', vector_math('SUBTRACT', tangent, normal_component))
    perpendicular = vector_math('CROSS_PRODUCT', normal, tangent)
    handedness = nodes.new('ShaderNodeMath')
    handedness.operation = 'SIGN'
    links.new(vector_math('DOT_PRODUCT', perpendicular, bitangent), handedness.inputs[0])
    bitangent = vector_math('SCALE', perpendicular, handedness.outputs[0])
    mapped = nodes.new('ShaderNodeSeparateXYZ')
    links.new(vector_math('ADD', vector_math('SCALE', color, 2), (-1, -1, -1)), mapped.inputs[0])
    tangent_component = vector_math('SCALE', tangent, mapped.outputs['X'])
    bitangent_component = vector_math('SCALE', bitangent, mapped.outputs['Y'])
    normal_component = vector_math('SCALE', normal, mapped.outputs['Z'])
    offset = vector_math('ADD', tangent_component, bitangent_component)
    return vector_math('NORMALIZE', vector_math('ADD', offset, normal_component))


def material(record, textures, vertex_colors, point_cloud=False):
    result = bpy.data.materials.new('Surface')
    result.use_nodes = True
    nodes, links = result.node_tree.nodes, result.node_tree.links
    nodes.clear()
    output = nodes.new('ShaderNodeOutputMaterial')
    base, surface = record['base'], record['surface']
    emission, absorption, detail = record['emission'], record['absorption'], record['detail']
    frame = None
    if not vertex_colors and record.get('world_projection'):
        coordinates, tangent, bitangent = world_projection(nodes, links)
        frame = (tangent, bitangent)
    elif point_cloud:
        raise ValueError('Textured sphere clouds require world projection')
    else:
        coordinates = nodes.new('ShaderNodeTexCoord').outputs['UV']

    def image_texture(index):
        scale = nodes.new('ShaderNodeVectorMath')
        scale.operation = 'SCALE'
        scale.inputs['Scale'].default_value = abs(detail[3])
        links.new(coordinates, scale.inputs[0])
        texture = nodes.new('ShaderNodeTexImage')
        texture.image = textures[index]
        texture.interpolation = 'Linear'
        texture.extension = 'REPEAT'
        links.new(scale.outputs[0], texture.inputs['Vector'])
        return texture

    if max(emission[:3]) > 0:
        shader = nodes.new('ShaderNodeEmission')
        shader.inputs['Color'].default_value = (*emission[:3], 1)
        links.new(shader.outputs[0], output.inputs['Surface'])
        return result
    if surface[2] == 2:
        shader = thin_glass_shader(nodes, links, surface[3])
    elif surface[2] > 0.5:
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
        else:
            color = nodes.new('ShaderNodeRGB')
            color.outputs[0].default_value = (*base[:3], 1)
            color_output = color.outputs[0]
        if detail[2] >= 1:
            texture = image_texture(int(detail[2]) - 1)
            strength = nodes.new('ShaderNodeMixRGB')
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
    if surface[2] <= 0.5:
        maps = record.get('maps', [0, 0, 0, 0])
        for lane, socket, factor in [(0, 'Normal', 1), (1, 'Roughness', surface[0]), (2, 'Metallic', surface[1])]:
            if not maps[lane]:
                continue
            texture = image_texture(maps[lane] - 1)
            if lane == 0:
                if frame:
                    normal = projected_normal(nodes, links, texture.outputs['Color'], *frame)
                else:
                    mapped = nodes.new('ShaderNodeNormalMap')
                    links.new(texture.outputs['Color'], mapped.inputs['Color'])
                    normal = mapped.outputs['Normal']
                links.new(normal, shader.inputs[socket])
            else:
                multiply = nodes.new('ShaderNodeMath')
                multiply.operation = 'MULTIPLY'
                multiply.inputs[1].default_value = factor
                links.new(texture.outputs['Color'], multiply.inputs[0])
                links.new(multiply.outputs[0], shader.inputs[socket])
    if surface[2] <= 0.5 and record.get('maps', [0, 0, 0, 0])[3]:
        mask = image_texture(record['maps'][3] - 1)
        # Lumen accepts values equal to the cutoff: alpha = 1 - (mask < 0.5).
        threshold = nodes.new('ShaderNodeMath')
        threshold.operation = 'LESS_THAN'
        threshold.inputs[1].default_value = 0.5
        links.new(mask.outputs['Color'], threshold.inputs[0])
        accepted = nodes.new('ShaderNodeMath')
        accepted.operation = 'SUBTRACT'
        accepted.inputs[0].default_value = 1
        links.new(threshold.outputs[0], accepted.inputs[1])
        links.new(accepted.outputs[0], shader.inputs['Alpha'])
    if surface[2] <= 0.5 and absorption[3] > 0:
        transmitted = nodes.new('ShaderNodeBsdfTranslucent')
        transmitted.inputs['Color'].default_value = (*base[:3], 1)
        links.new(color_output, transmitted.inputs['Color'])
        mixture = nodes.new('ShaderNodeMixShader')
        mixture.inputs[0].default_value = absorption[3]
        links.new(shader.outputs[0], mixture.inputs[1])
        links.new(transmitted.outputs[0], mixture.inputs[2])
        shader = mixture
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
    sphere_groups = {}
    for sphere in data['spheres']:
        sphere_groups.setdefault(sphere['material'], []).append(sphere)
    for index, spheres in sphere_groups.items():
        record = data['materials'][index]
        opaque = record['surface'][2] == 0 and max(record['emission'][:3]) == 0
        cloud = len(spheres) >= 64 and opaque and record.get('world_projection')
        surface = material(record, textures, False, point_cloud=cloud)
        if cloud:
            sphere_cloud(scene, spheres, surface)
            continue
        for sphere in spheres:
            bpy.ops.mesh.primitive_uv_sphere_add(segments=128, ring_count=64,
                radius=sphere['radius'], location=vector(sphere['center']))
            obj = bpy.context.object
            obj.name = 'Analytic sphere approximation'
            obj.data.materials.append(surface)
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
