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

    def cone(self, mat, center, radius_bottom, radius_top, height, axis="y", segments=10):
        geom = bmesh.ops.create_cone(
            self.bm[mat],
            cap_ends=True,
            segments=segments,
            radius1=radius_bottom,
            radius2=radius_top,
            depth=height,
        )

        def local(vert):
            x, y, z = vert.co.x, vert.co.y, vert.co.z
            if axis == "y":
                return (x, z, y)
            if axis == "z":
                return (x, y, z)
            return (z, y, x)

        self._put(mat, geom["verts"], center, 0.0, local)

    def ingot(self, mat, center, size, yaw=0.0):
        geom = bmesh.ops.create_cube(self.bm[mat], size=1.0)
        sx, sy, sz = size

        def local(vert):
            x, y, z = vert.co.x, vert.co.y, vert.co.z
            if z > 0:
                x *= 0.62
                y *= 0.62
            return (x * sx, z * sy, y * sz)

        self._put(mat, geom["verts"], center, yaw, local)

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


def pendant(room, x, z):
    room.cyl("metal", (x, 4.02, z), 0.015, 0.62, axis="y", segments=5)
    room.cone("metal", (x, 3.52, z), 0.28, 0.08, 0.26, segments=10)
    room.cyl("glow", (x, 3.38, z), 0.07, 0.05, axis="y", segments=8)


def dress(room):
    for z in (-3.6, 0.2, 3.5):
        room.box("trim", (0, 4.2, z), (13.4, 0.14, 0.2))
    for x, z in ((-5.5, -3.2), (5.5, -3.2), (-5.5, 2.4), (5.5, 2.4)):
        room.cyl("trim", (x, 2.15, z), 0.16, 3.9, axis="y", segments=8)
        room.box("trim", (x, 0.2, z), (0.46, 0.14, 0.46))
        room.box("trim", (x, 4.08, z), (0.4, 0.1, 0.4))
    for z in (-3.4, 0.0, 2.8):
        room.box("trim", (-6.9, 2.15, z), (0.05, 1.7, 1.5))
        room.box("trim", (6.9, 2.15, z), (0.05, 1.7, 1.5))
    for x, z in ((-2.4, -1.5), (2.4, -1.5), (0, 1.7)):
        pendant(room, x, z)
    room.box("cloth", (0, 0.1, 0.4), (3.2, 0.02, 2.0))


def bench(room, x, z):
    room.box("wood", (x, 0.46, z), (1.35, 0.08, 0.4))
    room.box("wood", (x, 0.78, z - 0.16), (1.35, 0.36, 0.08))
    for dx in (-0.52, 0.52):
        room.box("metal", (x + dx, 0.24, z), (0.06, 0.38, 0.32))


def planter(room, x, z):
    room.cyl("trim", (x, 0.28, z), 0.2, 0.4, axis="y", segments=8)
    room.ball("plant", (x, 0.62, z), 0.26, segments=8)


