# Original fe2VI station and agent meshes.
#
# Run from the repo root:
#   blender --background --python tools/blender/build_station.py
#
# Writes apps/web/public/models/station.glb, agent.glb, and layout.json.
# Blender is Z-up. The glTF files are Y-up. layout.json uses glTF meters.
# Nothing here is traced from a character pack. The body is a beveled ingot.

import json
import math
import os
import sys

import bpy
import bmesh

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "apps", "web", "public", "models")

WALL_H = 2.45
WALL_T = 0.28
DOOR_W = 1.45
DOOR_H = 1.85

# Role, hat, and accessory indexes match crates/fe2vi-core/src/skin.rs.
# Hats:     0 none, 1 beanie, 2 visor, 3 hood, 4 hardhat, 5 antenna
# Roles:    0 safecracker, 1 trader, 2 hacker, 3 auditor, 4 builder, 5 scout
# Accessory:0 none, 1 headset, 2 magnifier, 3 goggles, 4 satchel, 5 bolt
# The satchel sits on the hip. It is a tool kit, not a body-sized pack.


def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.scene.render.fps = 24


def rgba(hex_color, alpha=1.0):
    hex_color = hex_color.lstrip("#")
    return (
        int(hex_color[0:2], 16) / 255,
        int(hex_color[2:4], 16) / 255,
        int(hex_color[4:6], 16) / 255,
        alpha,
    )


def material(name, color, emission=None, strength=0.0, metallic=0.35, roughness=0.45):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    mat.use_backface_culling = False
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = rgba(color)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    if emission:
        bsdf.inputs["Emission Color"].default_value = rgba(emission)
        bsdf.inputs["Emission Strength"].default_value = strength
    return mat


def shade_flat(ob):
    for poly in ob.data.polygons:
        poly.use_smooth = False


def apply_modifiers(ob):
    if not ob.modifiers:
        shade_flat(ob)
        return
    depsgraph = bpy.context.evaluated_depsgraph_get()
    baked = bpy.data.meshes.new_from_object(ob.evaluated_get(depsgraph))
    old = ob.data
    ob.modifiers.clear()
    ob.data = baked
    if old.users == 0:
        bpy.data.meshes.remove(old)
    shade_flat(ob)


def select_only(ob):
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)


def apply_xform(ob, location=False, rotation=True, scale=True):
    select_only(ob)
    bpy.ops.object.transform_apply(location=location, rotation=rotation, scale=scale)


def bevel(ob, width):
    if width <= 0:
        shade_flat(ob)
        return
    mod = ob.modifiers.new("Bevel", "BEVEL")
    mod.width = width
    mod.segments = 1
    mod.limit_method = "ANGLE"
    mod.angle_limit = math.radians(45)
    mod.use_clamp_overlap = True
    apply_modifiers(ob)


def link(ob):
    if ob.name not in bpy.context.scene.collection.objects:
        bpy.context.scene.collection.objects.link(ob)
    return ob


class Kit:
    def __init__(self):
        self.objects = []

    def keep(self, ob):
        self.objects.append(ob)
        return ob

    def cube(self, name, loc, size, mat, bevel_width=0.035, rot=(0, 0, 0)):
        bpy.ops.mesh.primitive_cube_add(size=1, location=loc, rotation=rot)
        ob = bpy.context.active_object
        ob.name = name
        ob.dimensions = size
        apply_xform(ob)
        ob.data.materials.append(mat)
        bevel(ob, bevel_width)
        return self.keep(ob)

    def cylinder(self, name, loc, radius, depth, mat, vertices=16, rot=(0, 0, 0), bevel_width=0.0):
        bpy.ops.mesh.primitive_cylinder_add(
            vertices=vertices, radius=radius, depth=depth, location=loc, rotation=rot
        )
        ob = bpy.context.active_object
        ob.name = name
        apply_xform(ob)
        ob.data.materials.append(mat)
        bevel(ob, bevel_width)
        return self.keep(ob)

    def cone(self, name, loc, radius, depth, mat, vertices=4, rot=(0, 0, 0)):
        bpy.ops.mesh.primitive_cone_add(
            vertices=vertices, radius1=radius, radius2=0, depth=depth, location=loc, rotation=rot
        )
        ob = bpy.context.active_object
        ob.name = name
        apply_xform(ob)
        ob.data.materials.append(mat)
        shade_flat(ob)
        return self.keep(ob)

    def torus(self, name, loc, major, minor, mat, rot=(0, 0, 0)):
        bpy.ops.mesh.primitive_torus_add(
            major_radius=major,
            minor_radius=minor,
            major_segments=12,
            minor_segments=6,
            location=loc,
            rotation=rot,
        )
        ob = bpy.context.active_object
        ob.name = name
        apply_xform(ob)
        ob.data.materials.append(mat)
        shade_flat(ob)
        return self.keep(ob)

    def ico(self, name, loc, radius, mat):
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=radius, location=loc)
        ob = bpy.context.active_object
        ob.name = name
        ob.data.materials.append(mat)
        shade_flat(ob)
        return self.keep(ob)

    def marker(self, name, loc, district):
        ob = bpy.data.objects.new(name, None)
        ob.empty_display_type = "PLAIN_AXES"
        ob.empty_display_size = 0.4
        ob.location = loc
        ob["district"] = district
        link(ob)
        return self.keep(ob)


