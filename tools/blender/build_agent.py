"""Soft ingot body for fe2VI. One rig, idle / walk / celebrate / caught.

Does not touch station.glb. Role accessories are added by the same script
when ROLE_MESHES is filled; the base body is the shared mesh.
"""

import math
import os

import bmesh
import bpy

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "apps", "web", "public", "models", "agent.glb")


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.scene.render.fps = 24


def material(name, color, metallic=0.3, roughness=0.5, emission=0.0):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = next(node for node in mat.node_tree.nodes if node.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Base Color"].default_value = (*color, 1)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    if emission and "Emission Color" in bsdf.inputs:
        bsdf.inputs["Emission Color"].default_value = (*color, 1)
        bsdf.inputs["Emission Strength"].default_value = emission
    return mat


def shade_smooth(ob):
    for poly in ob.data.polygons:
        poly.use_smooth = True


def finish(ob, bevel=0.0, subdiv=0):
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if bevel > 0:
        mod = ob.modifiers.new("Bevel", "BEVEL")
        mod.width = bevel
        mod.segments = 3
        mod.affect = "EDGES"
        bpy.ops.object.modifier_apply(modifier=mod.name)
    if subdiv:
        mod = ob.modifiers.new("Subsurf", "SUBSURF")
        mod.levels = subdiv
        bpy.ops.object.modifier_apply(modifier=mod.name)
    shade_smooth(ob)
    return ob


def cube(name, loc, size, mat, bevel=0.08, subdiv=1, rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc, rotation=rot)
    ob = bpy.context.active_object
    ob.name = name
    ob.dimensions = size
    ob.data.materials.append(mat)
    return finish(ob, bevel, subdiv)


def ball(name, loc, radius, mat, scale=(1, 1, 1)):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=12, radius=radius, location=loc)
    ob = bpy.context.active_object
    ob.name = name
    ob.scale = scale
    ob.data.materials.append(mat)
    return finish(ob)


def cyl(name, loc, radius, depth, mat, vertices=8, rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=depth, location=loc, rotation=rot)
    ob = bpy.context.active_object
    ob.name = name
    ob.data.materials.append(mat)
    return finish(ob, bevel=0.01, subdiv=0)