def build_vault():
    reset()
    room = Room()
    room.mat("wall", (0.42, 0.34, 0.26), 0.02, 0.78)
    room.mat("floor", (0.34, 0.32, 0.3), 0.06, 0.55)
    room.mat("trim", (0.58, 0.5, 0.34), 0.45, 0.42)
    room.mat("metal", (0.62, 0.64, 0.66), 1.0, 0.38)
    room.mat("gold", (0.78, 0.58, 0.2), 1.0, 0.32)
    room.mat("wood", (0.4, 0.26, 0.14), 0.02, 0.68)
    room.mat("cloth", (0.36, 0.12, 0.12), 0.0, 0.86)
    room.mat("glow", (0.85, 0.55, 0.22), 0.0, 0.55, (0.9, 0.5, 0.16), 0.3)
    room.mat("laser", (0.55, 0.02, 0.03), 0.0, 0.35, (0.85, 0.02, 0.03), 4.0)
    room.mat("glass", (0.45, 0.55, 0.58), 0.2, 0.12, (0.2, 0.35, 0.4), 0.08)
    room.mat("screen", (0.08, 0.08, 0.08), 0.4, 0.5)
    room.mat("plant", (0.2, 0.36, 0.22), 0.0, 0.8)
    shell(room, "wall", "floor", "trim", "metal")
    room.cyl("metal", (0, 1.9, -5.68), 1.48, 0.28, axis="z", segments=28)
    room.cyl("gold", (0, 1.9, -5.5), 1.18, 0.06, axis="z", segments=28)
    room.cyl("metal", (0, 1.9, -5.44), 0.78, 0.08, axis="z", segments=20)
    room.box("metal", (0, 1.9, -5.38), (1.55, 0.06, 0.05))
    room.box("metal", (0, 1.9, -5.38), (0.06, 1.55, 0.05))
    room.cyl("gold", (0, 1.9, -5.32), 0.2, 0.08, axis="z", segments=12)
    for y in (1.15, 2.65):
        room.cyl("metal", (-1.62, y, -5.55), 0.07, 0.32, axis="y", segments=8)
        room.box("metal", (-1.78, y, -5.55), (0.18, 0.22, 0.16))
    for index in range(12):
        angle = index * math.pi / 6
        room.ball("gold", (math.cos(angle) * 1.32, 1.9 + math.sin(angle) * 1.32, -5.52), 0.05, segments=6)
    for px, pz in ((-3.6, -3.3), (-1.8, -3.8), (1.6, -3.5)):
        room.box("wood", (px, 0.14, pz), (1.35, 0.08, 0.72))
        for layer in range(3):
            for col in range(3):
                room.ingot(
                    "gold",
                    (px - 0.38 + col * 0.38, 0.28 + layer * 0.14, pz),
                    (0.34, 0.11, 0.16),
                    yaw=0.04 * layer,
                )
    for row in range(4):
        for col in range(5):
            room.box("metal", (-6.72, 0.55 + row * 0.72, -3.6 + col * 0.72), (0.16, 0.55, 0.55))
            room.box("gold", (-6.6, 0.55 + row * 0.72, -3.6 + col * 0.72), (0.04, 0.06, 0.16))
    for height in (0.85, 1.25, 1.65, 2.05):
        room.box("laser", (0.2, height, -2.4), (4.8, 0.008, 0.008))
    room.box("wood", (4.8, 0.55, 1.4), (1.6, 0.72, 0.7))
    room.box("trim", (4.8, 0.96, 1.4), (1.7, 0.06, 0.76))
    room.box("screen", (4.8, 1.15, 1.05), (0.7, 0.28, 0.04))
    bench(room, -3.4, 2.4)
    bench(room, 2.2, 2.8)
    planter(room, -5.6, 4.2)
    planter(room, 5.6, 4.2)
    for x, z in ((-5.4, -4.8), (5.4, -4.8), (-5.4, 3.6), (5.4, 3.6)):
        room.ball("glass", (x, 3.7, z), 0.1, segments=8)
    dress(room)
    room.finish("room.vault")
    export("interior-vault.glb")


def build_market():
    reset()
    room = Room()
    room.mat("wall", (0.46, 0.32, 0.2), 0.02, 0.76)
    room.mat("floor", (0.4, 0.28, 0.16), 0.04, 0.72)
    room.mat("trim", (0.55, 0.38, 0.2), 0.25, 0.48)
    room.mat("metal", (0.5, 0.52, 0.54), 0.9, 0.36)
    room.mat("wood", (0.45, 0.28, 0.14), 0.02, 0.66)
    room.mat("cloth", (0.55, 0.16, 0.14), 0.0, 0.82)
    room.mat("goods", (0.28, 0.42, 0.32), 0.05, 0.6)
    room.mat("glow", (0.9, 0.62, 0.28), 0.0, 0.5, (0.95, 0.6, 0.25), 0.28)
    room.mat("screen", (0.06, 0.07, 0.07), 0.35, 0.45)
    room.mat("crate", (0.36, 0.22, 0.12), 0.02, 0.74)
    room.mat("plant", (0.18, 0.4, 0.24), 0.0, 0.78)
    shell(room, "wall", "floor", "trim", "wood")
    stalls = ((-4.6, -1.6), (-1.5, -1.6), (1.6, -1.6), (4.6, -1.6))
    for index, (x, z) in enumerate(stalls):
        room.box("wood", (x, 0.55, z), (1.5, 0.7, 0.7))
        room.box("trim", (x, 0.95, z), (1.6, 0.06, 0.76))
        room.box("cloth", (x, 1.7, z - 0.15), (1.7, 0.08, 0.9))
        room.box("wood", (x - 0.7, 1.15, z - 0.45), (0.08, 0.9, 0.08))
        room.box("wood", (x + 0.7, 1.15, z - 0.45), (0.08, 0.9, 0.08))
        room.box("goods", (x - 0.3, 1.08, z), (0.28, 0.22, 0.24))
        room.cyl("crate", (x + 0.35, 1.05, z + 0.1), 0.12, 0.18, axis="y", segments=8)
    for x, z in ((-5.2, 3.2), (-2.4, 4.0), (3.2, 3.6), (5.4, 4.2)):
        room.box("crate", (x, 0.28, z), (0.48, 0.42, 0.42))
    bench(room, 0, 2.6)
    planter(room, -6.2, 4.4)
    planter(room, 6.2, 4.4)
    room.box("wood", (5.4, 0.55, 0.4), (1.3, 0.7, 0.6))
    room.box("trim", (5.4, 0.95, 0.4), (1.4, 0.06, 0.66))
    dress(room)
    room.finish("room.market")
    export("interior-market.glb")