def mats():
    return {
        "iron": material("iron", "#2a313a", metallic=0.72, roughness=0.38),
        "iron_dark": material("iron_dark", "#12161c", metallic=0.55, roughness=0.5),
        "iron_trim": material("iron_trim", "#8a939c", metallic=0.8, roughness=0.28),
        "floor": material("floor", "#1a1f27", metallic=0.25, roughness=0.7),
        "floor_line": material("floor_line", "#243038", metallic=0.2, roughness=0.6),
        "orange": material("orange", "#ff7a1a", "#ff7a1a", 1.4, 0.15, 0.35),
        "teal": material("teal", "#1ec8c0", "#2ee6d6", 1.6, 0.1, 0.3),
        "gold": material("gold", "#f0c14a", "#f0c14a", 0.35, 0.85, 0.25),
        "alarm": material("alarm", "#ef476f", "#ff3355", 2.4, 0.05, 0.2),
        "core": material("core", "#ffe27a", "#ffe27a", 2.2, 0.0, 0.15),
        "body": material("Body", "#8a939c", metallic=0.66, roughness=0.32),
        "cloth": material("cloth", "#3d4654", metallic=0.05, roughness=0.8),
        "glass": material("glass", "#14343a", "#2ee6d6", 0.8, 0.0, 0.08),
        "molten": material("molten", "#ff5a1f", "#ff7a1a", 3.0, 0.1, 0.2),
    }


def gltf_position(x, y, z):
    # Blender (x, y, z) Z-up becomes glTF (x, z, -y) Y-up.
    return [round(x, 3), round(z, 3), round(-y, 3)]


def wall_along_y(kit, name, x, y0, y1, mat, door=False):
    height = WALL_H
    thick = WALL_T
    z = height / 2
    if not door:
        kit.cube(name, (x, (y0 + y1) / 2, z), (thick, y1 - y0, height), mat, 0.04)
        return
    gap = DOOR_W
    mid = (y0 + y1) / 2
    left1, left0 = mid - gap / 2, y0
    right0, right1 = mid + gap / 2, y1
    if left1 - left0 > 0.05:
        kit.cube(f"{name}.jambL", (x, (left0 + left1) / 2, z), (thick, left1 - left0, height), mat, 0.03)
    if right1 - right0 > 0.05:
        kit.cube(f"{name}.jambR", (x, (right0 + right1) / 2, z), (thick, right1 - right0, height), mat, 0.03)
    lintel_h = height - DOOR_H
    kit.cube(
        f"{name}.lintel",
        (x, mid, DOOR_H + lintel_h / 2),
        (thick, gap, lintel_h),
        mat,
        0.02,
    )
    kit.cube(f"{name}.postL", (x, mid - gap / 2, DOOR_H / 2), (thick + 0.08, 0.16, DOOR_H), mat, 0.02)
    kit.cube(f"{name}.postR", (x, mid + gap / 2, DOOR_H / 2), (thick + 0.08, 0.16, DOOR_H), mat, 0.02)


def wall_along_x(kit, name, y, x0, x1, mat, door=False):
    height = WALL_H
    thick = WALL_T
    z = height / 2
    if not door:
        kit.cube(name, ((x0 + x1) / 2, y, z), (x1 - x0, thick, height), mat, 0.04)
        return
    gap = DOOR_W
    mid = (x0 + x1) / 2
    left0, left1 = x0, mid - gap / 2
    right0, right1 = mid + gap / 2, x1
    if left1 - left0 > 0.05:
        kit.cube(f"{name}.jambL", ((left0 + left1) / 2, y, z), (left1 - left0, thick, height), mat, 0.03)
    if right1 - right0 > 0.05:
        kit.cube(f"{name}.jambR", ((right0 + right1) / 2, y, z), (right1 - right0, thick, height), mat, 0.03)
    lintel_h = height - DOOR_H
    kit.cube(
        f"{name}.lintel",
        (mid, y, DOOR_H + lintel_h / 2),
        (gap, thick, lintel_h),
        mat,
        0.02,
    )
    kit.cube(f"{name}.postL", (mid - gap / 2, y, DOOR_H / 2), (0.16, thick + 0.08, DOOR_H), mat, 0.02)
    kit.cube(f"{name}.postR", (mid + gap / 2, y, DOOR_H / 2), (0.16, thick + 0.08, DOOR_H), mat, 0.02)


def corner_posts(kit, prefix, x0, x1, y0, y1, mat):
    for index, (x, y) in enumerate(((x0, y0), (x0, y1), (x1, y0), (x1, y1))):
        kit.cube(
            f"{prefix}.post{index}",
            (x, y, WALL_H / 2),
            (0.42, 0.42, WALL_H + 0.08),
            mat,
            0.04,
        )


