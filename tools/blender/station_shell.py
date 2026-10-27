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

ART_STEP = 5

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
    if prefix == "vault" and ART_STEP >= 4:
        dress_vault(kit, mats, cx, cy, radius, edges)
    if prefix == "market" and ART_STEP >= 5:
        dress_market(kit, mats, cx, cy, radius, edges)
    if prefix == "stadium" and ART_STEP >= 6:
        dress_stadium(kit, mats, cx, cy, radius, edges)
    if prefix == "war" and ART_STEP >= 7:
        dress_war(kit, mats, cx, cy, radius, edges)
    if prefix == "forge" and ART_STEP >= 8:
        dress_forge(kit, mats, cx, cy, radius)
    if prefix in ("pier", "club", "oracle") and ART_STEP >= 8:
        dress_locked(kit, mats, prefix, cx, cy)
    kit.marker(f"marker.{district}", (cx, cy, 0.3), district)


def place_disc(name, loc, radius, depth, mat, rot, verts=24):
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts, radius=radius, depth=depth, location=loc, rotation=rot)
    ob = bpy.context.active_object
    ob.name = name
    ob.data.materials.append(mat)
    core.shade_flat(ob)
    return ob


def dress_vault(kit, mats, cx, cy, radius, edges):
    back = max(edges, key=lambda edge: -(edge[7] * CAM[0] + edge[8] * CAM[1]))
    _, _, _, _, _, mx, my, ox, oy = back
    dx, dy = mx - ox * 0.85, my - oy * 0.85
    facing = (math.pi / 2, 0, math.atan2(oy, ox))
    door = place_disc("vault.door", (dx, dy, 1.35), 1.55, 0.22, mats["vaultsteel"], facing, 40)
    kit.keep(door)
    place_disc("vault.ring", (dx - ox * 0.12, dy - oy * 0.12, 1.35), 1.15, 0.08, mats["trim"], facing, 32)
    kit.objects.append(bpy.context.active_object)
    for bolt in range(12):
        angle = bolt * math.tau / 12
        # offset in the door plane: door right and up
        rx, ry = -oy, ox
        bx = dx + math.cos(angle) * 1.28 * rx
        by = dy + math.cos(angle) * 1.28 * ry
        bz = 1.35 + math.sin(angle) * 1.28
        bolt_ob = place_disc(f"vault.bolt.{bolt}", (bx, by, bz), 0.07, 0.08, mats["trim"], facing, 8)
        kit.keep(bolt_ob)
    hub = place_disc("vault.wheel", (dx - ox * 0.2, dy - oy * 0.2, 1.35), 0.18, 0.1, mats["gold"], facing, 12)
    kit.keep(hub)
    for spoke in range(3):
        angle = spoke * math.tau / 3
        sx = dx - ox * 0.22 + math.cos(angle) * 0.42 * (-oy)
        sy = dy - oy * 0.22 + math.cos(angle) * 0.42 * ox
        sz = 1.35 + math.sin(angle) * 0.42
        bar = box(
            kit,
            f"vault.spoke.{spoke}",
            (sx, sy, sz),
            (0.62, 0.06, 0.08),
            mats["gold"],
            math.atan2(oy, ox) + angle,
            0.0,
        )
        bar.parent = hub
        bar.matrix_parent_inverse = hub.matrix_world.inverted()
    # Two ingot piles and a coin scatter, toward the room center.
    for pile, (px, py) in enumerate(((cx - 1.3, cy - 0.4), (cx + 1.1, cy + 0.8))):
        index = 0
        for layer, width in enumerate((4, 3, 2)):
            for ix in range(width):
                for iy in range(width):
                    if index >= 20:
                        break
                    box(
                        kit,
                        f"vault.bar.{pile}.{index}",
                        (px + (ix - width / 2) * 0.38, py + (iy - width / 2) * 0.22, 0.22 + layer * 0.2),
                        (0.34, 0.18, 0.16),
                        mats["gold"] if index % 3 else mats["goldhi"],
                        0.15 * pile,
                        0.01,
                    )
                    index += 1
    for coin in range(28):
        angle = coin * 0.7
        rad = 0.4 + (coin % 5) * 0.18
        kit.cylinder(
            f"vault.coin.{coin}",
            (cx + 0.2 + math.cos(angle) * rad, cy + 1.5 + math.sin(angle) * rad * 0.6, 0.16 + (coin % 4) * 0.035),
            0.1,
            0.03,
            mats["gold"],
            vertices=10,
        )
    box(kit, "vault.crate", (cx + 2.2, cy - 1.4, 0.38), (0.7, 0.55, 0.55), mats["crate"], 0.2, 0.02)
    for cam, (oxs, oys) in enumerate(((1.6, -1.2), (-1.8, 1.1))):
        box(kit, f"vault.camera.{cam}", (cx + oxs, cy + oys, 2.15), (0.22, 0.16, 0.14), mats["trim"], 0.4, 0.0)
        kit.cylinder(f"vault.led.{cam}", (cx + oxs, cy + oys - 0.12, 2.05), 0.035, 0.04, mats["alarm"], vertices=8)
    for beam, height in enumerate((0.55, 0.95, 1.35)):
        box(kit, f"vault.laser.{beam}", (cx, cy, height), (radius * 1.3, 0.025, 0.025), mats["alarm"], 0.4, 0.0)