def build_war():
    reset()
    room = Room()
    room.mat("wall", (0.22, 0.24, 0.28), 0.08, 0.62)
    room.mat("floor", (0.16, 0.17, 0.18), 0.15, 0.55)
    room.mat("trim", (0.38, 0.4, 0.42), 0.55, 0.4)
    room.mat("metal", (0.45, 0.48, 0.5), 0.92, 0.32)
    room.mat("wood", (0.32, 0.22, 0.14), 0.04, 0.66)
    room.mat("cloth", (0.18, 0.2, 0.24), 0.0, 0.8)
    room.mat("glow", (0.25, 0.55, 0.6), 0.0, 0.4, (0.2, 0.55, 0.62), 0.35)
    room.mat("alert", (0.55, 0.08, 0.08), 0.0, 0.4, (0.7, 0.05, 0.05), 0.7)
    room.mat("screen", (0.05, 0.07, 0.08), 0.3, 0.4)
    room.mat("flag", (0.45, 0.16, 0.14), 0.0, 0.7)
    room.mat("plant", (0.16, 0.34, 0.22), 0.0, 0.8)
    shell(room, "wall", "floor", "trim", "metal")
    room.cyl("metal", (0, 0.42, -1.4), 0.9, 0.12, axis="y", segments=16)
    room.cyl("glow", (0, 0.85, -1.4), 0.22, 0.4, axis="y", segments=10)
    for x in (-3.6, 0, 3.6):
        room.box("wood", (x, 0.48, 1.6), (1.6, 0.08, 0.7))
        room.box("metal", (x, 0.28, 1.6), (1.5, 0.32, 0.6))
        room.box("screen", (x - 0.35, 0.85, 1.28), (0.5, 0.36, 0.04))
        room.box("screen", (x + 0.35, 0.9, 1.28), (0.42, 0.42, 0.04))
    for x in (-5.8, 5.8):
        room.box("metal", (x, 1.3, -4.6), (0.55, 2.2, 0.7))
        for row in range(4):
            room.box("glow", (x, 0.5 + row * 0.45, -4.2), (0.28, 0.08, 0.04))
    for x, mat in ((-4.4, "screen"), (0, "alert"), (4.4, "glow")):
        room.box(mat, (x, 0.08, -3.5), (2.2, 0.02, 1.6))
    room.box("cloth", (-2.2, 0.32, 3.4), (1.4, 0.4, 0.6))
    room.box("cloth", (2.4, 0.32, 3.6), (1.4, 0.4, 0.6))
    planter(room, -6.1, 4.3)
    planter(room, 6.1, 4.3)
    room.cyl("alert", (0, 4.05, -1.2), 0.1, 0.08, axis="y", segments=8)
    dress(room)
    room.finish("room.war")
    export("interior-war.glb")