def bars(kit, name, loc, axis, mat):
    """Iron bars across a doorway. axis is 'x' or 'y' for the wall normal."""
    x, y, _z = loc
    for i, h in enumerate((0.45, 0.9, 1.35)):
        if axis == "y":
            kit.cube(f"{name}.bar{i}", (x, y, h), (DOOR_W - 0.15, 0.06, 0.06), mat, 0.0)
        else:
            kit.cube(f"{name}.bar{i}", (x, y, h), (0.06, DOOR_W - 0.15, 0.06), mat, 0.0)
    kit.cube(f"{name}.stile", (x, y, DOOR_H / 2), (0.07, 0.07, DOOR_H), mat, 0.0)


def padlock(kit, name, loc, mat, gold):
    x, y, z = loc
    kit.cube(f"{name}.body", (x, y, z), (0.28, 0.16, 0.32), mat, 0.03)
    kit.torus(f"{name}.shackle", (x, y, z + 0.2), 0.11, 0.028, gold, rot=(math.pi / 2, 0, 0))


def room_shell(kit, prefix, cx, cy, w, d, m, doors, locked=False):
    x0, x1 = cx - w / 2, cx + w / 2
    y0, y1 = cy - d / 2, cy + d / 2
    kit.cube(f"{prefix}.floor", (cx, cy, 0.04), (w - 0.2, d - 0.2, 0.08), m["floor"], 0.02)
    kit.cube(f"{prefix}.rug", (cx, cy, 0.085), (w * 0.55, d * 0.45, 0.02), m["floor_line"], 0.0)
    wall_along_x(kit, f"{prefix}.north", y1, x0, x1, m["iron"], door="n" in doors)
    wall_along_x(kit, f"{prefix}.south", y0, x0, x1, m["iron"], door="s" in doors)
    wall_along_y(kit, f"{prefix}.west", x0, y0, y1, m["iron"], door="w" in doors)
    wall_along_y(kit, f"{prefix}.east", x1, y0, y1, m["iron"], door="e" in doors)
    corner_posts(kit, prefix, x0, x1, y0, y1, m["iron_trim"])
    # A cap band so the wall reads thick from above without hiding the room.
    kit.cube(f"{prefix}.lipN", (cx, y1, WALL_H + 0.04), (w, 0.36, 0.1), m["iron_trim"], 0.0)
    kit.cube(f"{prefix}.lipS", (cx, y0, WALL_H + 0.04), (w, 0.36, 0.1), m["iron_trim"], 0.0)
    if locked:
        if "s" in doors:
            bars(kit, f"{prefix}.bars", (cx, y0, 0), "y", m["iron_trim"])
            padlock(kit, f"{prefix}.lock", (cx, y0 - 0.35, 1.15), m["iron_dark"], m["gold"])
        elif "n" in doors:
            bars(kit, f"{prefix}.bars", (cx, y1, 0), "y", m["iron_trim"])
            padlock(kit, f"{prefix}.lock", (cx, y1 + 0.35, 1.15), m["iron_dark"], m["gold"])
        elif "e" in doors:
            bars(kit, f"{prefix}.bars", (x1, cy, 0), "x", m["iron_trim"])
            padlock(kit, f"{prefix}.lock", (x1 + 0.35, cy, 1.15), m["iron_dark"], m["gold"])
        elif "w" in doors:
            bars(kit, f"{prefix}.bars", (x0, cy, 0), "x", m["iron_trim"])
            padlock(kit, f"{prefix}.lock", (x0 - 0.35, cy, 1.15), m["iron_dark"], m["gold"])
    return (x0, x1, y0, y1)


def gold_stack(kit, name, loc, m, layers=3):
    x, y, z = loc
    for i in range(layers):
        kit.cube(
            f"{name}.{i}",
            (x, y, z + 0.08 + i * 0.1),
            (0.42, 0.18, 0.08),
            m["gold"],
            0.01,
        )


def build_vault(kit, m, cx, cy, w, d):
    room_shell(kit, "vault", cx, cy, w, d, m, doors={"s"})
    # Round vault door against the north wall, facing south.
    kit.cylinder(
        "vault.door",
        (cx, cy + d / 2 - 0.55, 1.2),
        1.05,
        0.34,
        m["iron_trim"],
        vertices=20,
        rot=(math.pi / 2, 0, 0),
        bevel_width=0.02,
    )
    kit.cylinder(
        "vault.doorInner",
        (cx, cy + d / 2 - 0.78, 1.2),
        0.72,
        0.08,
        m["iron_dark"],
        vertices=20,
        rot=(math.pi / 2, 0, 0),
    )
    kit.torus(
        "vault.wheel",
        (cx, cy + d / 2 - 0.9, 1.2),
        0.28,
        0.035,
        m["gold"],
        rot=(math.pi / 2, 0, 0),
    )
    for i, ang in enumerate((0, 60, 120, 180, 240, 300)):
        rad = math.radians(ang)
        kit.cylinder(
            f"vault.bolt{i}",
            (cx + math.cos(rad) * 0.82, cy + d / 2 - 0.78, 1.2 + math.sin(rad) * 0.82),
            0.06,
            0.08,
            m["gold"],
            vertices=6,
            rot=(math.pi / 2, 0, 0),
        )
    gold_stack(kit, "vault.goldA", (cx - 1.5, cy - 0.8, 0.08), m, 4)
    gold_stack(kit, "vault.goldB", (cx - 1.0, cy - 1.3, 0.08), m, 2)
    gold_stack(kit, "vault.goldC", (cx + 1.4, cy - 0.6, 0.08), m, 3)
    kit.cube("vault.lamp", (cx, cy, 0.12), (0.35, 0.35, 0.06), m["orange"], 0.0)


