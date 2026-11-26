"""Prepare the licensed SolidXperts scan as a closed, unit-height Y-up OBJ.

Run with Blender --background --python scripts/prepare-guardian-lion.py -- ARCHIVE.
The archive is downloaded by the user from the source credited in NOTICE.md.
Only the main connected sculpture is retained. Its base is leveled and small
openings are capped; the carved surface is neither decimated nor globally smoothed.
"""
import hashlib
import io
import json
import sys
import zipfile
from pathlib import Path

import bmesh
import bpy
import numpy as np
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
ARCHIVE_SHA256 = "d2c42f4c41f970e07a878960b2eeb9b0ec4e4d137915b08f322c26c8c9719bc4"
SOURCE_NAME = "Prescott Library_Textured.obj"
OUTPUT = ROOT / "assets/models/guardian_lion"


def connected_components(mesh):
    seen = set()
    components = []
    for vertex in mesh.verts:
        if vertex in seen:
            continue
        seen.add(vertex)
        stack = [vertex]
        component = []
        while stack:
            current = stack.pop()
            component.append(current)
            for edge in current.link_edges:
                neighbor = edge.other_vert(current)
                if neighbor not in seen:
                    seen.add(neighbor)
                    stack.append(neighbor)
        components.append(component)
    return components


def close_scan_openings(mesh, base_z):
    mesh.edges.index_update()
    remaining = {edge for edge in mesh.edges if edge.is_boundary}
    holes = []
    while remaining:
        edge = min(remaining, key=lambda e: e.index)
        vertices = [edge.verts[0]]
        current = vertices[0]
        ring = []
        while True:
            remaining.remove(edge)
            ring.append(edge)
            current = edge.other_vert(current)
            if current == vertices[0]:
                break
            vertices.append(current)
            candidates = [e for e in current.link_edges if e in remaining]
            if len(candidates) != 1:
                raise ValueError("Scan opening has a non-simple boundary")
            edge = candidates[0]
        center = sum((v.co for v in vertices), Vector()) / len(vertices)
        radius = max((v.co - center).length for v in vertices)
        base = all(abs(v.co.z - base_z) < 0.01 for v in vertices)
        if radius > 20.0 and not base:
            raise ValueError("Unexpected large scan opening; inspect before repairing")
        cap = mesh.verts.new(center)
        for edge in ring:
            mesh.faces.new((edge.verts[0], edge.verts[1], cap))
        holes.append({"edges": len(ring), "radius_source_units": radius, "base": base})
    return holes