def torus(name, loc, major, minor, mat, rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_torus_add(
        major_radius=major,
        minor_radius=minor,
        major_segments=28,
        minor_segments=8,
        location=loc,
        rotation=rot,
    )
    ob = bpy.context.active_object
    ob.name = name
    ob.data.materials.append(mat)
    return finish(ob, bevel=0.0, subdiv=0)


def plane(name, loc, size, mat, rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_plane_add(size=size, location=loc, rotation=rot)
    ob = bpy.context.active_object
    ob.name = name
    ob.data.materials.append(mat)
    return finish(ob, bevel=0.0, subdiv=0)


def _oct_verts(radius, y, z, twist):
    return [
        (math.cos(twist + i * math.pi / 4) * radius, y, z + math.sin(twist + i * math.pi / 4) * radius) for i in range(8)
    ]


def octagon_ring(name, lip_y, z, outer, inner, depth, mat, twist):
    """Open octagonal bezel. lip_y is the front edge, toward the camera (-Y)."""
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()

    def ring(y, radius):
        return [bm.verts.new(co) for co in _oct_verts(radius, y, z, twist)]

    front_y = lip_y
    back_y = lip_y + depth
    front_outer, front_inner = ring(front_y, outer), ring(front_y, inner)
    back_outer, back_inner = ring(back_y, outer), ring(back_y, inner)
    for i in range(8):
        j = (i + 1) % 8
        bm.faces.new((front_outer[i], front_outer[j], front_inner[j], front_inner[i]))
        bm.faces.new((back_outer[i], back_inner[i], back_inner[j], back_outer[j]))
        bm.faces.new((front_outer[i], back_outer[i], back_outer[j], front_outer[j]))
        bm.faces.new((front_inner[i], front_inner[j], back_inner[j], back_inner[i]))
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    ob = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(ob)
    ob.data.materials.append(mat)
    for poly in ob.data.polygons:
        poly.use_smooth = False
    return ob


def octagon_disc(name, y, z, radius, mat, twist, depth=0.006):
    """Thin octagonal glass. The depth keeps quantization from dropping the plate off the head."""
    mesh = bpy.data.meshes.new(name)
    front = _oct_verts(radius, y, z, twist)
    back = _oct_verts(radius, y + depth, z, twist)
    verts = front + back + [(0.0, y, z), (0.0, y + depth, z)]
    faces = []
    for i in range(8):
        j = (i + 1) % 8
        faces.append((i, j, 16))
        faces.append((8 + i, 17, 8 + j))
        faces.append((i, 8 + i, 8 + j, j))
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    if mesh.polygons[0].normal.y > 0:
        mesh.flip_normals()
        mesh.update()
    half = max(max(abs(co[0]) for co in front), max(abs(co[2] - z) for co in front))
    uv = mesh.uv_layers.new(name="UVMap")
    for poly in mesh.polygons:
        for loop_index in poly.loop_indices:
            co = mesh.vertices[mesh.loops[loop_index].vertex_index].co
            uv.data[loop_index].uv = (0.5 + co.x / (2 * half), 0.5 + (co.z - z) / (2 * half))
    ob = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(ob)
    ob.data.materials.append(mat)
    for poly in ob.data.polygons:
        poly.use_smooth = False
    return ob


def build_armature():
    bpy.ops.object.armature_add(enter_editmode=True, location=(0, 0, 0))
    arm = bpy.context.active_object
    arm.name = "AgentRig"
    bones = arm.data.edit_bones
    root = bones[0]
    root.name = "bone.root"
    root.head = (0, 0, 0)
    root.tail = (0, 0, 0.16)
    hips = bones.new("bone.hips")
    hips.head = (0, 0, 0.22)
    hips.tail = (0, 0, 0.36)
    hips.parent = root
    spine = bones.new("bone.spine")
    spine.head = (0, 0, 0.36)
    spine.tail = (0, -0.06, 0.78)
    spine.parent = hips
    head = bones.new("bone.head")
    head.head = (0, -0.06, 0.78)
    head.tail = (0, -0.1, 1.02)
    head.parent = spine
    for label, x in (("L", -0.16), ("R", 0.16)):
        leg = bones.new(f"bone.leg.{label}")
        leg.head = (x, 0.0, 0.22)
        leg.tail = (x, 0.02, 0.0)
        leg.parent = hips
        # Local +X stays sideways, so an X rotation swings the foot toward the front (-Y).
        leg.align_roll((1, 0, 0))
    for label, x in (("L", -0.36), ("R", 0.36)):
        arm_bone = bones.new(f"bone.arm.{label}")
        arm_bone.head = (x, -0.04, 0.7)
        arm_bone.tail = (x, -0.02, 0.38)
        arm_bone.parent = spine
        arm_bone.align_roll((1, 0, 0))
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


def swing(axis, amount):
    rot = [0.0, 0.0, 0.0]
    rot[axis] = amount
    return tuple(rot)


def verify_lift(arm, bone_name):
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="POSE")
    bone = arm.pose.bones[bone_name]
    bone.rotation_mode = "XYZ"
    bpy.context.view_layer.update()
    rest = (arm.matrix_world @ bone.tail).copy()
    best = None
    for axis in range(3):
        bone.rotation_euler = tuple(0.8 if i == axis else 0.0 for i in range(3))
        bpy.context.view_layer.update()
        delta = (arm.matrix_world @ bone.tail) - rest
        if best is None or delta.z > best[1]:
            best = (axis, delta.z)
        bone.rotation_euler = (0, 0, 0)
        bpy.context.view_layer.update()
    bpy.ops.pose.transforms_clear()
    bpy.ops.object.mode_set(mode="OBJECT")
    sign = 1.0 if best[1] > 0 else -1.0
    print("lift", bone_name, "XYZ"[best[0]], round(best[1], 3))
    return best[0], sign


def animate(arm, leg_axis, leg_sign, left_arm, right_arm):
    bpy.context.scene.frame_start = 1
    arm.animation_data_create()
    arm.animation_data.action = bpy.data.actions.new("idle")
    for frame, lift in ((1, 0.0), (16, 0.025), (32, 0.0), (48, 0.025), (64, 0.0)):
        key_loc(arm, "bone.hips", frame, (0, 0, lift))
        key_rot(arm, "bone.spine", frame, (0.02 * math.sin(frame / 8), 0, 0))
    stash_action(arm, "idle")

    # In-place walk. Stance foot rotates from forward to back while a small
    # bone lift keeps the sole near the ground; the swing foot clears it.
    arm.animation_data.action = bpy.data.actions.new("walk")
    poses = (
        (1, 0.42, 0.0),
        (7, 0.0, 0.05),
        (13, -0.42, 0.0),
        (19, 0.0, 0.05),
        (24, 0.42, 0.0),
    )
    for frame, swing_amount, lift in poses:
        key_loc(arm, "bone.hips", frame, (0, 0, 0.02 if abs(swing_amount) < 0.1 else 0.0))
        key_rot(arm, "bone.spine", frame, (0.12, 0, 0.04 if swing_amount > 0 else -0.04))
        forward = leg_sign * swing_amount
        key_rot(arm, "bone.leg.L", frame, swing(leg_axis, forward))
        key_loc(arm, "bone.leg.L", frame, (0, 0, lift if swing_amount > 0.05 else 0.0))
        key_rot(arm, "bone.leg.R", frame, swing(leg_axis, -forward))
        key_loc(arm, "bone.leg.R", frame, (0, 0, lift if swing_amount < -0.05 else 0.0))
        key_rot(arm, "bone.arm.L", frame, swing(leg_axis, -forward * 0.8))
        key_rot(arm, "bone.arm.R", frame, swing(leg_axis, forward * 0.8))
    stash_action(arm, "walk")

    arm.animation_data.action = bpy.data.actions.new("celebrate")
    for frame, lift, arm_amt in ((1, 0.0, 0.2), (6, 0.26, 2.2), (12, 0.06, 1.7), (18, 0.0, 0.2)):
        key_loc(arm, "bone.hips", frame, (0, 0, lift))
        key_rot(arm, "bone.arm.L", frame, swing(left_arm[0], left_arm[1] * arm_amt))
        key_rot(arm, "bone.arm.R", frame, swing(right_arm[0], right_arm[1] * arm_amt))
        key_rot(arm, "bone.spine", frame, (-0.5 if lift > 0.1 else 0.0, 0, 0))
    stash_action(arm, "celebrate")

    arm.animation_data.action = bpy.data.actions.new("caught")
    key_rot(arm, "bone.spine", 1, (0.7, 0, 0))
    key_rot(arm, "bone.head", 1, (0.4, 0, 0))
    key_rot(arm, "bone.arm.L", 1, (0.3, 0, 0.2))
    key_rot(arm, "bone.arm.R", 1, (0.3, 0, -0.2))
    key_loc(arm, "bone.hips", 1, (0, 0, -0.04))
    key_rot(arm, "bone.spine", 12, (0.7, 0, 0))
    stash_action(arm, "caught")


def verify_leg_axis(arm):
    """Find which euler axis swings the foot toward the face (-Y) and record it."""
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="POSE")
    bone = arm.pose.bones["bone.leg.L"]
    bone.rotation_mode = "XYZ"
    bpy.context.view_layer.update()
    rest = (arm.matrix_world @ bone.tail).copy()
    best = None
    for axis in range(3):
        bone.rotation_euler = tuple(0.5 if i == axis else 0.0 for i in range(3))
        bpy.context.view_layer.update()
        delta = (arm.matrix_world @ bone.tail) - rest
        print("leg axis", "XYZ"[axis], round(delta.x, 3), round(delta.y, 3), round(delta.z, 3))
        if abs(delta.y) > 0.01 and (best is None or abs(delta.y) > abs(best[1])):
            best = (axis, delta.y)
        bone.rotation_euler = (0, 0, 0)
        bpy.context.view_layer.update()
    bpy.ops.pose.transforms_clear()
    bpy.ops.object.mode_set(mode="OBJECT")
    if best is None:
        raise SystemExit("no leg axis swings the foot toward the face")
    # Positive pose values mean "toward the face". Flip when +rotation points backward.
    return best[0], (-1.0 if best[1] > 0 else 1.0)