def build_stadium(kit, m, cx, cy, w, d):
    room_shell(kit, "stadium", cx, cy, w, d, m, doors={"s"})
    # Stepped stands on the east and west. Open middle is the floor.
    for side, sx in (("W", -1), ("E", 1)):
        for step in range(3):
            inset = 0.45 + step * 0.45
            kit.cube(
                f"stadium.stand{side}{step}",
                (cx + sx * (w / 2 - inset), cy + 0.3, 0.2 + step * 0.28),
                (0.7, d - 1.6, 0.28),
                m["iron"],
                0.02,
            )
    kit.cube(
        "stadium.board",
        (cx, cy + d / 2 - 0.4, 1.7),
        (w * 0.62, 0.12, 1.15),
        m["iron_dark"],
        0.02,
    )
    kit.cube(
        "stadium.screen",
        (cx, cy + d / 2 - 0.5, 1.7),
        (w * 0.54, 0.04, 0.95),
        m["teal"],
        0.0,
    )
    for i, xoff in enumerate((-0.9, -0.3, 0.3, 0.9)):
        kit.cube(
            f"stadium.row{i}",
            (cx + xoff, cy + d / 2 - 0.56, 1.45 + (i % 2) * 0.25),
            (0.4, 0.02, 0.12),
            m["orange"] if i == 0 else m["gold"],
            0.0,
        )
    kit.cube("stadium.stripe", (cx, cy - 0.2, 0.1), (0.18, d * 0.55, 0.02), m["orange"], 0.0)


def build_market(kit, m, cx, cy, w, d):
    room_shell(kit, "market", cx, cy, w, d, m, doors={"n"})
    colors = (m["orange"], m["teal"], m["gold"])
    for i, color in enumerate(colors):
        sx = cx - w * 0.22 + i * (w * 0.22)
        sy = cy + 0.15
        kit.cube(f"market.counter{i}", (sx, sy, 0.5), (1.35, 0.7, 0.9), m["iron"], 0.03)
        kit.cube(
            f"market.awning{i}",
            (sx, sy - 0.15, 1.55),
            (1.7, 1.05, 0.06),
            color,
            0.0,
            rot=(0.22, 0, 0),
        )
        kit.cube(f"market.crate{i}", (sx + 0.15, sy + 0.7, 0.22), (0.32, 0.32, 0.32), m["iron_dark"], 0.02)
    kit.cube(
        "market.board",
        (cx, cy - d / 2 + 0.45, 1.65),
        (w * 0.7, 0.1, 1.4),
        m["iron_dark"],
        0.02,
    )
    kit.cube(
        "market.leader",
        (cx, cy - d / 2 + 0.55, 1.65),
        (w * 0.62, 0.04, 1.2),
        m["teal"],
        0.0,
    )
    for i in range(5):
        kit.cube(
            f"market.rank{i}",
            (cx - 0.9 + (i % 3) * 0.15, cy - d / 2 + 0.6, 2.05 - i * 0.22),
            (1.6 - i * 0.18, 0.02, 0.08),
            m["orange"] if i == 0 else m["gold"],
            0.0,
        )
    # 24/7 lamp built from blocks, above the north door.
    sign_seven_segment(kit, "market.sign", (cx, cy + d / 2 - 0.5, 2.15), m["orange"])


def sign_seven_segment(kit, name, loc, mat):
    """A small 24/7 mark made of boxes, so the market reads as always open."""
    x, y, z = loc
    # Each digit is a column of segments. Scale is tiny.
    digits = ("2", "4", "7")
    cursor = x - 0.55
    for label in digits:
        draw_digit(kit, f"{name}.{label}{cursor:.2f}", (cursor, y, z), mat, label)
        cursor += 0.42
    kit.cube(f"{name}.dot", (x + 0.15, y, z + 0.02), (0.06, 0.04, 0.06), mat, 0.0)


def draw_digit(kit, name, loc, mat, digit):
    x, y, z = loc
    seg = {
        "a": (0, 0.16, 0.34, 0.06),
        "b": (0.16, 0.08, 0.06, 0.16),
        "c": (0.16, -0.08, 0.06, 0.16),
        "d": (0, -0.16, 0.34, 0.06),
        "e": (-0.16, -0.08, 0.06, 0.16),
        "f": (-0.16, 0.08, 0.06, 0.16),
        "g": (0, 0, 0.34, 0.06),
    }
    on = {
        "2": "abged",
        "4": "fgbc",
        "7": "abc",
    }[digit]
    for key in on:
        ox, oz, w, h = seg[key]
        kit.cube(f"{name}.{key}", (x + ox, y, z + oz), (w, 0.04, h), mat, 0.0)


