"""Soft ingot body for fe2VI. One rig, idle / walk / celebrate / caught.

Does not touch station.glb. Role accessories are added by the same script
when ROLE_MESHES is filled; the base body is the shared mesh.
"""

import math
import os

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
    screen = material("Screen", (1.0, 0.79, 0.16), 0.05, 0.18, emission=2.2)
    bezel = material("Bezel", (0.05, 0.05, 0.06), 0.6, 0.25)
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
    bezel_mesh = cyl("body.bezel", (0, front - 0.012, 0.64), 0.25, 0.02, bezel, vertices=8, rot=(math.pi / 2, 0, math.pi / 8))
    screen_mesh = cyl("body.screen", (0, front - 0.028, 0.64), 0.22, 0.015, screen, vertices=8, rot=(math.pi / 2, 0, math.pi / 8))
    for ob in (bezel_mesh, screen_mesh):
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
    black = material("Knit", (0.04, 0.04, 0.05), 0.0, 0.85)
    gold = material("Gold", (0.85, 0.68, 0.22), 0.85, 0.28)
    navy = material("Hood", (0.08, 0.1, 0.14), 0.05, 0.8)
    cyan = material("Glyph", (0.18, 0.9, 0.9), 0.1, 0.3, emission=1.6)
    glass = material("Lens", (0.75, 0.86, 0.9), 0.15, 0.08)
    hat = material("Hat", (0.95, 0.62, 0.16), 0.15, 0.45)
    pack = material("Pack", (0.35, 0.22, 0.12), 0.05, 0.7)
    lime = material("Compass", (0.61, 1.0, 0.23), 0.2, 0.35, emission=0.4)

    wear(parts, skinned, ball("role.safecracker.beanie", (0, -0.02, 0.96), 0.2, black, scale=(1.35, 1.05, 0.55)), "bone.head")
    wear(parts, skinned, cyl("role.safecracker.keyhole", (0, front - 0.02, 0.9), 0.035, 0.012, gold, vertices=10, rot=(math.pi / 2, 0, 0)), "bone.head")

    wear(parts, skinned, cube("role.trader.band", (0, 0.0, 0.98), (0.46, 0.08, 0.05), gold, bevel=0.015, subdiv=0), "bone.head")
    wear(parts, skinned, cyl("role.trader.mic", (0.16, front - 0.02, 0.58), 0.012, 0.16, gold, vertices=6, rot=(math.pi / 2.4, 0, 0.4)), "bone.head")
    wear(parts, skinned, cube("role.trader.pad.L", (-0.36, -0.02, 0.74), (0.16, 0.16, 0.08), gold, bevel=0.02, subdiv=1), "bone.spine")
    wear(parts, skinned, cube("role.trader.pad.R", (0.36, -0.02, 0.74), (0.16, 0.16, 0.08), gold, bevel=0.02, subdiv=1), "bone.spine")

    wear(parts, skinned, ball("role.hacker.hood", (0, 0.06, 0.9), 0.26, navy, scale=(1.15, 1.05, 0.85)), "bone.head")
    wear(parts, skinned, cyl("role.hacker.string.L", (-0.06, front - 0.03, 0.55), 0.008, 0.22, navy, vertices=6), "bone.spine")
    wear(parts, skinned, cyl("role.hacker.string.R", (0.06, front - 0.03, 0.55), 0.008, 0.22, navy, vertices=6), "bone.spine")
    wear(parts, skinned, cube("role.hacker.glyph", (0, front - 0.03, 0.5), (0.08, 0.012, 0.08), cyan, bevel=0.01, subdiv=0), "bone.spine")

    wear(parts, skinned, cyl("role.auditor.glass", (0.34, front - 0.08, 0.4), 0.07, 0.015, glass, vertices=16, rot=(math.pi / 2, 0, 0)), "bone.arm.R")
    wear(parts, skinned, cyl("role.auditor.handle", (0.34, front - 0.05, 0.3), 0.012, 0.12, gold, vertices=6), "bone.arm.R")
    wear(parts, skinned, cyl("role.auditor.shield", (0.1, front - 0.03, 0.48), 0.05, 0.014, gold, vertices=6, rot=(math.pi / 2, 0, 0)), "bone.spine")

    wear(parts, skinned, ball("role.builder.hat", (0, -0.02, 0.98), 0.22, hat, scale=(1.4, 1.2, 0.55)), "bone.head")
    wear(parts, skinned, cube("role.builder.badge", (0, front - 0.03, 0.48), (0.12, 0.014, 0.08), hat, bevel=0.01, subdiv=0), "bone.spine")

    wear(parts, skinned, cyl("role.scout.goggle.L", (-0.08, front - 0.02, 0.86), 0.045, 0.02, lime, vertices=12, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, cyl("role.scout.goggle.R", (0.08, front - 0.02, 0.86), 0.045, 0.02, lime, vertices=12, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, cube("role.scout.pack", (0, 0.28, 0.58), (0.28, 0.14, 0.32), pack, bevel=0.03, subdiv=1), "bone.spine")
    wear(parts, skinned, cube("role.scout.strap", (0, 0.08, 0.7), (0.36, 0.03, 0.04), pack, bevel=0.008, subdiv=0), "bone.spine")
    wear(parts, skinned, cyl("role.scout.compass", (0, 0.36, 0.62), 0.04, 0.012, lime, vertices=12, rot=(math.pi / 2, 0, 0)), "bone.spine")

    crown = material("Crown", (0.92, 0.76, 0.28), 0.8, 0.25)
    cloth = material("Cape", (0.45, 0.12, 0.14), 0.05, 0.75)
    wear(parts, skinned, cyl("headwear.crown.band", (0, -0.02, 0.98), 0.2, 0.05, crown, vertices=8), "bone.head")
    for index, x in enumerate((-0.1, 0.0, 0.1)):
        wear(parts, skinned, cube(f"headwear.crown.point.{index}", (x, -0.02, 1.06), (0.04, 0.04, 0.08), crown, bevel=0.01, subdiv=0), "bone.head")
    wear(parts, skinned, ball("headwear.cap", (0, -0.04, 0.96), 0.18, hat, scale=(1.3, 1.15, 0.5)), "bone.head")
    wear(parts, skinned, cube("headwear.brim", (0, front - 0.02, 0.9), (0.28, 0.12, 0.025), hat, bevel=0.008, subdiv=0), "bone.head")

    wear(parts, skinned, cube("faceitem.0.visor", (0, front - 0.045, 0.78), (0.28, 0.02, 0.035), black, bevel=0.004, subdiv=0), "bone.head")
    wear(parts, skinned, cyl("faceitem.1.monocle", (0.09, front - 0.045, 0.68), 0.04, 0.012, gold, vertices=12, rot=(math.pi / 2, 0, 0)), "bone.head")
    wear(parts, skinned, cube("faceitem.2.mask", (0, front - 0.04, 0.52), (0.22, 0.02, 0.08), navy, bevel=0.008, subdiv=0), "bone.head")
    wear(parts, skinned, cyl("faceitem.3.mic", (-0.16, front - 0.02, 0.56), 0.012, 0.16, gold, vertices=6, rot=(math.pi / 2.4, 0, -0.4)), "bone.head")

    wear(parts, skinned, cube("back.0.pack", (0, 0.3, 0.58), (0.26, 0.12, 0.3), pack, bevel=0.03, subdiv=1), "bone.spine")
    wear(parts, skinned, cyl("back.1.left", (-0.08, 0.28, 0.62), 0.05, 0.22, gold, vertices=10), "bone.spine")
    wear(parts, skinned, cyl("back.1.right", (0.08, 0.28, 0.62), 0.05, 0.22, gold, vertices=10), "bone.spine")
    wear(parts, skinned, cube("back.2.cape", (0, 0.22, 0.5), (0.42, 0.02, 0.55), cloth, bevel=0.01, subdiv=0), "bone.spine")
    wear(parts, skinned, cyl("back.3.mast", (0, 0.16, 0.95), 0.012, 0.18, navy, vertices=6), "bone.spine")
    wear(parts, skinned, ball("back.3.tip", (0, 0.16, 1.06), 0.03, cyan), "bone.spine")

    emblem_colors = (gold, cyan, hat, lime, crown, cloth)
    for index, color in enumerate(emblem_colors):
        wear(
            parts,
            skinned,
            cyl(f"emblem.{index}", (-0.12, front - 0.02, 0.42), 0.035, 0.012, color, vertices=6 + index, rot=(math.pi / 2, 0, 0)),
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