def build_body(arm, parts, skinned):
    body = material("Body", (0.60, 0.63, 0.65), 0.3, 0.5)
    trim = material("Trim", (0.22, 0.22, 0.24), 0.45, 0.4)
    screen = material("Screen", (0.039, 0.039, 0.043), 0.62, 0.16, emission=0.0)
    bezel = material("Bezel", (0.78, 0.80, 0.82), 0.22, 0.38)
    eye = material("Eye", (0.02, 0.02, 0.025), 0.1, 0.3)
    bolt = material("Bolt", (0.55, 0.5, 0.42), 0.8, 0.35)

    torso = cube("body.torso", (0, -0.02, 0.58), (0.62, 0.48, 0.70), body, bevel=0.08, subdiv=1, rot=(-0.12, 0, 0))
    skinned.append((torso, "bone.spine"))
    parts.append(torso)

    for label, x in (("L", -0.14), ("R", 0.14)):
        leg = cube(f"body.leg.{label}", (x, 0.0, 0.13), (0.16, 0.16, 0.2), body, bevel=0.05, subdiv=1)
        boot = cube(f"body.boot.{label}", (x, -0.015, 0.045), (0.18, 0.2, 0.09), trim, bevel=0.03, subdiv=1)
        skinned.append((leg, f"bone.leg.{label}"))
        skinned.append((boot, f"bone.leg.{label}"))
        parts.extend((leg, boot))
        arm_mesh = cube(f"body.arm.{label}", (x * 2.5, -0.03, 0.58), (0.13, 0.13, 0.26), body, bevel=0.045, subdiv=1)
        hand = ball(f"body.hand.{label}", (x * 2.55, -0.04, 0.42), 0.07, body, scale=(1.15, 0.9, 0.85))
        skinned.append((arm_mesh, f"bone.arm.{label}"))
        skinned.append((hand, f"bone.arm.{label}"))
        parts.extend((arm_mesh, hand))

    front = min((torso.matrix_world @ vertex.co).y for vertex in torso.data.vertices)
    twist = math.pi / 8
    lip = front - 0.02
    bezel_mesh = octagon_ring("body.bezel", lip, 0.64, 0.275, 0.222, 0.032, bezel, twist)
    glass = material("Face", (0.039, 0.039, 0.043), 0.08, 0.72)
    # Flush with the inner wall, 4mm behind the lip, so the octagon fills the opening.
    plate = octagon_disc("body.faceplate", lip + 0.004, 0.64, 0.221, glass, twist)
    if plate.data.polygons[0].normal.y > -0.5:
        raise SystemExit(f"faceplate normal faces the wrong way: {plate.data.polygons[0].normal}")
    center_uv = plate.data.uv_layers[0].data[plate.data.polygons[0].loop_indices[-1]].uv
    print("face lip", round(lip, 4), "glass", round(lip + 0.004, 4), "normal", tuple(round(c, 3) for c in plate.data.polygons[0].normal), "center uv", tuple(round(c, 3) for c in center_uv))
    for ob in (bezel_mesh, plate):
        skinned.append((ob, "bone.head"))
        parts.append(ob)

    # Expressions are separate meshes so the runtime can show one set.
    def eyes(prefix, z_scale, rot_l, rot_r, z=0.68):
        left = ball(f"{prefix}.eye.L", (-0.08, front - 0.04, z), 0.035, eye, scale=(0.7, 0.45, z_scale))
        right = ball(f"{prefix}.eye.R", (0.08, front - 0.04, z), 0.035, eye, scale=(0.7, 0.45, z_scale))
        left.rotation_euler = rot_l
        right.rotation_euler = rot_r
        bpy.context.view_layer.objects.active = left
        bpy.ops.object.transform_apply(rotation=True)
        bpy.context.view_layer.objects.active = right
        bpy.ops.object.transform_apply(rotation=True)
        return left, right

    neutral = eyes("body.face.0", 1.7, (0, 0, 0), (0, 0, 0))
    happy = eyes("body.face.1", 0.55, (0, 0.4, 0.7), (0, -0.4, -0.7), 0.7)
    angry = eyes("body.face.2", 1.35, (0, 0.2, -0.45), (0, -0.2, 0.45), 0.7)
    smile_eyes = eyes("body.face.3", 1.2, (0, 0, 0.15), (0, 0, -0.15))
    smile = cyl("body.face.3.mouth", (0, front - 0.04, 0.56), 0.07, 0.012, eye, vertices=12, rot=(math.pi / 2, 0, 0))
    smile.scale = (1.4, 0.35, 0.45)
    bpy.context.view_layer.objects.active = smile
    bpy.ops.object.transform_apply(scale=True)
    wide = eyes("body.face.4", 2.35, (0, 0, 0), (0, 0, 0), 0.68)
    sleepy = eyes("body.face.5", 0.32, (0, 0, 0), (0, 0, 0), 0.655)
    for ob in (*neutral, *happy, *angry, *smile_eyes, smile, *wide, *sleepy):
        skinned.append((ob, "bone.head"))
        parts.append(ob)

    for index, (x, z) in enumerate(((-0.22, 0.78), (0.22, 0.78), (-0.2, 0.4), (0.2, 0.4))):
        stud = cyl(f"body.bolt.{index}", (x, front - 0.008, z), 0.018, 0.012, bolt, vertices=6, rot=(math.pi / 2, 0, 0))
        skinned.append((stud, "bone.spine"))
        parts.append(stud)
    line = cube("body.panel", (0, front + 0.01, 0.5), (0.4, 0.012, 0.012), trim, bevel=0.002, subdiv=0)
    emblem = cyl("body.emblem", (0, front - 0.006, 0.46), 0.045, 0.012, trim, vertices=12, rot=(math.pi / 2, 0, 0))
    ear = cube("body.ear", (0.34, 0.0, 0.72), (0.06, 0.1, 0.08), trim, bevel=0.02, subdiv=1)
    antenna = cyl("body.antenna", (0.1, 0.0, 0.98), 0.015, 0.16, trim, vertices=8)
    tip = ball("body.antenna.tip", (0.1, 0.0, 1.08), 0.035, screen)
    for ob, bone_name in ((line, "bone.spine"), (emblem, "bone.spine"), (ear, "bone.head"), (antenna, "bone.head"), (tip, "bone.head")):
        skinned.append((ob, bone_name))
        parts.append(ob)
    return front