def build_war(kit, m, cx, cy, w, d):
    room_shell(kit, "war", cx, cy, w, d, m, doors={"n"})
    for i, sx in enumerate((-1.15, 1.15)):
        kit.cube(f"war.desk{i}", (cx + sx, cy + 0.2, 0.45), (1.3, 0.7, 0.75), m["iron_dark"], 0.03)
        kit.cube(f"war.screen{i}", (cx + sx, cy + 0.55, 1.15), (0.9, 0.05, 0.5), m["teal"], 0.0)
        kit.cube(f"war.stand{i}", (cx + sx, cy + 0.5, 0.9), (0.08, 0.08, 0.35), m["iron"], 0.0)
    kit.cube("war.table", (cx, cy - 0.85, 0.42), (1.5, 0.9, 0.55), m["iron"], 0.03)
    kit.cube("war.holo", (cx, cy - 0.85, 0.74), (1.2, 0.62, 0.06), m["teal"], 0.0)
    kit.cube("war.bracket", (cx + w / 2 - 0.45, cy, 2.05), (0.12, 0.12, 0.7), m["iron_trim"], 0.0)
    kit.ico("war.alarm", (cx + w / 2 - 0.45, cy, 2.2), 0.16, m["alarm"])
    kit.cube("war.glow", (cx, cy - 0.85, 0.78), (0.35, 0.35, 0.02), m["alarm"], 0.0)


def build_forge(kit, m, cx, cy, w, d):
    room_shell(kit, "forge", cx, cy, w, d, m, doors={"n"})
    kit.cube("forge.furnace", (cx, cy + d / 2 - 0.85, 0.85), (2.2, 1.1, 1.6), m["iron_dark"], 0.04)
    kit.cube("forge.mouth", (cx, cy + d / 2 - 1.42, 0.7), (1.1, 0.08, 0.7), m["molten"], 0.0)
    kit.cube("forge.hearth", (cx, cy - 0.2, 0.12), (1.6, 0.45, 0.06), m["molten"], 0.0)
    kit.cube("forge.anvil", (cx, cy - 0.9, 0.45), (0.7, 0.35, 0.55), m["iron_trim"], 0.03)
    kit.cube("forge.base", (cx, cy - 0.9, 0.15), (0.4, 0.4, 0.3), m["iron"], 0.02)
    for i, xoff in enumerate((-0.7, -0.35, 0.0)):
        kit.cube(
            f"forge.ingot{i}",
            (cx + 1.5, cy - 0.4 + xoff, 0.2),
            (0.45, 0.18, 0.1),
            m["iron_trim"],
            0.01,
        )
    kit.cube("forge.stack", (cx - 1.5, cy - 0.5, 0.35), (0.4, 0.4, 0.55), m["iron"], 0.02)
    kit.ico("forge.spark", (cx, cy + d / 2 - 1.5, 1.15), 0.08, m["orange"])


def build_locked(kit, m, prefix, cx, cy, w, d, door):
    room_shell(kit, prefix, cx, cy, w, d, m, doors={door}, locked=True)
    kit.cube(f"{prefix}.crate", (cx, cy, 0.28), (0.7, 0.7, 0.45), m["iron_dark"], 0.03)
    kit.cube(f"{prefix}.plaque", (cx, cy + 0.6, 1.3), (0.9, 0.06, 0.4), m["iron_trim"], 0.0)


def build_concourse(kit, m, bounds):
    x0, x1, y0, y1 = bounds
    kit.cube("concourse.floor", ((x0 + x1) / 2, (y0 + y1) / 2, -0.02), (x1 - x0, y1 - y0, 0.12), m["iron_dark"], 0.0)
    kit.cube("concourse.guide", (0, 0, 0.05), (x1 - x0 - 1.5, 0.16, 0.02), m["teal"], 0.0)
    # Low curbs leave the doors clear. The concourse is the gap between the rows.
    kit.cube("concourse.curbN", (0, y1 - 0.2, 0.18), (x1 - x0, 0.18, 0.28), m["iron"], 0.02)
    kit.cube("concourse.curbS", (0, y0 + 0.2, 0.18), (x1 - x0, 0.18, 0.28), m["iron"], 0.02)


def build_observation(kit, m, cx, cy):
    w, d, rise = 20.0, 3.4, 0.7
    kit.cube("observation.deck", (cx, cy, rise / 2), (w, d, rise), m["iron"], 0.04)
    kit.cube("observation.trim", (cx, cy + d / 2 - 0.15, rise + 0.08), (w, 0.2, 0.1), m["iron_trim"], 0.0)
    posts = int(w / 1.6)
    for i in range(posts):
        px = cx - w / 2 + 0.8 + i * (w - 1.6) / max(posts - 1, 1)
        kit.cube(f"observation.post{i}", (px, cy + d / 2 - 0.15, rise + 0.45), (0.1, 0.1, 0.7), m["iron_trim"], 0.0)
    kit.cube(
        "observation.rail",
        (cx, cy + d / 2 - 0.15, rise + 0.75),
        (w - 0.4, 0.08, 0.08),
        m["orange"],
        0.0,
    )
    # Two ramps drop into the north alleys. A wedge mesh, not a scaled cube.
    for index, rx in enumerate((-4.4, 4.4)):
        wedge(kit, f"observation.ramp{index}", (rx, cy - d / 2 - 0.9, 0), (1.6, 1.8, rise), m["iron_dark"])
    kit.marker("marker.observation", (cx, cy, rise + 0.2), "observation")


