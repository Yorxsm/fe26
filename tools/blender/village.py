"""Outdoor technology village. Blender is Z-up. layout.json is glTF Y-up.

Step 2 is terrain: grass, edge hills, a pond, rocks, and flat pads where
the arenas will stand. Later steps add roads and buildings.
"""

import json
import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import build_station as core

ART_STEP = 2

# three.js (x, z). Blender y is -z.
SITES = [
    ("vault-quarter", "Vault Bank", "vault", -42.0, 18.0),
    ("cup-stadium", "Trading Cup Stadium", "stadium", -8.0, -42.0),
    ("trading-village", "Trading Market", "market", 46.0, 14.0),
    ("war-room", "Hacker War Room", "war", 12.0, 40.0),
    ("skin-forge", "Skin Forge", "forge", -38.0, -24.0),
    ("launch-pier", "Coming Soon", "pier", 50.0, -28.0),
    ("card-club", "Coming Soon", "club", -54.0, -2.0),
    ("oracle-plaza", "Coming Soon", "oracle", 20.0, -10.0),
]

TEX = os.path.join(core.ROOT, "apps", "web", "public", "textures")
POND = (8.0, -6.0)  # blender x, y


def blender_xy(tx, tz):
    return tx, -tz


def textured(name, albedo, metal, rough, uv_scale, tint):
    mat = core.material(name, tint, metallic=metal, roughness=rough)
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    coord = nodes.new("ShaderNodeTexCoord")
    mapping = nodes.new("ShaderNodeMapping")
    mapping.inputs["Scale"].default_value = (uv_scale, uv_scale, uv_scale)
    links.new(coord.outputs["Object"], mapping.inputs["Vector"])
    color = nodes.new("ShaderNodeTexImage")
    color.image = bpy.data.images.load(albedo)
    links.new(mapping.outputs["Vector"], color.inputs["Vector"])
    links.new(color.outputs["Color"], nodes["Principled BSDF"].inputs["Base Color"])
    return mat


def terrain_height(x, y):
    radius = math.hypot(x, y)
    edge = max(0.0, (radius - 62.0) / 36.0)
    hills = edge * 2.8 * math.sin(x * 0.05 + 0.6) * math.cos(y * 0.045)
    hills += edge * 0.9 * math.sin(x * 0.11 + y * 0.04)
    pond = math.hypot(x - POND[0], y - POND[1])
    if pond < 9:
        hills -= (1 - pond / 9) * 0.7
    return hills


def build_terrain(kit, mats):
    bpy.ops.mesh.primitive_grid_add(x_subdivisions=70, y_subdivisions=70, size=220, location=(0, 0, 0))
    ground = bpy.context.active_object
    ground.name = "terrain.grass"
    for vertex in ground.data.vertices:
        vertex.co.z = terrain_height(vertex.co.x, vertex.co.y)
    for poly in ground.data.polygons:
        poly.use_smooth = True
    ground.data.materials.append(mats["grass"])
    kit.keep(ground)
    kit.cylinder("terrain.pond", (POND[0], POND[1], -0.22), 6.2, 0.12, mats["water"], vertices=24)
    for index, (x, y, scale) in enumerate(
        ((-78, 20, 1.4), (70, -55, 1.1), (-60, -70, 0.8), (82, 30, 1.6), (40, 78, 0.9), (-20, 84, 1.2))
    ):
        kit.ico(f"terrain.rock.{index}", (x, y, terrain_height(x, y) + scale * 0.35), scale, mats["rock"])
    for _district, _name, prefix, tx, tz in SITES:
        cx, cy = blender_xy(tx, tz)
        kit.cylinder(f"{prefix}.pad", (cx, cy, 0.04), 11, 0.08, mats["dirt"], vertices=20)


def build_village():
    core.reset_scene()
    kit = core.Kit()
    mats = {
        "grass": textured("grass", os.path.join(TEX, "grass-albedo.png"), 0.0, 0.92, 0.18, "#5f9a4e"),
        "dirt": core.material("dirt", "#8a6a42", metallic=0.0, roughness=0.95),
        "water": core.material("water", "#7eb6c9", "#cfe7ef", 0.15, 0.05, 0.08),
        "rock": core.material("rock", "#8d8680", metallic=0.05, roughness=0.86),
    }
    build_terrain(kit, mats)
    rooms = []
    for district, name, _prefix, tx, tz in SITES:
        cx, cy = blender_xy(tx, tz)
        kit.marker(f"marker.{district}", (cx, cy, 0.4), district)
        rooms.append(
            {
                "id": district,
                "name": name,
                "position": core.gltf_position(cx, cy, 0),
                "size": [22, 0.2, 22],
            }
        )
    os.makedirs(core.OUT, exist_ok=True)
    core.export_glb(kit.objects, os.path.join(core.OUT, "station.glb"), animations=False)
    with open(os.path.join(core.OUT, "layout.json"), "w", encoding="utf-8") as handle:
        json.dump({"units": "gltf-y-up-meters", "artStep": ART_STEP, "rooms": rooms}, handle, indent=2)
        handle.write("\n")
    print(f"village step {ART_STEP} objects {len(kit.objects)}")
