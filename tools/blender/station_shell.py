"""Octagonal station shell. Blender is Z-up. layout.json is glTF Y-up.

Rooms are placed so a camera in the +X/+Z quadrant (yaw 45, pitch 50) sees
the vault on the left, the stadium far, the market on the right, and the
war room near. Locked rooms sit top-right and bottom-left of that view.
The forge stays in the middle.
"""

import json
import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import build_station as core

ART_STEP = 3

# three.js (x, z), radius in meters. Blender y is -z.
ROOMS = [
    ("vault-quarter", "Vault", "vault", -12.0, 8.0, 5.6),
    ("cup-stadium", "Stadium", "stadium", -8.0, -12.0, 5.6),
    ("trading-village", "Trading Market", "market", 12.0, -8.0, 5.6),
    ("war-room", "War Room", "war", 8.0, 12.0, 5.6),
    ("skin-forge", "Skin Forge", "forge", 0.0, 0.0, 4.0),
    ("launch-pier", "Launch Pier", "pier", 1.6, -18.4, 3.7),
    ("card-club", "Card Club", "club", -1.6, 18.4, 3.7),
    ("oracle-plaza", "Oracle Plaza", "oracle", -18.4, -1.6, 3.7),
]

INLAY = {
    "vault": "#6a4a22",
    "stadium": "#1c4a48",
    "market": "#4a2a58",
    "war": "#1a3a48",
    "forge": "#6a3018",
    "pier": "#3a342c",
    "club": "#3a342c",
    "oracle": "#3a342c",
}

CAM = (1.0, -1.0)
TEX = os.path.join(core.ROOT, "apps", "web", "public", "textures")


def blender_xy(tx, tz):
    return tx, -tz


def box(kit, name, loc, size, mat, rot_z=0.0, bevel_width=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc)
    ob = bpy.context.active_object
    ob.name = name
    ob.scale = size
    ob.rotation_euler = (0, 0, rot_z)
    core.apply_xform(ob)
    ob.data.materials.append(mat)
    core.bevel(ob, bevel_width)
    return kit.keep(ob)


def textured(name, albedo, roughness, metal, rough, uv_scale):
    mat = core.material(name, "#ffffff", metallic=metal, roughness=rough)
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
    if roughness and os.path.exists(roughness):
        rough_node = nodes.new("ShaderNodeTexImage")
        rough_node.image = bpy.data.images.load(roughness)
        rough_node.image.colorspace_settings.name = "Non-Color"
        links.new(mapping.outputs["Vector"], rough_node.inputs["Vector"])
        links.new(rough_node.outputs["Color"], nodes["Principled BSDF"].inputs["Roughness"])
    return mat


def oct_verts(cx, cy, radius):
    verts = []
    for index in range(8):
        angle = math.radians(22.5 + index * 45)
        verts.append((cx + math.cos(angle) * radius, cy + math.sin(angle) * radius))
    return verts