def wedge(kit, name, loc, size, mat):
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    w, d, h = size
    verts = [
        bm.verts.new((-w / 2, -d / 2, 0)),
        bm.verts.new((w / 2, -d / 2, 0)),
        bm.verts.new((w / 2, d / 2, 0)),
        bm.verts.new((-w / 2, d / 2, 0)),
        bm.verts.new((w / 2, d / 2, h)),
        bm.verts.new((-w / 2, d / 2, h)),
    ]
    bm.faces.new((verts[0], verts[1], verts[2], verts[3]))
    bm.faces.new((verts[3], verts[2], verts[4], verts[5]))
    bm.faces.new((verts[0], verts[3], verts[5]))
    bm.faces.new((verts[1], verts[4], verts[2]))
    bm.faces.new((verts[0], verts[5], verts[4], verts[1]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    ob = bpy.data.objects.new(name, mesh)
    ob.location = loc
    ob.data.materials.append(mat)
    shade_flat(ob)
    link(ob)
    return kit.keep(ob)


def build_station():
    reset_scene()
    m = mats()
    kit = Kit()
    w, d = 6.2, 5.8
    xs = [-8.8, 0.0, 8.8]
    top, bot = 5.2, -5.2
    rooms = []

    def add_room(district, name, cx, cy, builder, size=(w, d)):
        builder()
        kit.marker(f"marker.{district}", (cx, cy, 0.2), district)
        rooms.append(
            {
                "id": district,
                "name": name,
                "position": gltf_position(cx, cy, 0),
                "size": [size[0], WALL_H, size[1]],
            }
        )

    add_room("vault-quarter", "Vault", xs[0], top, lambda: build_vault(kit, m, xs[0], top, w, d))
    add_room(
        "oracle-plaza",
        "Oracle Plaza",
        xs[1],
        top,
        lambda: build_locked(kit, m, "oracle", xs[1], top, w, d, "s"),
    )
    add_room("cup-stadium", "Stadium", xs[2], top, lambda: build_stadium(kit, m, xs[2], top, w, d))
    add_room("war-room", "War Room", xs[0], bot, lambda: build_war(kit, m, xs[0], bot, w, d))
    add_room("skin-forge", "Skin Forge", xs[1], bot, lambda: build_forge(kit, m, xs[1], bot, w, d))
    add_room("trading-village", "Trading Market", xs[2], bot, lambda: build_market(kit, m, xs[2], bot, w, d))

    side_w, side_d = 5.0, 4.6
    pier_x, club_x = 16.4, -16.4
    add_room(
        "launch-pier",
        "Launch Pier",
        pier_x,
        0,
        lambda: build_locked(kit, m, "pier", pier_x, 0, side_w, side_d, "w"),
        size=(side_w, side_d),
    )
    add_room(
        "card-club",
        "Card Club",
        club_x,
        0,
        lambda: build_locked(kit, m, "club", club_x, 0, side_w, side_d, "e"),
        size=(side_w, side_d),
    )

    # Concourse between the two rows, plus alley floors out to the locked wings.
    build_concourse(kit, m, (-12.2, 12.2, -2.3, 2.3))
    kit.cube("alley.west", (-14.2, 0, -0.02), (4.6, 3.2, 0.1), m["iron_dark"], 0.0)
    kit.cube("alley.east", (14.2, 0, -0.02), (4.6, 3.2, 0.1), m["iron_dark"], 0.0)
    kit.cube("alley.westLine", (-14.2, 0, 0.05), (4.0, 0.12, 0.02), m["teal"], 0.0)
    kit.cube("alley.eastLine", (14.2, 0, 0.05), (4.0, 0.12, 0.02), m["teal"], 0.0)
    build_observation(kit, m, 0, 11.6)
    rooms.append(
        {
            "id": "observation",
            "name": "Observation Deck",
            "position": gltf_position(0, 11.6, 0.7),
            "size": [20, 1.2, 3.4],
        }
    )

    os.makedirs(OUT, exist_ok=True)
    export_glb(kit.objects, os.path.join(OUT, "station.glb"), animations=False)
    layout = {"units": "gltf-y-up-meters", "rooms": rooms}
    with open(os.path.join(OUT, "layout.json"), "w", encoding="utf-8") as handle:
        json.dump(layout, handle, indent=2)
        handle.write("\n")
    print(f"station objects {len(kit.objects)}")


def agent_mesh(name, loc, size, mat, bevel_width=0.05):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc)
    ob = bpy.context.active_object
    ob.name = name
    ob.dimensions = size
    apply_xform(ob)
    ob.data.materials.append(mat)
    bevel(ob, bevel_width)
    return ob


def build_armature():
    bpy.ops.object.armature_add(enter_editmode=True, location=(0, 0, 0))
    arm = bpy.context.active_object
    arm.name = "AgentRig"
    bones = arm.data.edit_bones
    root = bones[0]
    root.name = "bone.root"
    root.head = (0, 0, 0)
    root.tail = (0, 0, 0.25)
    body = bones.new("bone.body")
    body.head = (0, 0, 0.34)
    body.tail = (0, 0, 1.2)
    body.parent = root
    for label, x in (("L", -0.16), ("R", 0.16)):
        leg = bones.new(f"bone.leg.{label}")
        leg.head = (x, 0, 0.36)
        leg.tail = (x, 0, 0.02)
        leg.parent = root
    bpy.ops.object.mode_set(mode="OBJECT")
    return arm


def bind(ob, arm, bone):
    group = ob.vertex_groups.new(name=bone)
    group.add(list(range(len(ob.data.vertices))), 1.0, "REPLACE")
    mod = ob.modifiers.new("Armature", "ARMATURE")
    mod.object = arm
    ob.parent = arm


def key_loc(arm, bone, frame, loc):
    pose = arm.pose.bones[bone]
    pose.location = loc
    pose.keyframe_insert(data_path="location", frame=frame)


def key_rot(arm, bone, frame, rot):
    pose = arm.pose.bones[bone]
    pose.rotation_mode = "XYZ"
    pose.rotation_euler = rot
    pose.keyframe_insert(data_path="rotation_euler", frame=frame)


def stash_action(arm, name):
    action = arm.animation_data.action
    action.name = name
    track = arm.animation_data.nla_tracks.new()
    track.name = name
    track.strips.new(name, 1, action)
    arm.animation_data.action = None
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="POSE")
    bpy.ops.pose.select_all(action="SELECT")
    bpy.ops.pose.transforms_clear()
    bpy.ops.object.mode_set(mode="OBJECT")


