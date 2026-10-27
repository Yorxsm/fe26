"""CC0 interiors for the five live arenas. Kenney Furniture Kit and Survival Kit.

Rooms are built at the origin, open toward Blender -Y, which is glTF +Z.
"""

import math
import os

import bpy
from mathutils import Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT_DIR = os.path.join(ROOT, "apps", "web", "public", "models")
FURN = "/tmp/cc0src/dress/furniture"
SURV = "/tmp/cc0src/dress/survival"
POSTER = "/tmp/cc0src/dress/poster.png"


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def place(filename, name, x, y, yaw=0.0, height=None, wide=None, deep=None, z=0.0, src=FURN):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(src, filename))
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
    if height:
        scale = height / (ob.dimensions.z or 1)
        ob.scale = (scale, scale, scale)
        bpy.ops.object.transform_apply(scale=True)
    if wide:
        ob.scale = (wide / (ob.dimensions.x or 1), 1, 1)
        bpy.ops.object.transform_apply(scale=True)
    if deep:
        ob.scale = (1, deep / (ob.dimensions.y or 1), 1)
        bpy.ops.object.transform_apply(scale=True)
    ob.rotation_euler[2] = yaw
    bpy.ops.object.transform_apply(rotation=True)
    corners = [ob.matrix_world @ Vector(corner) for corner in ob.bound_box]
    cx = sum(corner.x for corner in corners) / len(corners)
    cy = sum(corner.y for corner in corners) / len(corners)
    min_z = min(corner.z for corner in corners)
    ob.location = (x - cx, y - cy, z - min_z)
    bpy.ops.object.transform_apply(location=True)
    ob.name = name
    return ob


def material(name, color, metallic=0.0, roughness=0.6, image=None):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = next(node for node in mat.node_tree.nodes if node.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Base Color"].default_value = (*color, 1)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    if image and os.path.exists(image):
        tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
        tex.image = bpy.data.images.load(image)
        mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    return mat


def bar(name, x, y, z):
    bpy.ops.mesh.primitive_cube_add(size=1)
    ob = bpy.context.active_object
    ob.scale = (0.62, 0.22, 0.16)
    bpy.ops.object.transform_apply(scale=True)
    ob.location = (x, y, z)
    bpy.ops.object.transform_apply(location=True)
    ob.data.materials.append(material("gold-bar", (0.86, 0.62, 0.16), 0.92, 0.28))
    ob.name = name
    return ob


def poster(name, x, y, yaw):
    bpy.ops.mesh.primitive_plane_add(size=1)
    ob = bpy.context.active_object
    ob.scale = (1.15, 1.45, 1)
    ob.rotation_euler = (math.pi / 2, 0, yaw)
    bpy.ops.object.transform_apply(rotation=True, scale=True)
    ob.location = (x, y, 1.7)
    bpy.ops.object.transform_apply(location=True)
    ob.data.materials.append(material(f"{name}-art", (0.2, 0.2, 0.2), image=POSTER))
    ob.name = name
    return ob


def shell(prefix):
    place("floorFull.glb", f"{prefix}.floor", 0, 0, wide=8.2, deep=7.2, z=0.02)
    place("wall.glb", f"{prefix}.wall.n", 0, 3.45, height=3.15, wide=8.2)
    place("wall.glb", f"{prefix}.wall.w", -4.05, 0, yaw=math.pi / 2, height=3.15, wide=7.0)
    place("wall.glb", f"{prefix}.wall.e", 4.05, 0, yaw=-math.pi / 2, height=3.15, wide=7.0)
    place("rugRectangle.glb", f"{prefix}.rug", 0, 0.2, wide=3.6, deep=2.4, z=0.04)


def export_room(prefix, filename):
    bpy.ops.object.select_all(action="DESELECT")
    for ob in bpy.data.objects:
        ob.select_set(ob.name.startswith(prefix))
    path = os.path.join(OUT_DIR, filename)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
        export_animations=False,
        export_materials="EXPORT",
    )
    print("wrote", filename, os.path.getsize(path))


