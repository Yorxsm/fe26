"""Original arena interiors. Primitives only: no Kenney kits, no paid assets.

Each room is one mesh per PBR material so the web draws a handful of calls.
Blender Z is up. Export is glTF Y-up, so three.js gets (x, blender z, -blender y).
Rooms are authored in three.js meters: x right, y up, z toward the camera.
"""

import math
import os

import bmesh
import bpy

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT_DIR = os.path.join(ROOT, "apps", "web", "public", "models")


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def make_mat(name, color, metallic, roughness, emission=None, strength=0.0):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = next(node for node in mat.node_tree.nodes if node.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Base Color"].default_value = (*color, 1)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    if emission is not None:
        bsdf.inputs["Emission Color"].default_value = (*emission, 1)
        bsdf.inputs["Emission Strength"].default_value = strength
    return mat


class Room:
    def __init__(self):
        self.bm = {}
        self.mats = {}
        self.count = 0

    def mat(self, name, color, metallic, roughness, emission=None, strength=0.0):
        self.mats[name] = make_mat(name, color, metallic, roughness, emission, strength)
        self.bm[name] = bmesh.new()
        return name

    def _put(self, mat, verts, center, yaw, local):
        co = math.cos(yaw)
        si = math.sin(yaw)
        cx, cy, cz = center
        for vert in verts:
            tx, ty, tz = local(vert)
            rx = tx * co - tz * si
            rz = tx * si + tz * co
            vert.co.x = cx + rx
            vert.co.y = -(cz + rz)
            vert.co.z = cy + ty
        self.count += 1

    def box(self, mat, center, size, yaw=0.0):
        geom = bmesh.ops.create_cube(self.bm[mat], size=1.0)
        sx, sy, sz = size

        def local(vert):
            return (vert.co.x * sx, vert.co.z * sy, vert.co.y * sz)

        self._put(mat, geom["verts"], center, yaw, local)

    def cyl(self, mat, center, radius, height, axis="y", yaw=0.0, segments=12):
        geom = bmesh.ops.create_cone(
            self.bm[mat],
            cap_ends=True,
            segments=segments,
            radius1=radius,
            radius2=radius,
            depth=height,
        )

        def local(vert):
            x, y, z = vert.co.x, vert.co.y, vert.co.z
            if axis == "y":
                return (x, z, y)
            if axis == "z":
                return (x, y, z)
            return (z, y, x)

        self._put(mat, geom["verts"], center, yaw, local)

    def ball(self, mat, center, radius, segments=8):
        geom = bmesh.ops.create_uvsphere(
            self.bm[mat],
            u_segments=segments,
            v_segments=max(4, segments // 2),
            radius=radius,
        )

        def local(vert):
            return (vert.co.x, vert.co.z, vert.co.y)

        self._put(mat, geom["verts"], center, 0.0, local)

    def finish(self, prefix):
        for name, bm in self.bm.items():
            if not bm.verts:
                bm.free()
                continue
            bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
            mesh = bpy.data.meshes.new(name)
            bm.to_mesh(mesh)
            bm.free()
            mesh.update()
            ob = bpy.data.objects.new(f"{prefix}.{name}", mesh)
            ob.data.materials.append(self.mats[name])
            bpy.context.collection.objects.link(ob)
        mins = [1e9, 1e9, 1e9]
        maxs = [-1e9, -1e9, -1e9]
        for ob in bpy.data.objects:
            if ob.type != "MESH":
                continue
            for corner in ob.bound_box:
                world = ob.matrix_world @ __import__("mathutils").Vector(corner)
                for axis in range(3):
                    mins[axis] = min(mins[axis], world[axis])
                    maxs[axis] = max(maxs[axis], world[axis])
        print(prefix, "props", self.count, "materials", len(self.mats))
        print("  blender bounds", [round(v, 2) for v in mins], [round(v, 2) for v in maxs])


def export(filename):
    path = os.path.join(OUT_DIR, filename)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=False,
        export_apply=True,
        export_yup=True,
        export_animations=False,
        export_materials="EXPORT",
    )
    print("wrote", filename, os.path.getsize(path))


def shell(room, wall, floor, trim, ceiling):
    room.box(floor, (0, 0.06, 0), (14.6, 0.12, 12.6))
    room.box(wall, (0, 2.25, -6.15), (14.6, 4.5, 0.28))
    room.box(wall, (-7.15, 2.25, 0), (0.28, 4.5, 12.4))
    room.box(wall, (7.15, 2.25, 0), (0.28, 4.5, 12.4))
    room.box(wall, (-4.3, 2.25, 6.15), (5.7, 4.5, 0.28))
    room.box(wall, (4.3, 2.25, 6.15), (5.7, 4.5, 0.28))
    room.box(wall, (0, 3.85, 6.15), (2.9, 1.3, 0.28))
    room.box(ceiling, (0, 4.42, 0), (14.4, 0.1, 12.4))
    room.box(trim, (0, 0.18, -5.95), (14.2, 0.16, 0.1))
    room.box(trim, (-6.95, 0.18, 0), (0.1, 0.16, 12.0))
    room.box(trim, (6.95, 0.18, 0), (0.1, 0.16, 12.0))
    for x in (-2.2, 2.2):
        room.box(trim, (x, 0.16, 5.4), (0.35, 0.08, 1.3))
    room.box(trim, (0, 0.04, 5.55), (2.4, 0.04, 0.9))


def lamps(room, glow, xs, z, y=3.55):
    for x in xs:
        room.cyl(glow, (x, y, z), 0.18, 0.08, axis="y", segments=8)
        room.box("metal", (x, y + 0.12, z), (0.08, 0.28, 0.08))


def build_vault():
    reset()
    room = Room()
    room.mat("wall", (0.45, 0.32, 0.22), 0.04, 0.82)
    room.mat("floor", (0.28, 0.2, 0.14), 0.08, 0.78)
    room.mat("trim", (0.62, 0.48, 0.28), 0.35, 0.45)
    room.mat("metal", (0.55, 0.57, 0.6), 0.86, 0.32)
    room.mat("gold", (0.86, 0.62, 0.16), 0.94, 0.24)
    room.mat("wood", (0.42, 0.26, 0.14), 0.02, 0.68)
    room.mat("cloth", (0.38, 0.14, 0.12), 0.0, 0.84)
    room.mat("glow", (1.0, 0.55, 0.16), 0.0, 0.4, (1.0, 0.45, 0.12), 2.2)
    room.mat("laser", (1.0, 0.16, 0.18), 0.0, 0.3, (1.0, 0.12, 0.14), 3.4)
    room.mat("glass", (0.55, 0.72, 0.78), 0.15, 0.12, (0.4, 0.7, 0.8), 0.35)
    room.mat("screen", (0.08, 0.09, 0.08), 0.2, 0.4, (0.25, 0.45, 0.22), 0.5)
    shell(room, "wall", "floor", "trim", "metal")
    room.cyl("metal", (0, 1.85, -5.82), 1.72, 0.42, axis="z", segments=20)
    room.cyl("gold", (0, 1.85, -5.55), 1.35, 0.08, axis="z", segments=20)
    room.cyl("trim", (0, 1.85, -5.48), 0.28, 0.16, axis="z", segments=10)
    for index in range(8):
        angle = index * math.pi / 4
        room.cyl("metal", (math.cos(angle) * 0.72, 1.85 + math.sin(angle) * 0.72, -5.42), 0.06, 0.7, axis="z", segments=6)
    for index in range(12):
        angle = index * math.pi / 6
        room.ball("gold", (math.cos(angle) * 1.55, 1.85 + math.sin(angle) * 1.55, -5.58), 0.07, segments=6)
    stacks = [(-3.4, -3.6, 5), (-2.2, -4.2, 3), (-1.0, -3.4, 4), (1.2, -3.8, 6), (2.4, -4.4, 2), (3.3, -3.2, 4)]
    for sx, sz, layers in stacks:
        for layer in range(layers):
            room.box("gold", (sx, 0.22 + layer * 0.16, sz), (0.62, 0.14, 0.32), yaw=0.15 * layer)
    for row in range(3):
        for col in range(4):
            room.box("metal", (-6.82, 0.7 + row * 0.85, -3.6 + col * 0.7), (0.16, 0.62, 0.5))
            room.box("trim", (-6.7, 0.7 + row * 0.85, -3.6 + col * 0.7), (0.04, 0.12, 0.12))
    for index, height in enumerate((0.45, 0.9, 1.35, 1.8, 2.25)):
        room.box("laser", (-2.4 + index * 0.2, height, -1.6), (4.6, 0.025, 0.025))
    for x in (-1.8, 0, 1.8):
        room.box("laser", (x, 1.15, -1.6), (0.025, 2.2, 0.025))
    room.box("wood", (4.6, 0.55, 1.6), (1.5, 0.78, 0.7))
    room.box("wood", (4.6, 0.48, 2.45), (0.45, 0.9, 0.45))
    room.box("cloth", (4.6, 0.95, 2.45), (0.48, 0.08, 0.48))
    room.cyl("glow", (4.15, 1.15, 1.6), 0.1, 0.28, axis="y", segments=8)
    room.box("glass", (4.9, 1.15, 1.45), (0.4, 0.28, 0.05))
    for x, z in ((-5.2, -4.6), (5.2, -4.6), (-5.2, 3.4), (5.2, 3.4)):
        room.ball("glass", (x, 3.85, z), 0.16, segments=8)
        room.box("metal", (x, 4.1, z), (0.08, 0.28, 0.08))
    room.box("glow", (0, 0.05, 5.35), (1.6, 0.04, 0.55))
    room.box("laser", (5.6, 0.05, -1.2), (0.7, 0.04, 0.7))
    room.box("metal", (5.7, 1.3, -1.2), (0.12, 1.5, 0.7))
    for x in (-1.3, 1.3):
        room.cyl("metal", (x, 0.55, 3.4), 0.05, 1.1, axis="y", segments=6)
        room.ball("gold", (x, 1.12, 3.4), 0.08, segments=6)
    room.box("metal", (0, 0.9, 3.4), (2.5, 0.03, 0.03))
    for x, z in ((-4.8, 2.2), (3.2, 3.6), (-3.6, 4.2)):
        room.box("wood", (x, 0.28, z), (0.55, 0.4, 0.45))
    room.box("cloth", (0.2, 0.08, 0.4), (3.4, 0.04, 2.2))
    room.box("screen", (-2.2, 2.45, -5.72), (2.6, 1.15, 0.06))
    room.box("metal", (-5.4, 2.4, -5.9), (0.7, 0.9, 0.08))
    room.cyl("trim", (2.8, 2.5, -5.9), 0.28, 0.06, axis="z", segments=12)
    lamps(room, "glow", (-3.5, 0, 3.5), -2.2)
    room.finish("room.vault")
    export("interior-vault.glb")


def build_market():
    reset()
    room = Room()
    room.mat("wall", (0.55, 0.28, 0.16), 0.02, 0.8)
    room.mat("floor", (0.36, 0.22, 0.14), 0.04, 0.84)
    room.mat("trim", (0.72, 0.4, 0.18), 0.12, 0.55)
    room.mat("metal", (0.5, 0.52, 0.55), 0.8, 0.34)
    room.mat("wood", (0.48, 0.3, 0.16), 0.02, 0.66)
    room.mat("cloth", (0.62, 0.22, 0.16), 0.0, 0.8)
    room.mat("goods", (0.2, 0.45, 0.32), 0.0, 0.7)
    room.mat("glow", (1.0, 0.62, 0.22), 0.0, 0.35, (1.0, 0.55, 0.16), 2.4)
    room.mat("screen", (0.08, 0.16, 0.14), 0.2, 0.25, (0.25, 0.85, 0.45), 1.1)
    room.mat("crate", (0.58, 0.4, 0.22), 0.05, 0.72)
    shell(room, "wall", "floor", "trim", "wood")
    stalls = [(-4.6, -2.4), (-4.6, 0.6), (-1.6, -3.2), (1.6, -3.2), (4.4, -1.8), (4.4, 1.2)]
    for index, (x, z) in enumerate(stalls):
        room.box("wood", (x, 0.5, z), (1.7, 0.78, 0.8))
        room.box("cloth", (x, 1.55, z - 0.15), (1.9, 0.08, 1.15))
        room.box("wood", (x - 0.75, 1.15, z - 0.15), (0.08, 1.1, 0.08))
        room.box("wood", (x + 0.75, 1.15, z - 0.15), (0.08, 1.1, 0.08))
        room.box("goods", (x - 0.35, 0.98, z), (0.32, 0.28, 0.28))
        room.box("crate", (x + 0.3, 1.02, z + 0.1), (0.36, 0.22, 0.3), yaw=0.2)
        if index % 2 == 0:
            room.cyl("goods", (x, 1.05, z + 0.28), 0.12, 0.2, axis="y", segments=8)
    room.box("screen", (0, 2.35, -5.9), (4.4, 1.5, 0.08))
    room.box("screen", (3.6, 1.7, -5.85), (1.8, 1.1, 0.06))
    room.box("trim", (0, 2.35, -5.78), (4.7, 1.75, 0.06))
    room.box("screen", (5.55, 2.15, -1.2), (0.08, 2.2, 2.4))
    room.box("trim", (5.4, 2.15, -1.2), (0.06, 2.45, 2.65))
    for x, z in ((-5.6, 3.4), (-3.2, 4.4), (2.2, 4.2), (5.2, 3.6), (-1.2, 4.6)):
        room.box("crate", (x, 0.32, z), (0.55, 0.48, 0.48))
    for x, z in ((-2.4, 2.2), (2.8, 2.6), (0.4, 3.8)):
        room.cyl("wood", (x, 0.45, z), 0.28, 0.7, axis="y", segments=8)
    room.box("metal", (-2.2, 0.7, -0.4), (0.55, 0.9, 0.4))
    room.box("trim", (-2.05, 1.25, -0.4), (0.35, 0.06, 0.22))
    room.box("wood", (1.4, 0.4, 0.8), (0.4, 0.5, 0.35))
    room.box("cloth", (0, 0.05, 1.2), (2.2, 0.03, 1.4))
    room.box("cloth", (-3.2, 0.05, -0.6), (1.6, 0.03, 1.1))
    for x in (-4.5, -1.5, 1.5, 4.5):
        room.box("metal", (x, 3.7, -1.0), (0.06, 0.7, 0.06))
        room.cyl("glow", (x, 3.25, -1.0), 0.16, 0.1, axis="y", segments=8)
    for x in (-3.0, 0, 3.0):
        room.box("metal", (x, 3.55, 2.4), (0.06, 0.55, 0.06))
        room.cyl("glow", (x, 3.2, 2.4), 0.14, 0.08, axis="y", segments=8)
    room.box("screen", (-5.9, 2.2, 1.5), (0.08, 0.7, 1.1))
    room.box("goods", (3.4, 0.2, -0.2), (0.4, 0.22, 0.35))
    room.finish("room.market")
    export("interior-market.glb")


def build_war():
    reset()
    room = Room()
    room.mat("wall", (0.16, 0.2, 0.26), 0.08, 0.62)
    room.mat("floor", (0.1, 0.12, 0.14), 0.12, 0.7)
    room.mat("trim", (0.28, 0.34, 0.4), 0.45, 0.38)
    room.mat("metal", (0.35, 0.4, 0.46), 0.78, 0.3)
    room.mat("wood", (0.28, 0.18, 0.12), 0.04, 0.6)
    room.mat("cloth", (0.12, 0.16, 0.22), 0.0, 0.75)
    room.mat("glow", (0.2, 0.85, 0.95), 0.1, 0.25, (0.25, 0.9, 1.0), 2.6)
    room.mat("alert", (1.0, 0.18, 0.14), 0.0, 0.3, (1.0, 0.12, 0.1), 3.2)
    room.mat("screen", (0.05, 0.12, 0.16), 0.15, 0.22, (0.2, 0.7, 0.95), 1.3)
    room.mat("flag", (0.75, 0.22, 0.18), 0.0, 0.55)
    shell(room, "wall", "floor", "trim", "metal")
    room.box("wood", (0, 0.48, -1.2), (3.4, 0.72, 1.6))
    room.cyl("glow", (0, 0.95, -1.2), 0.7, 0.06, axis="y", segments=16)
    room.cyl("metal", (0, 0.78, -1.2), 0.12, 0.4, axis="y", segments=8)
    for index, x in enumerate((-4.8, -3.4, 3.6)):
        room.box("metal", (x, 1.35, -5.55), (0.9, 2.3, 0.55))
        for row in range(6):
            room.box("screen", (x, 0.5 + row * 0.34, -5.22), (0.62, 0.18, 0.04))
        room.cyl("alert", (x, 2.65, -5.22), 0.06, 0.04, axis="z", segments=8)
    colors_x = (-1.6, 0, 1.6)
    for x in colors_x:
        room.box("metal", (x, 0.35, -4.3), (0.45, 0.55, 0.45))
        room.box("flag", (x, 1.15, -4.3), (0.06, 0.9, 0.38))
        room.box("trim", (x, 0.7, -4.3), (0.06, 0.7, 0.06))
    for index, x in enumerate((-4.2, -1.4, 1.4, 4.2)):
        room.box("metal", (x, 0.5, 2.4), (1.15, 0.72, 0.55))
        room.box("screen", (x, 1.05, 2.15), (0.7, 0.4, 0.04))
        room.box("cloth", (x, 0.42, 3.15), (0.42, 0.78, 0.42))
        room.cyl("alert", (x + 0.45, 2.7, -5.7), 0.08, 0.08, axis="y", segments=8)
    for z in (-2.5, 0.2, 2.8):
        room.box("trim", (-6.9, 2.6, z), (0.08, 0.16, 0.5))
        room.box("alert", (-6.82, 2.6, z), (0.04, 0.1, 0.28))
    room.box("metal", (0, 0.08, 0.6), (2.2, 0.04, 0.08))
    room.box("metal", (6.4, 1.6, -5.7), (0.5, 1.1, 0.12))
    room.cyl("glow", (0, 3.5, -1.2), 0.22, 0.08, axis="y", segments=10)
    room.box("cloth", (2.4, 0.05, 0.2), (1.8, 0.03, 1.2))
    room.box("screen", (6.7, 1.8, 0.4), (0.06, 0.9, 1.3))
    for x, mat in ((-4.4, "screen"), (0, "alert"), (4.4, "glow")):
        room.box(mat, (x, 0.07, -3.6), (2.4, 0.03, 1.8))
    room.finish("room.war")
    export("interior-war.glb")


def build_forge():
    reset()
    room = Room()
    room.mat("wall", (0.42, 0.18, 0.12), 0.05, 0.78)
    room.mat("floor", (0.22, 0.14, 0.12), 0.1, 0.8)
    room.mat("trim", (0.55, 0.32, 0.16), 0.2, 0.5)
    room.mat("metal", (0.4, 0.42, 0.45), 0.88, 0.28)
    room.mat("wood", (0.36, 0.22, 0.12), 0.02, 0.7)
    room.mat("cloth", (0.48, 0.2, 0.14), 0.0, 0.72)
    room.mat("glow", (1.0, 0.42, 0.1), 0.0, 0.35, (1.0, 0.35, 0.08), 3.0)
    room.mat("skin", (0.72, 0.55, 0.38), 0.05, 0.55)
    room.mat("gold", (0.84, 0.6, 0.18), 0.9, 0.26)
    room.mat("soot", (0.12, 0.1, 0.1), 0.15, 0.85)
    room.mat("screen", (0.1, 0.08, 0.06), 0.15, 0.45, (0.45, 0.28, 0.12), 0.4)
    shell(room, "wall", "floor", "trim", "wood")
    room.box("wood", (-3.2, 0.45, -2.4), (1.3, 0.7, 0.7))
    room.box("metal", (-3.2, 0.95, -2.4), (0.7, 0.28, 0.38))
    room.box("metal", (-2.7, 1.15, -2.4), (0.45, 0.12, 0.12))
    room.box("soot", (-1.2, 0.7, -5.4), (1.5, 1.2, 1.1))
    room.box("glow", (-1.2, 0.85, -4.85), (0.7, 0.55, 0.08))
    room.box("soot", (-1.2, 2.3, -5.5), (0.45, 1.6, 0.45))
    room.box("glow", (-1.2, 3.2, -5.5), (0.2, 0.35, 0.2))
    for index, coal in enumerate((-0.5, 0.1, 0.55)):
        room.ball("soot", (-1.6 + coal, 0.2, -4.2), 0.12, segments=6)
    for index, x in enumerate((-4.6, -2.0, 0.8, 3.4)):
        room.box("trim", (x, 0.45, 1.4), (0.55, 0.7, 0.55))
        room.box("skin", (x, 1.05, 1.4), (0.42, 0.5, 0.18))
        room.box("metal", (x, 1.4, 1.4), (0.5, 0.06, 0.28))
    for index, x in enumerate((-5.2, -3.4, 4.6)):
        room.box("cloth", (x, 0.85, -0.4), (0.42, 0.7, 0.28))
        room.ball("skin", (x, 1.4, -0.4), 0.16, segments=8)
        room.box("metal", (x, 0.35, -0.4), (0.16, 0.5, 0.16))
        room.box("wood", (x, 0.15, -0.2), (0.4, 0.12, 0.4))
    room.box("wood", (5.4, 1.15, -4.6), (1.3, 0.12, 0.45))
    room.box("wood", (4.85, 0.6, -4.6), (0.1, 1.1, 0.4))
    room.box("wood", (5.95, 0.6, -4.6), (0.1, 1.1, 0.4))
    for index in range(5):
        room.cyl("gold", (4.95 + index * 0.18, 1.4, -4.6), 0.07, 0.22, axis="y", segments=8)
    room.box("wood", (6.55, 1.4, -1.0), (0.12, 1.6, 2.2))
    for index in range(8):
        room.box("metal", (6.35, 0.8 + (index % 4) * 0.35, -1.8 + (index // 4) * 1.4), (0.45, 0.08, 0.1), yaw=0.4)
    room.cyl("wood", (-5.4, 0.45, 3.6), 0.32, 0.8, axis="y", segments=8)
    room.cyl("metal", (-4.2, 0.28, 3.5), 0.28, 0.4, axis="y", segments=8)
    room.box("wood", (2.2, 0.28, 3.8), (0.6, 0.4, 0.5))
    room.box("soot", (0.2, 0.16, 3.4), (0.45, 0.2, 0.4))
    room.box("cloth", (-3.2, 0.06, 0.2), (2.0, 0.03, 1.3))
    for x, z in ((-6.4, -2.4), (-6.4, 1.6), (6.4, -0.6)):
        room.box("trim", (x, 1.7, z), (0.16, 2.4, 1.4))
        room.box("screen", (x * 0.985, 1.7, z), (0.05, 1.7, 0.85))
    lamps(room, "glow", (-4.0, -1.2, 2.4), -3.0, y=3.4)
    room.finish("room.forge")
    export("interior-forge.glb")


def build_stadium():
    reset()
    room = Room()
    room.mat("wall", (0.62, 0.5, 0.32), 0.04, 0.74)
    room.mat("floor", (0.34, 0.4, 0.26), 0.06, 0.8)
    room.mat("trim", (0.78, 0.62, 0.36), 0.2, 0.48)
    room.mat("metal", (0.55, 0.56, 0.58), 0.84, 0.3)
    room.mat("seat", (0.55, 0.18, 0.12), 0.0, 0.62)
    room.mat("track", (0.72, 0.28, 0.14), 0.02, 0.7)
    room.mat("infield", (0.22, 0.38, 0.24), 0.0, 0.78)
    room.mat("glow", (1.0, 0.78, 0.42), 0.0, 0.3, (1.0, 0.7, 0.3), 2.8)
    room.mat("screen", (0.06, 0.1, 0.12), 0.2, 0.24, (0.3, 0.9, 0.55), 1.2)
    room.mat("gold", (0.88, 0.66, 0.2), 0.92, 0.22)
    shell(room, "wall", "floor", "trim", "metal")
    room.cyl("infield", (0, 0.14, -0.4), 2.3, 0.06, axis="y", segments=20)
    for index in range(16):
        angle = index * math.pi / 8
        room.box(
            "track",
            (math.cos(angle) * 3.35, 0.16, -0.4 + math.sin(angle) * 2.5),
            (0.85, 0.06, 0.4),
            yaw=angle,
        )
    for row in range(4):
        for seat in range(12):
            x = -5.0 + seat * 0.9
            z = 2.6 + row * 0.72
            y = 0.32 + row * 0.28
            room.box("seat", (x, y, z), (0.7, 0.16, 0.42))
            room.box("trim", (x, y - 0.16, z), (0.72, 0.12, 0.46))
    room.box("screen", (0, 2.7, -5.85), (5.2, 1.5, 0.1))
    room.box("trim", (0, 2.7, -5.7), (5.5, 1.75, 0.08))
    room.box("trim", (0, 0.35, -1.5), (0.7, 0.5, 0.7))
    room.cyl("gold", (0, 0.85, -1.5), 0.22, 0.28, axis="y", segments=10)
    room.cyl("gold", (0, 1.15, -1.5), 0.28, 0.12, axis="y", segments=10)
    room.box("gold", (-0.28, 0.95, -1.5), (0.16, 0.08, 0.06))
    room.box("gold", (0.28, 0.95, -1.5), (0.16, 0.08, 0.06))
    for x, z in ((-5.4, -4.2), (5.4, -4.2), (-5.4, 1.6), (5.4, 1.6)):
        room.box("metal", (x, 2.6, z), (0.16, 2.2, 0.16))
        room.cyl("glow", (x, 3.8, z), 0.28, 0.12, axis="y", segments=8)
        room.box("trim", (x, 3.2, z), (0.35, 0.12, 0.35))
    for x in (-3.2, 3.2):
        room.box("metal", (x, 0.45, 2.15), (0.08, 0.7, 1.4))
    room.box("trim", (0, 3.6, 6.0), (2.2, 0.4, 0.08))
    room.box("screen", (-6.85, 2.0, -2.0), (0.08, 0.8, 1.6))
    room.box("screen", (6.85, 2.0, -2.0), (0.08, 0.8, 1.6))
    room.box("metal", (0, 0.2, 4.4), (3.2, 0.08, 0.12))
    room.finish("room.stadium")
    export("interior-stadium.glb")


def main():
    build_vault()
    build_market()
    build_war()
    build_forge()
    build_stadium()


if __name__ == "__main__":
    main()