def animate_agent(arm):
    scene = bpy.context.scene
    scene.frame_start = 1
    # Idle: whole ingot bobs on the root. Root local Y is world Z (bone points up).
    arm.animation_data_create()
    arm.animation_data.action = bpy.data.actions.new("idle")
    for frame, lift in ((1, 0.0), (16, 0.045), (32, 0.0), (48, 0.045), (64, 0.0)):
        key_loc(arm, "bone.root", frame, (0, lift, 0))
        key_rot(arm, "bone.body", frame, (0, 0, 0))
    stash_action(arm, "idle")

    arm.animation_data.action = bpy.data.actions.new("walk")
    swings = ((1, 0.45), (8, -0.45), (16, 0.45))
    for frame, swing in swings:
        key_loc(arm, "bone.root", frame, (0, 0.03 if frame == 8 else 0.0, 0))
        key_rot(arm, "bone.leg.L", frame, (swing, 0, 0))
        key_rot(arm, "bone.leg.R", frame, (-swing, 0, 0))
        key_rot(arm, "bone.body", frame, (0.08 if swing > 0 else -0.08, 0, 0))
    stash_action(arm, "walk")

    arm.animation_data.action = bpy.data.actions.new("celebrate")
    for frame, lift, twist in ((1, 0.0, 0.0), (8, 0.16, 0.25), (16, 0.02, -0.2), (24, 0.14, 0.2), (32, 0.0, 0.0)):
        key_loc(arm, "bone.root", frame, (0, lift, 0))
        key_rot(arm, "bone.body", frame, (0, 0, twist))
    stash_action(arm, "celebrate")