def back_wall(edges):
    return max(edges, key=lambda edge: -(edge[7] * CAM[0] + edge[8] * CAM[1]))


def dress_market(kit, mats, cx, cy, radius, edges):
    _, _, _, _, _, mx, my, ox, oy = back_wall(edges)
    box(kit, "market.screen", (mx - ox * 0.7, my - oy * 0.7, 1.45), (2.6, 0.08, 1.5), mats["screen"], math.atan2(oy, ox), 0)
    for index in range(8):
        angle = -2.4 + index * 0.55
        x = cx + math.cos(angle) * radius * 0.48
        y = cy + math.sin(angle) * radius * 0.48
        box(kit, f"market.counter.{index}", (x, y, 0.48), (1.15, 0.55, 0.72), mats["wood"], angle, 0.02)
        box(kit, f"market.awning.{index}", (x, y, 1.15), (1.35, 0.7, 0.08), mats[f"awning.{index}"], angle, 0)
        box(kit, f"market.stripe.{index}", (x, y, 1.2), (0.22, 0.72, 0.05), mats["cloth"], angle, 0)
        box(kit, f"market.good.{index}", (x, y, 0.95), (0.22, 0.18, 0.18), mats[f"awning.{(index + 3) % 8}"], angle, 0.01)
        kit.cylinder(f"market.lantern.{index}", (x + 0.4, y, 1.35), 0.06, 0.16, mats["amber"], vertices=6)
    for index, (oxs, oys) in enumerate(((1.4, 0.2), (-0.8, 1.3), (0.4, -1.6))):
        kit.cylinder(f"market.barrel.{index}", (cx + oxs, cy + oys, 0.32), 0.22, 0.55, mats["wood"], vertices=8)
    box(kit, "market.crate", (cx - 1.6, cy + 0.4, 0.28), (0.5, 0.4, 0.4), mats["crate"], 0.3, 0.01)
    kit.cylinder("market.lamp", (cx + 0.2, cy - 0.2, 0.7), 0.08, 0.9, mats["amber"], vertices=6)


def dress_stadium(kit, mats, cx, cy, radius, edges):
    _, _, _, _, _, mx, my, ox, oy = back_wall(edges)
    box(kit, "stadium.screen", (mx - ox * 0.55, my - oy * 0.55, 1.7), (3.2, 0.08, 1.6), mats["screen"], math.atan2(oy, ox), 0)
    box(kit, "stadium.ring", (cx, cy, 0.16), (radius * 0.7, radius * 0.7, 0.04), mats["hazard"], 0.2, 0)
    kit.cylinder("stadium.emblem", (cx, cy, 0.2), 0.7, 0.04, mats["gold"], vertices=8)
    for side, sign in enumerate((-1, 1)):
        for step in range(5):
            box(
                kit,
                f"stadium.seat.{side}.{step}",
                (cx + ox * sign * (1.2 + step * 0.45), cy + oy * sign * (1.2 + step * 0.45), 0.28 + step * 0.22),
                (3.4, 0.4, 0.18 + step * 0.08),
                mats["trim"] if step % 2 == 0 else mats["wall"],
                math.atan2(oy, ox),
                0.01,
            )
    for spot in range(3):
        kit.cylinder(f"stadium.spot.{spot}", (cx - 1.2 + spot * 1.2, cy, 2.35), 0.12, 0.08, mats["cloth"], vertices=8)


