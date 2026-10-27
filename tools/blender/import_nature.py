"""Pack a few Quaternius CC0 nature meshes into one glTF.

Source: Ultimate Nature Pack (Jun 2019), CC0.
https://opengameart.org/content/low-poly-nature-pack-1
The FBX files are not committed. Drop the pack's FBX folder on NATUR_FBX.
"""

import os
import sys

import bpy
from mathutils import Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "apps", "web", "public", "models", "nature.glb")
SRC = os.environ.get(
    "NATURE_FBX",
    "/tmp/cc0src/nature/Ultimate Nature Pack - Jun 2019/FBX",
)
KENNEY = os.environ.get("KENNEY_NATURE", "/tmp/cc0src/dress/nature")

# name, file, target height, folder. Kenney's detailed and round trees replace the faceted pines.
SPECS = [
    ("pine.a", "tree_pineRoundC.glb", 11.0, KENNEY),
    ("pine.b", "tree_pineRoundA.glb", 12.0, KENNEY),
    ("oak.a", "tree_detailed.glb", 10.5, KENNEY),
    ("oak.b", "tree_oak.glb", 9.0, KENNEY),
    ("birch.a", "tree_thin.glb", 11.0, KENNEY),
    ("birch.b", "tree_pineDefaultA.glb", 10.0, KENNEY),
    ("bush.a", "Bush_1.fbx", 1.6, SRC),
    ("bush.b", "BushBerries_1.fbx", 1.4, SRC),
    ("flower.a", "Flowers.fbx", 0.55, SRC),
    ("rock.a", "Rock_1.fbx", 1.5, SRC),
    ("rock.b", "Rock_3.fbx", 2.2, SRC),
    ("rock.c", "Rock_Moss_2.fbx", 1.8, SRC),
    ("grass.a", "Grass_Short.fbx", 0.4, SRC),
    ("plant.a", "Plant_1.fbx", 0.9, SRC),
]


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    try:
        bpy.ops.preferences.addon_enable(module="io_scene_fbx")
    except Exception:
        pass


def imported_meshes(before):
    return [ob for ob in bpy.data.objects if ob not in before and ob.type == "MESH"]


def normalize(ob, height):
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    corners = [ob.matrix_world @ Vector(corner) for corner in ob.bound_box]
    zs = [corner.z for corner in corners]
    span = max(zs) - min(zs)
    if span < 1e-4:
        return
    scale = height / span
    ob.scale = (scale, scale, scale)
    bpy.ops.object.transform_apply(scale=True)
    corners = [ob.matrix_world @ Vector(corner) for corner in ob.bound_box]
    min_z = min(corner.z for corner in corners)
    cx = sum(corner.x for corner in corners) / len(corners)
    cy = sum(corner.y for corner in corners) / len(corners)
    ob.location = (-cx, -cy, -min_z)
    bpy.ops.object.transform_apply(location=True)
    for slot in ob.material_slots:
        mat = slot.material
        if not mat or not mat.use_nodes:
            continue
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        if bsdf:
            bsdf.inputs["Metallic"].default_value = 0.0
            bsdf.inputs["Roughness"].default_value = 0.82
            if "Alpha" in bsdf.inputs:
                bsdf.inputs["Alpha"].default_value = 1.0
            label = mat.name.lower()
            if "leaf" in label:
                bsdf.inputs["Base Color"].default_value = (0.13, 0.48, 0.16, 1.0)
            elif "wood" in label or "bark" in label:
                bsdf.inputs["Base Color"].default_value = (0.45, 0.27, 0.14, 1.0)
        mat.blend_method = "OPAQUE"
        mat.use_backface_culling = False
    bpy.ops.object.shade_smooth()


def main():
    reset()
    for name, filename, height, folder in SPECS:
        path = os.path.join(folder, filename)
        before = set(bpy.data.objects)
        if path.lower().endswith(".glb"):
            bpy.ops.import_scene.gltf(filepath=path)
        else:
            bpy.ops.import_scene.fbx(filepath=path)
        meshes = imported_meshes(before)
        if not meshes:
            raise SystemExit(f"no mesh in {filename}")
        bpy.ops.object.select_all(action="DESELECT")
        for mesh in meshes:
            mesh.select_set(True)
        bpy.context.view_layer.objects.active = meshes[0]
        if len(meshes) > 1:
            bpy.ops.object.join()
        ob = bpy.context.view_layer.objects.active
        normalize(ob, height)
        ob.name = name
        print(f"{name} height {ob.dimensions.z:.2f} verts {len(ob.data.vertices)}")
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