def build_agent():
    reset_scene()
    m = mats()
    parts = []

    def add(ob, bone):
        parts.append(ob)
        return ob, bone

    body = agent_mesh("body", (0, 0, 0.78), (0.66, 0.46, 0.78), m["body"], 0.08)
    leg_l = agent_mesh("leg.L", (-0.16, 0, 0.2), (0.16, 0.16, 0.32), m["body"], 0.03)
    leg_r = agent_mesh("leg.R", (0.16, 0, 0.2), (0.16, 0.16, 0.32), m["body"], 0.03)
    # Hex core. Cylinder axis becomes the facing direction after the X rotation.
    bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.16, depth=0.05, location=(0, -0.27, 0.86))
    core = bpy.context.active_object
    core.name = "core"
    core.rotation_euler = (math.pi / 2, 0, math.radians(30))
    apply_xform(core)
    core.data.materials.append(m["core"])
    shade_flat(core)

    # Four expression plates. The scene shows one. Game state picks happy or alarm.
    faces = [core]
    for index, scale, mat_name in (
        (1, (1.05, 0.05, 1.15), m["core"]),
        (2, (1.25, 0.05, 0.85), m["alarm"]),
        (3, (0.7, 0.05, 0.35), m["teal"]),
    ):
        bpy.ops.mesh.primitive_cylinder_add(
            vertices=6, radius=0.1, depth=0.04, location=(0, -0.3 - index * 0.01, 0.86)
        )
        face = bpy.context.active_object
        face.name = f"face.{index}"
        face.rotation_euler = (math.pi / 2, 0, math.radians(30))
        face.scale = scale
        apply_xform(face)
        face.data.materials.append(mat_name)
        shade_flat(face)
        faces.append(face)
    core.name = "face.0"

    hat_beanie = agent_mesh("hat.1", (0, 0.02, 1.22), (0.42, 0.4, 0.22), m["cloth"], 0.05)
    hat_visor = agent_mesh("hat.2", (0, -0.22, 1.12), (0.4, 0.16, 0.05), m["iron_dark"], 0.01)
    hat_hood = agent_mesh("hat.3", (0, 0.1, 1.16), (0.74, 0.42, 0.4), m["cloth"], 0.04)
    hat_hard = agent_mesh("hat.4", (0, 0, 1.26), (0.7, 0.5, 0.16), m["gold"], 0.03)
    bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.03, depth=0.45, location=(0.22, -0.05, 1.4))
    antenna = bpy.context.active_object
    antenna.name = "hat.5"
    antenna.data.materials.append(m["teal"])
    shade_flat(antenna)
    kit_tip = agent_mesh("hat.5.tip", (0.22, -0.05, 1.66), (0.08, 0.08, 0.08), m["orange"], 0.02)

    headset_l = agent_mesh("acc.1.L", (-0.36, 0, 0.9), (0.1, 0.16, 0.18), m["iron_dark"], 0.02)
    headset_r = agent_mesh("acc.1.R", (0.36, 0, 0.9), (0.1, 0.16, 0.18), m["iron_dark"], 0.02)
    band = agent_mesh("acc.1.band", (0, 0.02, 1.18), (0.7, 0.08, 0.06), m["iron_trim"], 0.0)
    glass = agent_mesh("acc.2", (0.28, -0.32, 0.95), (0.16, 0.04, 0.16), m["glass"], 0.0)
    handle = agent_mesh("acc.2.handle", (0.42, -0.2, 0.78), (0.05, 0.05, 0.28), m["iron_trim"], 0.0)
    goggle_l = agent_mesh("acc.3.L", (-0.12, -0.3, 0.98), (0.16, 0.06, 0.12), m["glass"], 0.01)
    goggle_r = agent_mesh("acc.3.R", (0.12, -0.3, 0.98), (0.16, 0.06, 0.12), m["glass"], 0.01)
    strap = agent_mesh("acc.3.strap", (0, 0, 0.98), (0.5, 0.06, 0.05), m["cloth"], 0.0)
    # Hip kit. Side mounted so the silhouette stays an ingot.
    satchel = agent_mesh("acc.4", (-0.42, 0.05, 0.62), (0.18, 0.22, 0.28), m["cloth"], 0.03)
    satchel_strap = agent_mesh("acc.4.strap", (-0.28, 0.02, 0.9), (0.06, 0.06, 0.4), m["iron_trim"], 0.0)
    bolt = agent_mesh("acc.5", (0.34, -0.22, 0.7), (0.08, 0.1, 0.22), m["gold"], 0.01)

    arm = build_armature()
    parts.append(arm)
    skinned = [
        (body, "bone.body"),
        (leg_l, "bone.leg.L"),
        (leg_r, "bone.leg.R"),
        (hat_beanie, "bone.body"),
        (hat_visor, "bone.body"),
        (hat_hood, "bone.body"),
        (hat_hard, "bone.body"),
        (antenna, "bone.body"),
        (kit_tip, "bone.body"),
        (headset_l, "bone.body"),
        (headset_r, "bone.body"),
        (band, "bone.body"),
        (glass, "bone.body"),
        (handle, "bone.body"),
        (goggle_l, "bone.body"),
        (goggle_r, "bone.body"),
        (strap, "bone.body"),
        (satchel, "bone.body"),
        (satchel_strap, "bone.body"),
        (bolt, "bone.body"),
    ]
    for face in faces:
        skinned.append((face, "bone.body"))
    for ob, bone in skinned:
        bind(ob, arm, bone)
        parts.append(ob)
    animate_agent(arm)
    # TODO: walk swing is bone-local. Confirm the axis once the city plays the clip.
    export_glb(parts, os.path.join(OUT, "agent.glb"), animations=True)
    print(f"agent objects {len(parts)} actions {len(bpy.data.actions)}")


def export_glb(objects, path, animations):
    bpy.ops.object.select_all(action="DESELECT")
    for ob in objects:
        ob.hide_set(False)
        ob.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_apply=False,
        export_yup=True,
        export_animations=animations,
        export_nla_strips=True,
        export_skins=True,
        export_materials="EXPORT",
        export_cameras=False,
        export_lights=False,
    )
    print(f"wrote {path} ({os.path.getsize(path)} bytes)")


if __name__ == "__main__":
    sys_path = os.path.dirname(__file__)
    if sys_path not in sys.path:
        sys.path.insert(0, sys_path)
    import village

    village.build_village()
    build_agent()
