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

ART_STEP = 3

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


def box(kit, name, loc, size, mat, rot_z=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc)
    ob = bpy.context.active_object
    ob.name = name
    ob.scale = size
    ob.rotation_euler = (0, 0, rot_z)
    core.apply_xform(ob)
    ob.data.materials.append(mat)
    core.shade_flat(ob)
    return kit.keep(ob)


def road_points():
    order = ["card-club", "skin-forge", "cup-stadium", "launch-pier", "oracle-plaza", "trading-village", "war-room", "vault-quarter"]
    by_id = {site[0]: blender_xy(site[3], site[4]) for site in SITES}
    return [by_id[key] for key in order]


def build_roads(kit, mats):
    points = road_points()
    lamp = 0
    for index, start in enumerate(points):
        end = points[(index + 1) % len(points)]
        dx, dy = end[0] - start[0], end[1] - start[1]
        length = math.hypot(dx, dy)
        if length < 1:
            continue
        angle = math.atan2(dy, dx)
        ux, uy = dx / length, dy / length
        px, py = -uy, ux
        mid = ((start[0] + end[0]) / 2, (start[1] + end[1]) / 2)
        box(kit, f"road.asphalt.{index}", (mid[0], mid[1], 0.06), (length, 6.0, 0.08), mats["asphalt"], angle)
        box(kit, f"road.kerb.L.{index}", (mid[0] + px * 3.15, mid[1] + py * 3.15, 0.12), (length, 0.28, 0.12), mats["kerb"], angle)
        box(kit, f"road.kerb.R.{index}", (mid[0] - px * 3.15, mid[1] - py * 3.15, 0.12), (length, 0.28, 0.12), mats["kerb"], angle)
        dashes = int(length // 4)
        for dash in range(dashes):
            dist = 2 + dash * 4
            if dist > length - 2:
                break
            box(
                kit,
                f"road.dash.{index}.{dash}",
                (start[0] + ux * dist, start[1] + uy * dist, 0.12),
                (1.2, 0.12, 0.02),
                mats["paint"],
                angle,
            )
        for step in range(2, int(length) - 2, 16):
            lx = start[0] + ux * step + px * 4.2
            ly = start[1] + uy * step + py * 4.2
            kit.cylinder(f"road.lamp.{lamp}", (lx, ly, 1.6), 0.06, 3.2, mats["iron"], vertices=6)
            kit.cylinder(f"road.lamphead.{lamp}", (lx, ly, 3.25), 0.16, 0.12, mats["amber"], vertices=6)
            lamp += 1
    for index, (district, _name, prefix, tx, tz) in enumerate(SITES):
        cx, cy = blender_xy(tx, tz)
        kit.cylinder(f"{prefix}.post", (cx + 6, cy + 4, 1.1), 0.08, 2.2, mats["iron"], vertices=6)
        box(kit, f"{prefix}.bench", (cx - 5, cy + 5, 0.35), (1.3, 0.45, 0.12), mats["wood"], 0.4)
        if prefix not in ("pier", "club", "oracle"):
            box(kit, f"{prefix}.solar", (cx + 7, cy - 3, 1.6), (1.1, 0.7, 0.06), mats["glass"], 0.5)
            kit.cylinder(f"{prefix}.antenna", (cx - 6, cy - 4, 2.2), 0.04, 3.4, mats["iron"], vertices=6)


def blocked(x, y, points):
    for _district, _name, _prefix, tx, tz in SITES:
        cx, cy = blender_xy(tx, tz)
        if math.hypot(x - cx, y - cy) < 14:
            return True
    for index, start in enumerate(points):
        end = points[(index + 1) % len(points)]
        dx, dy = end[0] - start[0], end[1] - start[1]
        length = math.hypot(dx, dy) or 1
        t = max(0, min(1, ((x - start[0]) * dx + (y - start[1]) * dy) / (length * length)))
        px, py = start[0] + dx * t, start[1] + dy * t
        if math.hypot(x - px, y - py) < 6:
            return True
    return False


def build_trees(kit, mats):
    points = road_points()
    kinds = ("pine", "oak", "birch")
    placed = 0
    for n in range(240):
        if placed >= 42:
            break
        angle = n * 2.399
        radius = 28 + (n * 3) % 55
        x, y = math.cos(angle) * radius, math.sin(angle) * radius
        if abs(x) > 95 or abs(y) > 95 or blocked(x, y, points):
            continue
        kind = kinds[placed % 3]
        z = terrain_height(x, y)
        kit.cylinder(f"tree.{kind}.trunk.{placed}", (x, y, z + 0.8), 0.12 if kind != "oak" else 0.18, 1.6, mats["birch"] if kind == "birch" else mats["bark"], vertices=6)
        if kind == "pine":
            kit.cone(f"tree.pine.crown.{placed}", (x, y, z + 2.4), 1.1, 2.4, mats["needle"])
        elif kind == "oak":
            kit.ico(f"tree.oak.crown.{placed}", (x, y, z + 2.3), 1.15, mats["leaf"])
        else:
            kit.ico(f"tree.birch.crown.{placed}", (x, y, z + 2.1), 0.75, mats["leaf"])
        placed += 1
    for index in range(10):
        angle = index * 0.9
        x, y = 22 + math.cos(angle) * 6, -18 + math.sin(angle) * 4
        kit.ico(f"tree.bush.{index}", (x, y, 0.35), 0.45, mats["leaf"])
        kit.cone(f"tree.flower.{index}", (x + 1.2, y + 0.4, 0.15), 0.12, 0.2, mats["flower"])


def build_village():
    core.reset_scene()
    kit = core.Kit()
    mats = {
        "grass": textured("grass", os.path.join(TEX, "grass-albedo.png"), 0.0, 0.92, 0.18, "#5f9a4e"),
        "dirt": core.material("dirt", "#8a6a42", metallic=0.0, roughness=0.95),
        "water": core.material("water", "#7eb6c9", "#cfe7ef", 0.15, 0.05, 0.08),
        "rock": core.material("rock", "#8d8680", metallic=0.05, roughness=0.86),
        "asphalt": core.material("asphalt", "#3a3e44", metallic=0.05, roughness=0.82),
        "kerb": core.material("kerb", "#c8c2b4", metallic=0.0, roughness=0.7),
        "paint": core.material("paint", "#f3e6c8", metallic=0.0, roughness=0.5),
        "iron": core.material("iron", "#6e757c", metallic=0.7, roughness=0.4),
        "amber": core.material("amber", "#ffb347", "#ffb347", 3.0, 0.05, 0.4),
        "wood": core.material("wood", "#8a5a32", metallic=0.02, roughness=0.72),
        "glass": core.material("glass", "#2f4f6a", "#9fd0ff", 0.4, 0.2, 0.15),
        "bark": core.material("bark", "#5c4030", metallic=0.0, roughness=0.9),
        "birch": core.material("birch", "#efe8dc", metallic=0.0, roughness=0.7),
        "needle": core.material("needle", "#2f6a38", metallic=0.0, roughness=0.85),
        "leaf": core.material("leaf", "#3e8a44", metallic=0.0, roughness=0.8),
        "flower": core.material("flower", "#e07a9a", "#ffd0dc", 0.2, 0.0, 0.5),
    }
    build_terrain(kit, mats)
    if ART_STEP >= 3:
        build_roads(kit, mats)
        build_trees(kit, mats)
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
