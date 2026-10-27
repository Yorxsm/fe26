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

ART_STEP = 4

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


def disc(kit, name, loc, radius, depth, mat, rot, verts=20):
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts, radius=radius, depth=depth, location=loc, rotation=rot)
    ob = bpy.context.active_object
    ob.name = name
    ob.data.materials.append(mat)
    core.shade_flat(ob)
    return kit.keep(ob)


def build_vault_bank(kit, mats):
    cx, cy = blender_xy(-42.0, 18.0)
    sx, sy, height = 22.0, 16.0, 8.0
    box(kit, "vault.floor", (cx, cy, 0.16), (sx, sy, 0.22), mats["marble"], 0)
    wall_z = 0.2 + height / 2
    thick = 0.45
    box(kit, "vault.wall.n", (cx, cy + sy / 2, wall_z), (sx, thick, height), mats["stone"], 0)
    box(kit, "vault.wall.e", (cx + sx / 2, cy, wall_z), (thick, sy, height), mats["stone"], 0)
    box(kit, "vault.wall.w", (cx - sx / 2, cy, wall_z), (thick, sy, height), mats["stone"], 0)
    door = 3.4
    front_y = cy - sy / 2
    jamb = (sx - door) / 2
    box(kit, "vault.jamb.L", (cx - door / 2 - jamb / 2, front_y, wall_z), (jamb, thick, height), mats["stone"], 0)
    box(kit, "vault.jamb.R", (cx + door / 2 + jamb / 2, front_y, wall_z), (jamb, thick, height), mats["stone"], 0)
    box(kit, "vault.lintel", (cx, front_y, 0.2 + height - 1.3), (door, thick, 2.6), mats["stone"], 0)
    box(kit, "vault.roof", (cx, cy, 0.2 + height), (sx + 0.8, sy + 0.8, 0.35), mats["stone"], 0)
    box(kit, "vault.sign", (cx, front_y - 0.4, height - 0.4), (6.5, 0.2, 1.1), mats["gold"], 0)
    for index, x in enumerate((-6.5, -2.2, 2.2, 6.5)):
        kit.cylinder(f"vault.column.{index}", (cx + x, front_y - 1.3, height / 2), 0.42, height, mats["marble"], vertices=10)
    for step in range(3):
        box(kit, f"vault.step.{step}", (cx, front_y - 2.2 - step * 0.7, 0.12 + step * 0.08), (7.5, 0.7, 0.16), mats["stone"], 0)
    facing = (math.pi / 2, 0, 0)
    door_y = cy + sy / 2 - 1.1
    disc(kit, "vault.door", (cx, door_y, 3.2), 2.4, 0.35, mats["vaultsteel"], facing, 36)
    disc(kit, "vault.ring", (cx, door_y - 0.2, 3.2), 1.7, 0.12, mats["iron"], facing, 28)
    for bolt in range(12):
        angle = bolt * math.tau / 12
        disc(
            kit,
            f"vault.bolt.{bolt}",
            (cx + math.cos(angle) * 2.0, door_y - 0.28, 3.2 + math.sin(angle) * 2.0),
            0.1,
            0.08,
            mats["iron"],
            facing,
            8,
        )
    disc(kit, "vault.wheel", (cx, door_y - 0.35, 3.2), 0.28, 0.12, mats["gold"], facing, 12)
    for pile, px in enumerate((-3.2, 3.0)):
        index = 0
        for layer, width in enumerate((4, 3, 2)):
            for ix in range(width):
                for iy in range(width):
                    box(
                        kit,
                        f"vault.bar.{pile}.{index}",
                        (cx + px + (ix - width / 2) * 0.42, cy + (iy - width / 2) * 0.24, 0.4 + layer * 0.22),
                        (0.38, 0.2, 0.16),
                        mats["gold"] if index % 3 else mats["goldhi"],
                        0.1,
                    )
                    index += 1
    for coin in range(24):
        angle = coin * 0.55
        kit.cylinder(
            f"vault.coin.{coin}",
            (cx + math.cos(angle) * (1.2 + coin % 4 * 0.15), cy - 2 + math.sin(angle) * 0.8, 0.28 + (coin % 3) * 0.04),
            0.12,
            0.03,
            mats["gold"],
            vertices=8,
        )
    for rope, x in enumerate((-4.5, 4.5)):
        kit.cylinder(f"vault.stanchion.{rope}.a", (cx + x, cy - 1, 0.55), 0.06, 1.0, mats["gold"], vertices=6)
        kit.cylinder(f"vault.stanchion.{rope}.b", (cx + x, cy + 2, 0.55), 0.06, 1.0, mats["gold"], vertices=6)
        box(kit, f"vault.rope.{rope}", (cx + x, cy + 0.5, 0.85), (0.08, 3.2, 0.08), mats["velvet"], 0)
    for cam, x in enumerate((-7, 7)):
        box(kit, f"vault.camera.{cam}", (cx + x, cy + 4, 6.2), (0.28, 0.2, 0.16), mats["iron"], 0)
        kit.cylinder(f"vault.led.{cam}", (cx + x, cy + 3.8, 6.05), 0.04, 0.04, mats["alarm"], vertices=6)
    for beam, z in enumerate((1.1, 1.7, 2.3)):
        box(kit, f"vault.laser.{beam}", (cx, cy, z), (sx * 0.7, 0.03, 0.03), mats["alarm"], 0.2)
    kit.cylinder("vault.chandelier", (cx, cy - 1, 6.6), 0.9, 0.08, mats["gold"], vertices=12)
    for bulb in range(6):
        angle = bulb * math.tau / 6
        kit.ico(f"vault.bulb.{bulb}", (cx + math.cos(angle) * 0.7, cy - 1 + math.sin(angle) * 0.7, 6.35), 0.12, mats["amber"])


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
        "stone": core.material("stone", "#d7d1c6", metallic=0.05, roughness=0.62),
        "marble": core.material("marble", "#efeae2", metallic=0.08, roughness=0.32),
        "vaultsteel": core.material("vaultsteel", "#4a515a", metallic=0.88, roughness=0.32),
        "gold": core.material("gold", "#f2b227", "#fff1a8", 0.35, 0.92, 0.22),
        "goldhi": core.material("goldhi", "#fff1a8", "#fff6c8", 0.45, 0.8, 0.18),
        "velvet": core.material("velvet", "#7a1f33", metallic=0.0, roughness=0.8),
        "alarm": core.material("alarm", "#ff3355", "#ff3355", 3.2, 0.05, 0.25),
    }
    build_terrain(kit, mats)
    if ART_STEP >= 3:
        build_roads(kit, mats)
        build_trees(kit, mats)
    if ART_STEP >= 4:
        build_vault_bank(kit, mats)
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