def dress_war(kit, mats, cx, cy, radius, edges):
    _, _, _, _, _, mx, my, ox, oy = back_wall(edges)
    box(kit, "war.map", (mx - ox * 0.55, my - oy * 0.55, 1.55), (2.8, 0.06, 1.5), mats["cyan"], math.atan2(oy, ox), 0)
    for index in range(6):
        angle = index * math.tau / 6
        x = cx + math.cos(angle) * radius * 0.48
        y = cy + math.sin(angle) * radius * 0.48
        box(kit, f"war.desk.{index}", (x, y, 0.42), (0.9, 0.5, 0.6), mats["wall"], angle, 0.01)
        box(kit, f"war.screen.{index}", (x, y, 0.95), (0.7, 0.05, 0.42), mats["cyan"], angle, 0)
    kit.cylinder("war.table", (cx, cy, 0.45), 0.7, 0.12, mats["trim"], vertices=16)
    kit.cylinder("war.holo", (cx, cy, 0.9), 0.28, 0.55, mats["cyan"], vertices=8)
    kit.cylinder("war.beacon", (cx, cy, 2.25), 0.12, 0.2, mats["alarm"], vertices=8)


def dress_forge(kit, mats, cx, cy, radius):
    box(kit, "forge.anvil", (cx - 1.1, cy + 0.4, 0.45), (0.7, 0.4, 0.55), mats["trim"], 0.2, 0.02)
    box(kit, "forge.furnace", (cx + 1.2, cy - 0.3, 0.7), (1.1, 0.8, 1.2), mats["wall"], 0.1, 0.02)
    box(kit, "forge.mouth", (cx + 1.2, cy - 0.72, 0.55), (0.45, 0.08, 0.38), mats["molten"], 0.1, 0)
    kit.cylinder("forge.pedestal", (cx, cy + 0.2, 0.28), 0.45, 0.4, mats["trim"], vertices=8)
    for index, height in enumerate((1.6, 1.9, 2.15)):
        box(kit, f"forge.tool.{index}", (cx - 0.2 + index * 0.25, cy - 1.4, height), (0.08, 0.08, 0.45), mats["trim"], 0.4, 0)


def dress_locked(kit, mats, prefix, cx, cy):
    kit.torus(f"{prefix}.lock", (cx, cy, 1.15), 0.42, 0.07, mats["trim"], rot=(math.pi / 2, 0, 0))
    box(kit, f"{prefix}.lockbody", (cx, cy, 0.72), (0.48, 0.18, 0.55), mats["trim"], 0.2, 0.02)
    box(kit, f"{prefix}.plaque", (cx, cy - 0.9, 0.7), (1.4, 0.08, 0.4), mats["wood"], 0.1, 0.01)


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
        "vaultsteel": core.material("vaultsteel", "#4a515a", metallic=0.88, roughness=0.32),
        "gold": core.material("gold", "#f2b227", "#fff1a8", 0.35, 0.92, 0.22),
        "goldhi": core.material("goldhi", "#fff1a8", "#fff6c8", 0.45, 0.8, 0.18),
        "crate": core.material("crate", "#d8692a", metallic=0.15, roughness=0.62),
        "alarm": core.material("alarm", "#ff3355", "#ff3355", 3.2, 0.05, 0.25),
        "wood": core.material("wood", "#8a5a32", metallic=0.02, roughness=0.72),
        "cloth": core.material("cloth", "#f4efe6", metallic=0.0, roughness=0.85),
        "glass": core.material("glass", "#102028", "#7dffc3", 0.6, 0.05, 0.12),
        "cyan": core.material("cyan", "#14343a", "#2fe6e6", 1.4, 0.1, 0.2),
        "molten": core.material("molten", "#ff6a1a", "#ff6a1a", 4.0, 0.15, 0.25),
        "screen": core.material("screen", "#101418", "#1c3a32", 0.35, 0.05, 0.18),
    }
    awnings = ["#d64545", "#2a9d8f", "#7a4fc4", "#3d8c4a", "#d64545", "#2a9d8f", "#e0a106", "#7a4fc4"]
    for index, color in enumerate(awnings):
        mats[f"awning.{index}"] = core.material(f"awning.{index}", color, metallic=0.02, roughness=0.78)
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
