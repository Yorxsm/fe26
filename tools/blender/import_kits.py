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
HOLIDAY = "/tmp/cc0src/dress/holiday"
RACING = "/tmp/cc0src/dress/racing"
SUBURBAN = "/tmp/cc0src/dress/suburban"
SURVIVAL = "/tmp/cc0src/dress/survival"
CRANE_FBX = "/tmp/cc0src/zips/crane.fbx"
CRANE_DIFF = "/tmp/cc0src/dress/crane-diff.jpg"


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def load(filename, name, tx, tz, size, yaw=0.0, dx=0.0, dy=0.0, max_height=16.0, src=SRC, dz=0.0, fit="footprint"):
    before = set(bpy.data.objects)
    path = filename if os.path.isabs(filename) else os.path.join(src, filename)
    if path.lower().endswith(".fbx"):
        bpy.ops.preferences.addon_enable(module="io_scene_fbx")
        bpy.ops.import_scene.fbx(filepath=path)
    else:
        bpy.ops.import_scene.gltf(filepath=path)
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
    if fit == "height":
        span = ob.dimensions.z or 1
    elif fit == "max":
        span = max(ob.dimensions.x, ob.dimensions.y, ob.dimensions.z) or 1
    else:
        span = max(ob.dimensions.x, ob.dimensions.y) or 1
    scale = size / span
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
    if dz:
        ob.location.z += dz
        bpy.ops.object.transform_apply(location=True)
    ob.name = name
    print(f"{name} {ob.dimensions.x:.1f} x {ob.dimensions.y:.1f} x {ob.dimensions.z:.1f}")
    return ob


def paint_crane(ob):
    if not os.path.exists(CRANE_DIFF):
        return
    image = bpy.data.images.load(CRANE_DIFF)
    image.name = "crane-diff"
    for mat in ob.data.materials:
        if mat is None:
            continue
        mat.use_nodes = True
        nodes = mat.node_tree.nodes
        links = mat.node_tree.links
        bsdf = next((node for node in nodes if node.type == "BSDF_PRINCIPLED"), None)
        if bsdf is None:
            continue
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = image
        links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        bsdf.inputs["Roughness"].default_value = 0.62
        bsdf.inputs["Metallic"].default_value = 0.08


def main():
    reset()
    load("building-j.glb", "kit.vault", -42, 18, 24)
    load("building-n.glb", "kit.market", 46, 14, 26)
    for index, dx in enumerate((-8, -2.5, 3, 8.5)):
        load("detail-awning.glb", f"kit.market.awning.{index}", 46, 14, 6, dy=-12, dx=dx)
    load("building-skyscraper-d.glb", "kit.war", 12, 40, 18)
    load("building-e.glb", "kit.forge", -38, -24, 18)
    load("low-detail-building-a.glb", "kit.pier", 50, -28, 14, max_height=8)
    load("low-detail-building-b.glb", "kit.club", -54, -2, 14, max_height=8)
    load("low-detail-building-c.glb", "kit.oracle", 20, -10, 14, max_height=8)
    # Portico in front of the bank, gold-sign post, and a roof dish / chimney.
    load("detail-overhang.glb", "kit.vault.portico", -42, 18, 16, dy=-11, max_height=8)
    load("signpost.glb", "kit.vault.sign", -42, 18, 3.2, dy=-14, fit="height", src=SURVIVAL, max_height=8)
    load("lights-colored.glb", "kit.market.lights", 46, 14, 18, dy=-13, dz=6.2, src=HOLIDAY, max_height=8)
    load("grandStandCovered.glb", "kit.stadium.n", -8, -42, 24, dy=12, max_height=10, src=RACING)
    load("grandStand.glb", "kit.stadium.e", -8, -42, 18, yaw=1.57, dx=14, max_height=10, src=RACING)
    load("grandStand.glb", "kit.stadium.w", -8, -42, 18, yaw=-1.57, dx=-14, max_height=10, src=RACING)
    for index, (dx, dy) in enumerate(((-16, 14), (16, 14), (-16, -6), (16, -6))):
        load("lightPostLarge.glb", f"kit.stadium.flood.{index}", -8, -42, 12, dx=dx, dy=dy, fit="height", src=RACING, max_height=14)
    load("overheadLights.glb", "kit.stadium.gantry", -8, -42, 20, max_height=12, src=RACING)
    for index, (dx, dy) in enumerate(((-12, 8), (12, 8), (-12, -2), (12, -2))):
        flag = "flagGreen.glb" if index % 2 == 0 else "flagRed.glb"
        load(flag, f"kit.stadium.flag.{index}", -8, -42, 6, dx=dx, dy=dy, fit="height", src=RACING, max_height=8)
    load("radarEquipment.glb", "kit.war.dish", 12, 40, 4.5, dz=15.6, fit="height", src=RACING, max_height=6)
    load("cabin-roof-chimney.glb", "kit.forge.chimney", -38, -24, 4.2, dx=4, dy=2, dz=9.2, fit="height", src=HOLIDAY, max_height=6)
    if os.path.exists(CRANE_FBX):
        crane = load(CRANE_FBX, "kit.pier.crane", 50, -28, 16, dx=0, dy=16, fit="max", max_height=18)
        paint_crane(crane)
    for tx, tz, prefix in ((50, -28, "kit.pier"), (-54, -2, "kit.club"), (20, -10, "kit.oracle")):
        load("fence-1x4.glb", f"{prefix}.fence.s", tx, tz, 14, dy=-9, src=SUBURBAN, max_height=4)
        load("fence-1x4.glb", f"{prefix}.fence.n", tx, tz, 14, dy=9, yaw=3.1416, src=SUBURBAN, max_height=4)
        load("fence-1x4.glb", f"{prefix}.fence.e", tx, tz, 14, dx=9, yaw=1.5708, src=SUBURBAN, max_height=4)
        load("fence-1x4.glb", f"{prefix}.fence.w", tx, tz, 14, dx=-9, yaw=-1.5708, src=SUBURBAN, max_height=4)
        for index, (dx, dy) in enumerate(((7, 7), (7, -7), (-7, 7))):
            load("pylon.glb", f"{prefix}.cone.{index}", tx, tz, 1.5, dx=dx, dy=dy, fit="height", src=RACING, max_height=3)
        load("structure-metal.glb", f"{prefix}.scaffold", tx, tz, 6, dx=5, src=SURVIVAL, max_height=8)
        load("resource-planks.glb", f"{prefix}.planks", tx, tz, 5, dx=5, dy=1.2, dz=3.4, src=SURVIVAL, max_height=3)
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