def wear(parts, skinned, ob, bone):
    skinned.append((ob, bone))
    parts.append(ob)


def build_roles(parts, skinned, front):
    knit = material("Knit", (0.05, 0.05, 0.06), 0.0, 0.92)
    rib = material("Rib", (0.09, 0.09, 0.1), 0.0, 0.84)
    gold = material("Gold", (0.85, 0.68, 0.22), 0.85, 0.28)
    navy = material("Hood", (0.07, 0.09, 0.13), 0.04, 0.82)
    fold = material("Fold", (0.12, 0.14, 0.18), 0.04, 0.78)
    cyan = material("Glyph", (0.18, 0.9, 0.9), 0.1, 0.3, emission=1.6)
    lens = material("Lens", (0.72, 0.88, 0.95), 0.15, 0.05, emission=0.35)
    hat = material("Hat", (0.95, 0.62, 0.16), 0.12, 0.48)
    ridge = material("Ridge", (0.75, 0.42, 0.1), 0.2, 0.4)
    pack = material("Pack", (0.32, 0.2, 0.11), 0.05, 0.72)
    pocket = material("Pocket", (0.42, 0.26, 0.14), 0.08, 0.6)
    strap = material("Strap", (0.18, 0.12, 0.08), 0.1, 0.55)
    lime = material("Compass", (0.61, 1.0, 0.23), 0.2, 0.35, emission=0.55)
    crown_mat = material("Crown", (0.93, 0.78, 0.32), 0.86, 0.22)
    cloth = material("Cape", (0.42, 0.1, 0.12), 0.02, 0.78)
    cloth_dark = material("CapeFold", (0.28, 0.06, 0.08), 0.02, 0.8)
    jewel = material("Jewel", (0.35, 0.85, 0.95), 0.1, 0.08, emission=1.2)

    # Knit beanie with a ribbed cuff, plus the safecracker keyhole.
    wear(parts, skinned, ball("role.safecracker.beanie.cap", (0, -0.02, 1.0), 0.2, knit, scale=(1.28, 1.12, 0.62)), "bone.head")
    for index in range(5):
        wear(
            parts,
            skinned,
            torus(
                f"role.safecracker.beanie.rib.{index}",
                (0, -0.02, 0.9 - index * 0.012),
                0.175,
                0.012,
                rib if index % 2 else knit,
                rot=(math.pi / 2, 0, 0),
            ),
            "bone.head",
        )
    wear(parts, skinned, cyl("role.safecracker.keyhole", (0, front - 0.02, 0.93), 0.028, 0.016, gold, vertices=12, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, cube("role.safecracker.keyhole.slot", (0, front - 0.03, 0.91), (0.012, 0.01, 0.028), knit, bevel=0.0, subdiv=0), "bone.head")

    # Headset band, ear cups, and a mic boom.
    wear(parts, skinned, torus("role.trader.band.arch", (0, 0.0, 0.98), 0.2, 0.016, gold, rot=(0, math.pi / 2, 0)), "bone.head")
    for label, x in (("L", -0.2), ("R", 0.2)):
        wear(parts, skinned, cyl(f"role.trader.band.cup.{label}", (x, 0.0, 0.86), 0.055, 0.04, gold, vertices=16, rot=(0, math.pi / 2, 0)), "bone.head")
        wear(parts, skinned, cyl(f"role.trader.band.pad.{label}", (x, front * 0.15, 0.86), 0.04, 0.02, navy, vertices=16, rot=(0, math.pi / 2, 0)), "bone.head")
    wear(parts, skinned, cyl("role.trader.band.boom", (0.32, front - 0.02, 0.78), 0.008, 0.22, gold, vertices=8, rot=(math.pi / 2.6, 0, 0.5)), "bone.head")
    wear(parts, skinned, ball("role.trader.band.mic", (0.36, front - 0.06, 0.66), 0.028, gold), "bone.head")
    wear(parts, skinned, cube("role.trader.pad.L", (-0.36, -0.02, 0.74), (0.16, 0.14, 0.07), gold, bevel=0.02, subdiv=1), "bone.spine")
    wear(parts, skinned, cube("role.trader.pad.R", (0.36, -0.02, 0.74), (0.16, 0.14, 0.07), gold, bevel=0.02, subdiv=1), "bone.spine")

    # Hood shell with side folds and drawstrings.
    wear(parts, skinned, ball("role.hacker.hood.shell", (0, 0.05, 0.92), 0.27, navy, scale=(1.18, 1.08, 0.9)), "bone.head")
    wear(parts, skinned, ball("role.hacker.hood.brow", (0, front + 0.06, 0.98), 0.16, navy, scale=(1.5, 0.35, 0.28)), "bone.head")
    for index, x, tilt in ((0, -0.16, 0.5), (1, 0.0, 0.15), (2, 0.16, -0.5)):
        wear(
            parts,
            skinned,
            cube(f"role.hacker.hood.fold.{index}", (x, 0.1, 0.78), (0.035, 0.08, 0.32), fold, bevel=0.01, subdiv=0, rot=(0.35, 0, tilt)),
            "bone.head",
        )
    wear(parts, skinned, cyl("role.hacker.string.L", (-0.08, front - 0.02, 0.58), 0.007, 0.24, navy, vertices=8), "bone.spine")
    wear(parts, skinned, cyl("role.hacker.string.R", (0.08, front - 0.02, 0.58), 0.007, 0.24, navy, vertices=8), "bone.spine")
    wear(parts, skinned, cube("role.hacker.glyph", (0, front - 0.03, 0.34), (0.07, 0.012, 0.07), cyan, bevel=0.008, subdiv=0), "bone.spine")

    # Loupe on a strap, plus a rimmed shield.
    wear(parts, skinned, torus("role.auditor.strap", (0, 0.02, 0.9), 0.2, 0.01, gold, rot=(math.pi / 2.4, 0.4, 0)), "bone.head")
    wear(parts, skinned, torus("role.auditor.rim", (0.34, front - 0.03, 0.7), 0.045, 0.01, gold, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, cyl("role.auditor.glass", (0.34, front - 0.04, 0.7), 0.038, 0.012, lens, vertices=20, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, cyl("role.auditor.handle", (0.16, front - 0.02, 0.58), 0.01, 0.14, gold, vertices=8, rot=(0.4, 0, 0.3)), "bone.head")
    wear(parts, skinned, cyl("role.auditor.shield", (0.1, front - 0.02, 0.46), 0.055, 0.012, gold, vertices=6, rot=(math.pi / 2, 0, 0)), "bone.spine")
    wear(parts, skinned, cyl("role.auditor.shield.rim", (0.1, front - 0.012, 0.46), 0.07, 0.008, ridge, vertices=6, rot=(math.pi / 2, 0, 0)), "bone.spine")

    # Hard hat: dome, brim, and a center ridge.
    wear(parts, skinned, ball("role.builder.hat.dome", (0, -0.01, 0.98), 0.21, hat, scale=(1.35, 1.2, 0.55)), "bone.head")
    wear(parts, skinned, cyl("role.builder.hat.brim", (0, -0.02, 0.9), 0.28, 0.018, hat, vertices=20), "bone.head")
    wear(parts, skinned, cube("role.builder.hat.ridge", (0, -0.04, 1.05), (0.03, 0.16, 0.08), ridge, bevel=0.008, subdiv=0), "bone.head")
    wear(parts, skinned, cube("role.builder.badge", (0, front - 0.03, 0.48), (0.12, 0.014, 0.07), hat, bevel=0.008, subdiv=0), "bone.spine")

    # Goggles, a strap, and the scout compass. The pack lives on back.0.
    for label, x in (("L", -0.08), ("R", 0.08)):
        wear(parts, skinned, torus(f"role.scout.goggle.{label}", (x, front + 0.02, 0.98), 0.04, 0.01, lime, rot=(math.pi / 2, 0, 0)), "bone.head")
        wear(parts, skinned, cyl(f"role.scout.goggle.{label}.lens", (x, front + 0.01, 0.98), 0.032, 0.01, lens, vertices=16, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, torus("role.scout.goggle.strap", (0, 0.04, 0.98), 0.2, 0.012, strap, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, cube("role.scout.goggle.bridge", (0, front + 0.02, 0.98), (0.06, 0.016, 0.016), lime, bevel=0.004, subdiv=0), "bone.head")
    wear(parts, skinned, cyl("role.scout.compass", (0.16, front - 0.03, 0.5), 0.038, 0.012, lime, vertices=16, rot=(math.pi / 2, 0, 0)), "bone.spine")
    wear(parts, skinned, cyl("role.scout.compass.ring", (0.16, front - 0.02, 0.5), 0.048, 0.008, gold, vertices=16, rot=(math.pi / 2, 0, 0)), "bone.spine")

    # Crown with points and a jewel, and a cap with a stiff brim.
    wear(parts, skinned, torus("headwear.crown.band", (0, -0.01, 0.96), 0.16, 0.022, crown_mat, rot=(math.pi / 2, 0, 0)), "bone.head")
    for index, x in enumerate((-0.1, 0.0, 0.1)):
        wear(parts, skinned, cube(f"headwear.crown.point.{index}", (x, -0.02, 1.06), (0.035, 0.03, 0.1), crown_mat, bevel=0.008, subdiv=0), "bone.head")
    wear(parts, skinned, ball("headwear.crown.jewel", (0, front - 0.03, 0.96), 0.022, jewel), "bone.head")
    wear(parts, skinned, ball("headwear.cap.crown", (0, -0.03, 0.98), 0.18, hat, scale=(1.25, 1.15, 0.48)), "bone.head")
    wear(parts, skinned, cube("headwear.brim", (0, front - 0.06, 0.9), (0.32, 0.16, 0.02), hat, bevel=0.006, subdiv=0), "bone.head")
    wear(parts, skinned, cube("headwear.cap.button", (0, -0.02, 1.08), (0.04, 0.04, 0.02), ridge, bevel=0.004, subdiv=0), "bone.head")

    # Face items sit around the glass, not across the drawn eyes.
    wear(parts, skinned, cube("faceitem.0.visor", (0, front - 0.01, 0.96), (0.32, 0.025, 0.028), gold, bevel=0.006, subdiv=0), "bone.head")
    wear(parts, skinned, torus("faceitem.1.monocle", (0.34, front - 0.03, 0.7), 0.04, 0.008, gold, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, cyl("faceitem.1.chain", (0.16, 0.02, 0.78), 0.006, 0.16, gold, vertices=8, rot=(0.3, 0, 0.2)), "bone.head")
    wear(parts, skinned, cube("faceitem.2.mask", (0, front - 0.02, 0.36), (0.24, 0.03, 0.08), navy, bevel=0.01, subdiv=0), "bone.head")
    wear(parts, skinned, cube("faceitem.2.trim", (0, front - 0.03, 0.40), (0.26, 0.012, 0.016), gold, bevel=0.003, subdiv=0), "bone.head")
    wear(parts, skinned, cyl("faceitem.3.mic", (-0.34, front - 0.02, 0.66), 0.01, 0.18, gold, vertices=8, rot=(math.pi / 2.5, 0, -0.45)), "bone.head")
    wear(parts, skinned, ball("faceitem.3.head", (-0.38, front - 0.06, 0.56), 0.026, gold), "bone.head")

    # Back items: pack with straps and a pocket, tanks, a folded cape, an array.
    wear(parts, skinned, cube("back.0.pack", (0, 0.32, 0.58), (0.3, 0.14, 0.34), pack, bevel=0.03, subdiv=1), "bone.spine")
    wear(parts, skinned, cube("back.0.pocket", (0, 0.4, 0.5), (0.16, 0.04, 0.1), pocket, bevel=0.01, subdiv=0), "bone.spine")
    wear(parts, skinned, cube("back.0.flap", (0, 0.36, 0.74), (0.22, 0.04, 0.06), pocket, bevel=0.008, subdiv=0), "bone.spine")
    wear(parts, skinned, cube("back.0.strap.L", (-0.1, 0.16, 0.72), (0.04, 0.08, 0.28), strap, bevel=0.006, subdiv=0), "bone.spine")
    wear(parts, skinned, cube("back.0.strap.R", (0.1, 0.16, 0.72), (0.04, 0.08, 0.28), strap, bevel=0.006, subdiv=0), "bone.spine")
    wear(parts, skinned, cube("back.0.buckle", (0, 0.2, 0.62), (0.06, 0.02, 0.04), gold, bevel=0.004, subdiv=0), "bone.spine")
    for label, x in (("L", -0.08), ("R", 0.08)):
        wear(parts, skinned, cyl(f"back.1.tank.{label}", (x, 0.3, 0.62), 0.05, 0.26, gold, vertices=12), "bone.spine")
        wear(parts, skinned, cyl(f"back.1.nozzle.{label}", (x, 0.3, 0.46), 0.02, 0.06, ridge, vertices=8), "bone.spine")
    wear(parts, skinned, cube("back.1.strap", (0, 0.18, 0.7), (0.22, 0.02, 0.04), strap, bevel=0.004, subdiv=0), "bone.spine")
    wear(parts, skinned, cube("back.2.cape", (0, 0.24, 0.48), (0.46, 0.02, 0.58), cloth, bevel=0.008, subdiv=0), "bone.spine")
    wear(parts, skinned, cube("back.2.fold", (0.08, 0.27, 0.4), (0.16, 0.025, 0.36), cloth_dark, bevel=0.006, subdiv=0, rot=(0, 0, 0.08)), "bone.spine")
    wear(parts, skinned, cube("back.2.collar", (0, 0.16, 0.78), (0.28, 0.04, 0.06), cloth, bevel=0.008, subdiv=0), "bone.spine")
    wear(parts, skinned, cube("back.3.bar", (0, 0.2, 0.9), (0.22, 0.03, 0.03), navy, bevel=0.006, subdiv=0), "bone.spine")
    for index, x in enumerate((-0.08, 0.0, 0.08)):
        wear(parts, skinned, cyl(f"back.3.mast.{index}", (x, 0.2, 1.02), 0.01, 0.16, navy, vertices=8), "bone.spine")
        wear(parts, skinned, ball(f"back.3.tip.{index}", (x, 0.2, 1.12), 0.022, cyan), "bone.spine")

    emblem_colors = (gold, cyan, hat, lime, crown_mat, cloth)
    for index, color in enumerate(emblem_colors):
        wear(
            parts,
            skinned,
            cyl(f"emblem.{index}.plate", (-0.12, front - 0.02, 0.4), 0.032, 0.01, color, vertices=6, rot=(math.pi / 2, 0, 0)),
            "bone.spine",
        )
        wear(
            parts,
            skinned,
            torus(f"emblem.{index}.rim", (-0.12, front - 0.028, 0.4), 0.032, 0.006, gold, rot=(math.pi / 2, 0, 0)),
            "bone.spine",
        )


def main():
    reset()
    arm = build_armature()
    parts = [arm]
    skinned = []
    front = build_body(arm, parts, skinned)
    build_roles(parts, skinned, front)
    for ob, bone in skinned:
        bind(ob, arm, bone)
    leg_axis, leg_sign = verify_leg_axis(arm)
    left_arm = verify_lift(arm, "bone.arm.L")
    right_arm = verify_lift(arm, "bone.arm.R")
    animate(arm, leg_axis, leg_sign, left_arm, right_arm)
    bpy.ops.object.select_all(action="DESELECT")
    for ob in parts:
        ob.hide_set(False)
        ob.select_set(True)
    bpy.context.view_layer.objects.active = arm
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=OUT,
        export_format="GLB",
        use_selection=True,
        export_apply=False,
        export_yup=True,
        export_animations=True,
        export_nla_strips=True,
        export_skins=True,
        export_materials="EXPORT",
    )
    print("wrote", OUT, os.path.getsize(OUT))


if __name__ == "__main__":
    main()