def main():
    reset()
    shell("room.vault")
    place("desk.glb", "room.vault.desk", -0.4, 1.5, height=0.78)
    place("chairDesk.glb", "room.vault.chair", -0.3, 0.55, yaw=math.pi, height=0.95)
    place("bookcaseOpen.glb", "room.vault.shelf", -2.7, 2.5, height=2.15)
    place("books.glb", "room.vault.books", -0.1, 1.55, height=0.16, z=0.78)
    place("lampRoundTable.glb", "room.vault.lamp", -1.15, 1.45, height=0.42, z=0.78)
    place("pottedPlant.glb", "room.vault.plant", 2.5, 2.4, height=1.35)
    place("cardboardBoxClosed.glb", "room.vault.box", 2.3, -0.6, height=0.48)
    place("structure-metal-doorway.glb", "room.vault.door", 1.4, 3.15, height=2.55, src=SURV)
    poster("room.vault.poster", -3.92, -0.4, math.pi / 2)
    for index, dx in enumerate((-0.3, 0.35, 1.0)):
        bar(f"room.vault.gold.{index}", dx, 0.15, 0.2)

    shell("room.market")
    place("table.glb", "room.market.stall.0", -1.8, 1.2, height=0.78)
    place("tableCloth.glb", "room.market.stall.1", 1.6, 1.3, height=0.78)
    place("cardboardBoxOpen.glb", "room.market.goods.0", -1.7, 1.25, height=0.32, z=0.78)
    place("cardboardBoxClosed.glb", "room.market.goods.1", 1.5, 1.35, height=0.36, z=0.78)
    place("books.glb", "room.market.goods.2", 1.9, 1.15, height=0.14, z=0.78)
    place("bookcaseOpen.glb", "room.market.shelf", -2.8, 2.45, height=2.0)
    place("pottedPlant.glb", "room.market.plant", 2.6, 2.3, height=1.2)
    place("lampSquareFloor.glb", "room.market.lamp", 0.2, 2.6, height=1.7)
    place("plantSmall1.glb", "room.market.sprout", -1.9, 0.2, height=0.35)
    poster("room.market.poster", 0, 3.32, 0)

    shell("room.war")
    place("desk.glb", "room.war.desk", 0.2, 1.45, height=0.78)
    place("chairDesk.glb", "room.war.chair", 0.15, 0.5, yaw=math.pi, height=0.95)
    place("computerScreen.glb", "room.war.screen", 0.35, 1.7, height=0.48, z=0.78)
    place("laptop.glb", "room.war.laptop", -0.45, 1.45, height=0.16, z=0.78)
    place("computerKeyboard.glb", "room.war.keys", 0.15, 1.25, height=0.05, z=0.78)
    place("bookcaseOpen.glb", "room.war.shelf", -2.7, 2.4, height=2.1)
    place("books.glb", "room.war.books", -2.7, 2.15, height=0.18, z=0.9)
    place("lampRoundTable.glb", "room.war.lamp", 1.15, 1.55, height=0.4, z=0.78)
    place("pottedPlant.glb", "room.war.plant", 2.6, 2.35, height=1.25)
    poster("room.war.poster", 3.92, 0.2, -math.pi / 2)

    shell("room.forge")
    place("workbench-anvil.glb", "room.forge.anvil", -0.2, 0.8, height=1.05, src=SURV)
    place("tool-hammer.glb", "room.forge.hammer", 1.3, 1.4, yaw=0.6, height=0.7, src=SURV)
    place("tool-axe.glb", "room.forge.axe", 1.7, 0.6, yaw=-0.4, height=0.85, src=SURV)
    place("barrel.glb", "room.forge.barrel", -2.4, 1.8, height=1.05, src=SURV)
    place("box.glb", "room.forge.crate", 2.3, 1.6, height=0.7, src=SURV)
    place("cardboardBoxClosed.glb", "room.forge.box", 2.2, -0.4, height=0.42)
    place("lampSquareFloor.glb", "room.forge.lamp", -2.5, -0.8, height=1.6)
    poster("room.forge.poster", 0.2, 3.32, 0)

    shell("room.stadium")
    place("bench.glb", "room.stadium.bench", 0, 0.3, wide=2.4, height=0.5)
    place("cabinetTelevision.glb", "room.stadium.locker", -2.4, 2.3, height=1.8)
    place("computerScreen.glb", "room.stadium.screen", 1.8, 2.2, height=0.7)
    place("sideTable.glb", "room.stadium.table", 1.7, 1.5, height=0.7)
    place("pottedPlant.glb", "room.stadium.plant", 2.6, -0.5, height=1.1)
    place("cardboardBoxClosed.glb", "room.stadium.crate", -2.2, -0.6, height=0.45)

    for prefix, filename in (
        ("room.vault", "interior-vault.glb"),
        ("room.market", "interior-market.glb"),
        ("room.war", "interior-war.glb"),
        ("room.forge", "interior-forge.glb"),
        ("room.stadium", "interior-stadium.glb"),
    ):
        export_room(prefix, filename)


if __name__ == "__main__":
    main()