def prepare(archive):
    data = archive.read_bytes()
    if hashlib.sha256(data).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("Archive checksum differs from the inspected source")
    cache = ROOT / ".runtime/source-assets/guardian_lion/solid_xperts"
    cache.mkdir(parents=True, exist_ok=True)
    # Extract explicit members only; archive paths never control output paths.
    with zipfile.ZipFile(io.BytesIO(data)) as outer:
        with zipfile.ZipFile(io.BytesIO(outer.read("source/Prescott Library_Textured.zip"))) as inner:
            source = inner.read(SOURCE_NAME)
            (cache / SOURCE_NAME).write_bytes(source)

    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.wm.obj_import(
        filepath=str(cache / SOURCE_NAME), forward_axis="Y", up_axis="Z",
        use_split_objects=False, use_split_groups=False,
    )
    obj = next(o for o in bpy.context.scene.objects if o.type == "MESH")
    mesh = bmesh.new()
    mesh.from_mesh(obj.data)
    source_faces = len(mesh.faces)
    components = connected_components(mesh)
    sculpture = max(components, key=len)
    bmesh.ops.delete(mesh, geom=[v for c in components if c is not sculpture for v in c], context="VERTS")
    retained_faces = len(mesh.faces)
    # The scan's underside is tilted and contains nearly coplanar slivers.
    # Fit its support plane, level the whole sculpture rigidly, and resurface
    # just one source millimeter above that plane so the pedestal sits flat.
    points = np.array([tuple(v.co) for v in mesh.verts])
    bottom = points[points[:, 2] < points[:, 2].min() + 30.0]
    design = np.column_stack((bottom[:, :2], np.ones(len(bottom))))
    keep = np.ones(len(bottom), dtype=bool)
    for _ in range(8):
        plane = np.linalg.lstsq(design[keep], bottom[keep, 2], rcond=None)[0]
        residual = bottom[:, 2] - design @ plane
        keep = abs(residual) < np.quantile(abs(residual), 0.65)
    normal = Vector((-plane[0], -plane[1], 1.0)).normalized()
    rotation = normal.rotation_difference(Vector((0, 0, 1)))
    for vertex in mesh.verts:
        vertex.co = rotation @ vertex.co
    base_z = float(plane[2]) * normal.z + 1.0
    bmesh.ops.bisect_plane(
        mesh, geom=list(mesh.verts) + list(mesh.edges) + list(mesh.faces),
        dist=0.00001, plane_co=(0, 0, base_z), plane_no=(0, 0, 1), clear_inner=True,
    )
    holes = close_scan_openings(mesh, base_z)
    bmesh.ops.triangulate(mesh, faces=list(mesh.faces))
    bmesh.ops.recalc_face_normals(mesh, faces=list(mesh.faces))
    if mesh.calc_volume(signed=True) < 0:
        bmesh.ops.reverse_faces(mesh, faces=list(mesh.faces))
    if any(not e.is_manifold for e in mesh.edges) or any(not v.is_manifold for v in mesh.verts):
        print("Remaining edge defects", [(len(e.link_faces), [list(v.co) for v in e.verts]) for e in mesh.edges if not e.is_manifold][:20], flush=True)
        raise ValueError("Converted sculpture must be a closed manifold")
    if len(connected_components(mesh)) != 1:
        raise ValueError("Converted sculpture must be one connected shell")

    lo = Vector(tuple(min(v.co[k] for v in mesh.verts) for k in range(3)))
    hi = Vector(tuple(max(v.co[k] for v in mesh.verts) for k in range(3)))
    height = hi.z - lo.z
    center = Vector(((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, lo.z))
    for vertex in mesh.verts:
        p = (vertex.co - center) / height
        vertex.co = (p.x, p.z, -p.y)
    collapsed = 0
    for face in list(mesh.faces):
        if face.is_valid and face.calc_area() * 1.4 ** 2 <= 2e-12:
            edge = min(face.edges, key=lambda e: e.calc_length())
            if edge.calc_length() > 0.000002:
                raise ValueError("Unexpected sliver needs manual inspection")
            bmesh.ops.collapse(mesh, edges=[edge])
            collapsed += 1
    bmesh.ops.recalc_face_normals(mesh, faces=list(mesh.faces))
    mesh.normal_update()
    if min(f.calc_area() for f in mesh.faces) * 1.4 ** 2 <= 1e-12:
        print("Tiny triangles", [(f.calc_area(), [list(v.co) for v in f.verts]) for f in mesh.faces if f.calc_area() * 1.4 ** 2 <= 1e-12][:20], flush=True)
        raise ValueError("Remaining triangle below renderer area tolerance")
    if any(not e.is_manifold for e in mesh.edges) or any(not v.is_manifold for v in mesh.verts):
        raise ValueError("Microscopic edge repair must preserve the closed shell")
    mesh.verts.index_update()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    destination = OUTPUT / "guardian_lion.obj"
    with destination.open("w") as file:
        file.write("# SolidXperts, CC BY-NC 4.0. See NOTICE.md. Unit-height, Y-up.\n")
        for vertex in mesh.verts:
            file.write("v " + " ".join(f"{v:.9g}" for v in vertex.co) + "\n")
        for vertex in mesh.verts:
            file.write("vn " + " ".join(f"{v:.9g}" for v in vertex.normal) + "\n")
        for face in mesh.faces:
            file.write("f " + " ".join(f"{v.index + 1}//{v.index + 1}" for v in face.verts) + "\n")
    report = {
        "source": "https://sketchfab.com/3d-models/chinese-guardian-lions-c981456846cc49f59cfa92553b7a7506",
        "author": "SolidXperts", "license": "CC-BY-NC-4.0",
        "archive_sha256": ARCHIVE_SHA256,
        "source_obj_sha256": hashlib.sha256(source).hexdigest(),
        "source_triangles": source_faces, "source_components": len(components),
        "retained_surface_triangles": retained_faces, "closed_openings": holes,
        "base_leveling_plane": plane.tolist(), "base_resurfacing_source_units": 1.0,
        "collapsed_micrometer_edges": collapsed,
        "triangles": len(mesh.faces), "vertices": len(mesh.verts),
        "boundary_edges": 0, "nonmanifold_edges": 0,
        "unit_dimensions_xyz": [(hi.x - lo.x) / height, 1.0, (hi.y - lo.y) / height],
        "unit_volume": mesh.calc_volume(signed=True),
        "output_sha256": hashlib.sha256(destination.read_bytes()).hexdigest(),
    }
    (OUTPUT / "conversion.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2), flush=True)
    mesh.free()


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if len(args) != 1:
        raise SystemExit("Pass the downloaded chinese-guardian-lions.zip after --")
    prepare(Path(args[0]).expanduser().resolve())