def build_room(kit, mats, district, prefix, cx, cy, radius):
    kit.cylinder(
        f"{prefix}.floor",
        (cx, cy, 0.07),
        radius * 0.98,
        0.1,
        mats["steel"],
        vertices=8,
        rot=(0, 0, math.radians(22.5)),
    )
    kit.cylinder(
        f"{prefix}.inlay",
        (cx, cy, 0.13),
        radius * 0.42,
        0.03,
        mats[f"inlay.{prefix}"],
        vertices=8,
        rot=(0, 0, math.radians(22.5)),
    )
    verts = oct_verts(cx, cy, radius)
    edges = []
    for index in range(8):
        x1, y1 = verts[index]
        x2, y2 = verts[(index + 1) % 8]
        mx, my = (x1 + x2) / 2, (y1 + y2) / 2
        dx, dy = mx - cx, my - cy
        length = math.hypot(dx, dy) or 1.0
        edges.append((index, x1, y1, x2, y2, mx, my, dx / length, dy / length))
    cut = max(edges, key=lambda edge: edge[7] * CAM[0] + edge[8] * CAM[1])[0]
    door = None
    origin = math.hypot(cx, cy)
    if origin > radius + 1:
        ux, uy = -cx / origin, -cy / origin
        door = max(edges, key=lambda edge: edge[7] * ux + edge[8] * uy)[0]
        if door == cut:
            door = None
    for index, x1, y1, x2, y2, mx, my, ox, oy in edges:
        span = math.hypot(x2 - x1, y2 - y1)
        angle = math.atan2(y2 - y1, x2 - x1)
        px, py = mx - ox * 0.28, my - oy * 0.28
        if index == cut:
            box(kit, f"{prefix}.sill.{index}", (px, py, 0.16), (span * 0.96, 0.6, 0.16), mats["wall"], angle, 0.01)
            continue
        if index == door:
            gap = 2.4
            jamb = max(0.2, (span - gap) / 2)
            ax, ay = (x2 - x1) / span, (y2 - y1) / span
            for side, sign in (("L", -1), ("R", 1)):
                jx = px + ax * sign * (gap / 2 + jamb / 2)
                jy = py + ay * sign * (gap / 2 + jamb / 2)
                box(kit, f"{prefix}.jamb.{index}.{side}", (jx, jy, 1.28), (jamb, 0.6, 2.4), mats["wall"], angle, 0.015)
            box(kit, f"{prefix}.lintel.{index}", (px, py, 2.22), (gap, 0.6, 0.36), mats["wall"], angle, 0.01)
            box(
                kit,
                f"{prefix}.hazard.{index}",
                (px - ox * 0.55, py - oy * 0.55, 0.15),
                (2.2, 0.28, 0.03),
                mats["hazard"],
                angle,
                0.0,
            )
            continue
        box(kit, f"{prefix}.wall.{index}", (px, py, 1.28), (span * 0.98, 0.6, 2.4), mats["wall"], angle, 0.02)
        box(
            kit,
            f"{prefix}.panel.{index}",
            (px - ox * 0.32, py - oy * 0.32, 1.4),
            (span * 0.55, 0.05, 1.15),
            mats["trim"],
            angle,
            0.0,
        )
        box(
            kit,
            f"{prefix}.light.{index}",
            (px - ox * 0.36, py - oy * 0.36, 2.15),
            (0.34, 0.06, 0.1),
            mats["amber"],
            angle,
            0.0,
        )
        if index % 2 == 0:
            box(
                kit,
                f"{prefix}.vent.{index}",
                (px - ox * 0.36, py - oy * 0.36, 0.32),
                (0.5, 0.06, 0.18),
                mats["vent"],
                angle,
                0.0,
            )
    for index, (x, y) in enumerate(verts):
        ox, oy = x - cx, y - cy
        length = math.hypot(ox, oy) or 1.0
        ox, oy = ox / length, oy / length
        kit.cylinder(
            f"{prefix}.pipe.{index}",
            (x - ox * 0.2, y - oy * 0.2, 1.2),
            0.055,
            2.25,
            mats["trim"],
            vertices=6,
        )
    kit.marker(f"marker.{district}", (cx, cy, 0.3), district)


def corridor(kit, mats, prefix, cx, cy, radius):
    dist = math.hypot(cx, cy)
    if dist < radius:
        return
    ux, uy = cx / dist, cy / dist
    inner = 4.3
    outer = dist - radius + 0.15
    if outer - inner < 0.6:
        return
    length = outer - inner
    mid = (inner + outer) / 2
    box(
        kit,
        f"{prefix}.path",
        (ux * mid, uy * mid, 0.11),
        (length, 3.0, 0.05),
        mats["grate"],
        math.atan2(uy, ux),
        0.0,
    )


def build_station():
    core.reset_scene()
    kit = core.Kit()
    steel = textured(
        "steel",
        os.path.join(TEX, "steel-albedo.png"),
        os.path.join(TEX, "steel-roughness.png"),
        0.62,
        0.58,
        0.5,
    )
    grate = textured("grate", os.path.join(TEX, "grate-albedo.png"), None, 0.7, 0.48, 1.2)
    mats = {
        "steel": steel,
        "grate": grate,
        "wall": core.material("wall", "#2a2e33", metallic=0.58, roughness=0.64),
        "trim": core.material("trim", "#6e757c", metallic=0.7, roughness=0.4),
        "amber": core.material("amber", "#ffb347", "#ffb347", 4.0, 0.05, 0.35),
        "vent": core.material("vent", "#ff6a1a", "#ff6a1a", 2.4, 0.1, 0.4),
        "hazard": core.material("hazard", "#d8692a", "#ff8a3d", 0.7, 0.2, 0.45),
    }
    for prefix, color in INLAY.items():
        mats[f"inlay.{prefix}"] = core.material(f"inlay.{prefix}", color, color, 0.18, 0.4, 0.5)

    kit.cylinder("deck.plate", (0, 0, -0.02), 30, 0.18, steel, vertices=20)
    rooms = []
    for district, name, prefix, tx, tz, radius in ROOMS:
        cx, cy = blender_xy(tx, tz)
        build_room(kit, mats, district, prefix, cx, cy, radius)
        corridor(kit, mats, prefix, cx, cy, radius)
        rooms.append(
            {
                "id": district,
                "name": name,
                "position": core.gltf_position(cx, cy, 0),
                "size": [round(radius * 2, 3), core.WALL_H, round(radius * 2, 3)],
            }
        )

    os.makedirs(core.OUT, exist_ok=True)
    core.export_glb(kit.objects, os.path.join(core.OUT, "station.glb"), animations=False)
    layout = {"units": "gltf-y-up-meters", "artStep": ART_STEP, "rooms": rooms}
    with open(os.path.join(core.OUT, "layout.json"), "w", encoding="utf-8") as handle:
        json.dump(layout, handle, indent=2)
        handle.write("\n")
    print(f"station shell step {ART_STEP} objects {len(kit.objects)}")