def build_forge():
    reset()
    room = Room()
    room.mat("wall", (0.4, 0.24, 0.18), 0.04, 0.74)
    room.mat("floor", (0.28, 0.2, 0.16), 0.08, 0.7)
    room.mat("trim", (0.52, 0.36, 0.22), 0.3, 0.46)
    room.mat("metal", (0.48, 0.5, 0.52), 0.9, 0.34)
    room.mat("wood", (0.4, 0.24, 0.14), 0.02, 0.66)
    room.mat("cloth", (0.42, 0.18, 0.14), 0.0, 0.78)
    room.mat("glow", (0.85, 0.4, 0.12), 0.0, 0.45, (0.9, 0.35, 0.08), 0.4)
    room.mat("skin", (0.62, 0.48, 0.34), 0.08, 0.5)
    room.mat("gold", (0.76, 0.56, 0.2), 1.0, 0.34)
    room.mat("soot", (0.14, 0.12, 0.11), 0.1, 0.8)
    room.mat("screen", (0.08, 0.07, 0.06), 0.3, 0.45)
    room.mat("plant", (0.2, 0.36, 0.2), 0.0, 0.78)
    shell(room, "wall", "floor", "trim", "wood")
    room.box("wood", (-3.4, 0.78, 0.2), (1.8, 0.78, 0.7))
    room.box("trim", (-3.4, 1.2, 0.2), (1.9, 0.06, 0.76))
    room.box("screen", (-3.4, 1.35, -0.1), (0.6, 0.22, 0.04))
    room.box("soot", (-1.4, 0.7, -4.8), (1.4, 1.15, 1.0))
    room.box("glow", (-1.4, 0.85, -4.25), (0.7, 0.45, 0.08))
    room.box("soot", (-1.4, 2.2, -5.2), (0.4, 1.5, 0.4))
    for x in (-4.8, -2.2, 0.6, 3.2):
        room.box("trim", (x, 0.42, 2.2), (0.5, 0.55, 0.5))
        room.box("skin", (x, 0.9, 2.2), (0.36, 0.42, 0.16))
    for x in (-5.2, 4.8):
        room.box("cloth", (x, 0.85, -0.6), (0.38, 0.7, 0.26))
        room.ball("skin", (x, 1.38, -0.6), 0.14, segments=8)
        room.box("metal", (x, 0.32, -0.6), (0.14, 0.46, 0.14))
    bench(room, 1.6, 0.6)
    planter(room, 6.0, 4.2)
    planter(room, -6.0, 4.2)
    for x, z in ((-6.3, -2.2), (-6.3, 1.8), (6.3, -0.4)):
        room.box("trim", (x, 1.7, z), (0.16, 2.3, 1.35))
        room.box("wood", (x * 0.985, 1.7, z), (0.06, 1.9, 0.9))
    room.box("wood", (5.2, 1.15, -4.4), (1.2, 0.1, 0.4))
    for index in range(4):
        room.cyl("gold", (4.85 + index * 0.18, 1.35, -4.4), 0.06, 0.18, axis="y", segments=8)
    dress(room)
    room.finish("room.forge")
    export("interior-forge.glb")


def build_stadium():
    reset()
    room = Room()
    room.mat("wall", (0.55, 0.46, 0.3), 0.04, 0.7)
    room.mat("floor", (0.32, 0.36, 0.24), 0.04, 0.75)
    room.mat("trim", (0.7, 0.56, 0.32), 0.25, 0.45)
    room.mat("metal", (0.52, 0.54, 0.56), 0.88, 0.34)
    room.mat("seat", (0.48, 0.16, 0.12), 0.0, 0.66)
    room.mat("track", (0.62, 0.26, 0.14), 0.02, 0.7)
    room.mat("infield", (0.2, 0.36, 0.22), 0.0, 0.78)
    room.mat("glow", (0.9, 0.7, 0.35), 0.0, 0.4, (0.95, 0.65, 0.28), 0.25)
    room.mat("screen", (0.06, 0.08, 0.08), 0.3, 0.4)
    room.mat("gold", (0.76, 0.58, 0.2), 1.0, 0.3)
    room.mat("plant", (0.18, 0.36, 0.2), 0.0, 0.78)
    room.mat("wood", (0.4, 0.26, 0.14), 0.02, 0.66)
    room.mat("cloth", (0.42, 0.16, 0.12), 0.0, 0.8)
    shell(room, "wall", "floor", "trim", "metal")
    room.cyl("infield", (0, 0.14, -0.6), 2.2, 0.05, axis="y", segments=20)
    for index in range(16):
        angle = index * math.pi / 8
        room.box("track", (math.cos(angle) * 3.2, 0.16, -0.6 + math.sin(angle) * 2.3), (0.8, 0.05, 0.36), yaw=angle)
    for row in range(4):
        for seat in range(12):
            x = -5.0 + seat * 0.9
            z = 2.5 + row * 0.7
            y = 0.32 + row * 0.26
            room.box("seat", (x, y, z), (0.66, 0.12, 0.38))
            room.box("trim", (x, y - 0.14, z), (0.7, 0.1, 0.42))
    room.box("trim", (0, 0.32, -1.6), (0.55, 0.42, 0.55))
    room.cyl("gold", (0, 0.75, -1.6), 0.16, 0.28, axis="y", segments=10)
    room.cyl("gold", (0, 1.05, -1.6), 0.22, 0.08, axis="y", segments=10)
    for x, z in ((-5.2, -4.0), (5.2, -4.0)):
        room.box("metal", (x, 2.4, z), (0.12, 2.4, 0.12))
        room.cone("metal", (x, 3.7, z), 0.22, 0.08, 0.2, segments=8)
        room.cyl("glow", (x, 3.55, z), 0.06, 0.04, axis="y", segments=6)
    planter(room, -6.0, 4.4)
    planter(room, 6.0, 4.4)
    bench(room, -3.2, -3.4)
    bench(room, 3.2, -3.4)
    dress(room)
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
