"""Place Kenney CC0 commercial buildings on the village sites.

Source: City Kit (Commercial) 2.1, Kenney, CC0.
https://kenney.nl/assets/city-kit-commercial
"""

import os

import bpy
from mathutils import Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "apps", "web", "public", "models", "kits.glb")
SRC = os.environ.get("KENNEY_GLB", "/tmp/cc0src/kit/Models/GLB format")


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def load(filename, name, tx, tz, width, yaw=0.0, dx=0.0, dy=0.0, max_height=16.0):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(SRC, filename))
    meshes = [ob for ob in bpy.data.objects if ob not in before and ob.type == "MESH"]
    if not meshes:
        raise SystemExit(filename)
    bpy.ops.object.select_all(action="DESELECT")
    for mesh in meshes:
        mesh.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    span = max(ob.dimensions.x, ob.dimensions.y) or 1
    scale = width / span
    ob.scale = (scale, scale, scale)
    ob.rotation_euler[2] = yaw
    bpy.ops.object.transform_apply(rotation=True, scale=True)
    corners = [ob.matrix_world @ Vector(corner) for corner in ob.bound_box]
    cx = sum(corner.x for corner in corners) / len(corners)
    cy = sum(corner.y for corner in corners) / len(corners)
    min_z = min(corner.z for corner in corners)
    ob.location = (tx + dx - cx, -tz + dy - cy, -min_z)
    bpy.ops.object.transform_apply(location=True)
    if ob.dimensions.z > max_height:
        ob.scale = (1, 1, max_height / ob.dimensions.z)
        bpy.ops.object.transform_apply(scale=True)
        corners = [ob.matrix_world @ Vector(corner) for corner in ob.bound_box]
        min_z = min(corner.z for corner in corners)
        ob.location.z -= min_z
        bpy.ops.object.transform_apply(location=True)
    ob.name = name
    print(f"{name} {ob.dimensions.x:.1f} x {ob.dimensions.y:.1f} x {ob.dimensions.z:.1f}")
    return ob


def main():
    reset()
    load("building-j.glb", "kit.vault", -42, 18, 24)
    load("building-n.glb", "kit.market", 46, 14, 26)
    for index, dx in enumerate((-8, -2.5, 3, 8.5)):
        load("detail-awning.glb", f"kit.market.awning.{index}", 46, 14, 6, dy=-12, dx=dx)
    load("building-skyscraper-d.glb", "kit.war", 12, 40, 18)
    load("building-e.glb", "kit.forge", -38, -24, 18)
    load("low-detail-building-wide-a.glb", "kit.stadium.n", -8, -42, 22, dy=8, max_height=9)
    load("low-detail-building-wide-b.glb", "kit.stadium.e", -8, -42, 16, yaw=1.57, dx=10, max_height=9)
    load("low-detail-building-wide-a.glb", "kit.stadium.w", -8, -42, 16, yaw=-1.57, dx=-10, max_height=9)
    load("low-detail-building-a.glb", "kit.pier", 50, -28, 14, max_height=8)
    load("low-detail-building-b.glb", "kit.club", -54, -2, 14, max_height=8)
    load("low-detail-building-c.glb", "kit.oracle", 20, -10, 14, max_height=8)
    for ob in list(bpy.data.objects):
        if ob.type != "MESH":
            bpy.data.objects.remove(ob, do_unlink=True)
    bpy.ops.object.select_all(action="SELECT")
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=OUT,
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
        export_animations=False,
        export_materials="EXPORT",
        export_cameras=False,
        export_lights=False,
    )
    print("wrote", OUT, os.path.getsize(OUT))


if __name__ == "__main__":
    main()
