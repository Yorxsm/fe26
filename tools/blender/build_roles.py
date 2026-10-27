"""Six original ingot bodies. One rig, idle / walk / celebrate. Does not touch station.glb."""

import math
import os

import bpy

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "apps", "web", "public", "models", "agent.glb")


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, color, metallic=0.0, roughness=0.55, emission=0.0):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = next(node for node in mat.node_tree.nodes if node.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Base Color"].default_value = (*color, 1)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    if emission and "Emission Strength" in bsdf.inputs:
        bsdf.inputs["Emission Color"].default_value = (*color, 1)
        bsdf.inputs["Emission Strength"].default_value = emission
    return mat


def apply_xform(ob):
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)


def box(name, loc, size, mat, bevel=0.04):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc)
    ob = bpy.context.active_object
    ob.name = name
    ob.dimensions = size
    apply_xform(ob)
    ob.data.materials.append(mat)
    if bevel > 0:
        mod = ob.modifiers.new("Bevel", "BEVEL")
        mod.width = bevel
        mod.segments = 1
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return ob


def cyl(name, loc, radius, depth, mat, vertices=8, rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=depth, location=loc)
    ob = bpy.context.active_object
    ob.name = name
    ob.rotation_euler = rot
    apply_xform(ob)
    ob.data.materials.append(mat)
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
    body.tail = (0, 0, 1.25)
    body.parent = root
    for label, x in (("L", -0.18), ("R", 0.18)):
        leg = bones.new(f"bone.leg.{label}")
        leg.head = (x, 0, 0.38)
        leg.tail = (x, 0, 0.02)
        leg.parent = root
    for label, x in (("L", -0.46), ("R", 0.46)):
        limb = bones.new(f"bone.arm.{label}")
        limb.head = (x, 0, 1.05)
        limb.tail = (x, 0, 0.55)
        limb.parent = body
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


def animate(arm):
    bpy.context.scene.frame_start = 1
    arm.animation_data_create()
    arm.animation_data.action = bpy.data.actions.new("idle")
    for frame, lift in ((1, 0.0), (16, 0.045), (32, 0.0), (48, 0.045), (64, 0.0)):
        key_loc(arm, "bone.root", frame, (0, lift, 0))
        key_rot(arm, "bone.body", frame, (0, 0, 0))
    stash_action(arm, "idle")

    arm.animation_data.action = bpy.data.actions.new("walk")
    for frame, swing in ((1, 0.5), (8, -0.5), (16, 0.5)):
        key_loc(arm, "bone.root", frame, (0, 0.04 if frame == 8 else 0.0, 0))
        key_rot(arm, "bone.leg.L", frame, (swing, 0, 0))
        key_rot(arm, "bone.leg.R", frame, (-swing, 0, 0))
        key_rot(arm, "bone.arm.L", frame, (-swing, 0, 0))
        key_rot(arm, "bone.arm.R", frame, (swing, 0, 0))
        key_rot(arm, "bone.body", frame, (0.08 if swing > 0 else -0.08, 0, 0))
    stash_action(arm, "walk")

    arm.animation_data.action = bpy.data.actions.new("celebrate")
    for frame, lift, twist in ((1, 0.0, 0.0), (8, 0.18, 0.3), (16, 0.02, -0.22), (24, 0.16, 0.22), (32, 0.0, 0.0)):
        key_loc(arm, "bone.root", frame, (0, lift, 0))
        key_rot(arm, "bone.body", frame, (0, 0, twist))
        key_rot(arm, "bone.arm.L", frame, (-1.2 if lift > 0.05 else 0.0, 0, 0.35))
        key_rot(arm, "bone.arm.R", frame, (-1.2 if lift > 0.05 else 0.0, 0, -0.35))
    stash_action(arm, "celebrate")


def add_base(prefix, body_mat, arm, parts, skinned):
    body = box(f"{prefix}.body", (0, 0, 0.82), (0.72, 0.5, 0.86), body_mat, 0.08)
    leg_l = box(f"{prefix}.leg.L", (-0.18, 0, 0.2), (0.18, 0.18, 0.36), body_mat, 0.03)
    leg_r = box(f"{prefix}.leg.R", (0.18, 0, 0.2), (0.18, 0.18, 0.36), body_mat, 0.03)
    arm_l = box(f"{prefix}.arm.L", (-0.48, 0, 0.78), (0.14, 0.14, 0.5), body_mat, 0.02)
    arm_r = box(f"{prefix}.arm.R", (0.48, 0, 0.78), (0.14, 0.14, 0.5), body_mat, 0.02)
    for ob, bone in (
        (body, "bone.body"),
        (leg_l, "bone.leg.L"),
        (leg_r, "bone.leg.R"),
        (arm_l, "bone.arm.L"),
        (arm_r, "bone.arm.R"),
    ):
        skinned.append((ob, bone))
        parts.append(ob)


def add_face(prefix, screen, dark, arm_unused, parts, skinned):
    del arm_unused
    face_rot = (math.pi / 2, 0, math.radians(30))
    plates = [
        (0, (1.0, 1.0, 1.0), screen, 0.0),
        (1, (1.05, 1.0, 1.12), screen, 0.012),
        (2, (1.2, 1.0, 0.72), screen, 0.024),
        (3, (0.62, 1.0, 0.32), screen, 0.036),
    ]
    for index, scale, mat, push in plates:
        ob = cyl(
            f"{prefix}.face.{index}",
            (0, -0.3 - push, 0.9),
            0.16,
            0.04,
            mat,
            vertices=6,
            rot=face_rot,
        )
        ob.scale = scale
        apply_xform(ob)
        skinned.append((ob, "bone.body"))
        parts.append(ob)
    for label, x in (("L", -0.1), ("R", 0.1)):
        eye = box(f"{prefix}.eye.{label}", (x, -0.34, 0.92), (0.07, 0.03, 0.08), dark, 0)
        skinned.append((eye, "bone.body"))
        parts.append(eye)


def build_roles(arm, parts, skinned):
    grey = material("Body", (0.60, 0.63, 0.65))
    black = material("Cloth", (0.08, 0.08, 0.09), roughness=0.8)
    yellow = material("Screen", (0.95, 0.75, 0.12), emission=0.35)
    purple = material("Body", (0.48, 0.31, 0.77))
    magenta = material("Screen", (0.85, 0.25, 0.62), emission=0.25)
    gold = material("Trim", (0.85, 0.62, 0.18), metallic=0.85, roughness=0.28)
    navy = material("Body", (0.11, 0.16, 0.23))
    cyan = material("Screen", (0.25, 0.85, 0.9), emission=0.4)
    cream = material("Body", (0.91, 0.89, 0.86))
    orange = material("Body", (0.88, 0.45, 0.16))
    hat_yellow = material("Hat", (0.93, 0.75, 0.18), roughness=0.4)
    olive = material("Body", (0.42, 0.48, 0.23))
    lime = material("Screen", (0.65, 0.85, 0.22), emission=0.2)
    iron = material("Iron", (0.15, 0.16, 0.18), metallic=0.6, roughness=0.35)

    add_base("role.safecracker", grey, arm, parts, skinned)
    add_face("role.safecracker", yellow, black, arm, parts, skinned)
    for ob in (
        box("role.safecracker.beanie", (0, 0.02, 1.32), (0.5, 0.46, 0.28), black, 0.05),
        box("role.safecracker.keyhole", (0, -0.22, 1.34), (0.08, 0.04, 0.12), yellow, 0),
    ):
        skinned.append((ob, "bone.body"))
        parts.append(ob)

    add_base("role.trader", purple, arm, parts, skinned)
    add_face("role.trader", magenta, black, arm, parts, skinned)
    for ob in (
        box("role.trader.trim", (0, 0, 0.7), (0.78, 0.54, 0.08), gold, 0.01),
        box("role.trader.can.L", (-0.42, 0, 0.95), (0.12, 0.18, 0.2), iron, 0.02),
        box("role.trader.can.R", (0.42, 0, 0.95), (0.12, 0.18, 0.2), iron, 0.02),
        box("role.trader.band", (0, 0.04, 1.22), (0.78, 0.08, 0.08), gold, 0),
        box("role.trader.mic", (0.16, -0.34, 0.72), (0.04, 0.16, 0.04), gold, 0),
    ):
        skinned.append((ob, "bone.body"))
        parts.append(ob)

    add_base("role.hacker", navy, arm, parts, skinned)
    add_face("role.hacker", cyan, black, arm, parts, skinned)
    for ob in (
        box("role.hacker.hood", (0, 0.06, 1.22), (0.86, 0.62, 0.55), navy, 0.05),
        box("role.hacker.string.L", (-0.1, -0.28, 0.7), (0.03, 0.03, 0.28), cyan, 0),
        box("role.hacker.string.R", (0.1, -0.28, 0.7), (0.03, 0.03, 0.28), cyan, 0),
    ):
        skinned.append((ob, "bone.body"))
        parts.append(ob)

    add_base("role.auditor", cream, arm, parts, skinned)
    add_face("role.auditor", yellow, black, arm, parts, skinned)
    lens = cyl("role.auditor.lens", (0.34, -0.36, 0.98), 0.12, 0.03, yellow, vertices=12, rot=(math.pi / 2, 0, 0))
    for ob in (
        box("role.auditor.badge", (0.18, -0.28, 0.7), (0.12, 0.04, 0.14), gold, 0.01),
        lens,
        box("role.auditor.handle", (0.5, -0.22, 0.78), (0.05, 0.05, 0.32), gold, 0),
    ):
        skinned.append((ob, "bone.body"))
        parts.append(ob)

    add_base("role.builder", orange, arm, parts, skinned)
    add_face("role.builder", hat_yellow, black, arm, parts, skinned)
    dome = cyl("role.builder.dome", (0, 0, 1.34), 0.28, 0.22, hat_yellow, vertices=10)
    for ob in (
        box("role.builder.brim", (0, 0, 1.24), (0.84, 0.62, 0.08), hat_yellow, 0.02),
        dome,
    ):
        skinned.append((ob, "bone.body"))
        parts.append(ob)

    add_base("role.scout", olive, arm, parts, skinned)
    add_face("role.scout", lime, black, arm, parts, skinned)
    for ob in (
        cyl("role.scout.goggle.L", (-0.14, -0.34, 1.0), 0.09, 0.05, lime, vertices=10, rot=(math.pi / 2, 0, 0)),
        cyl("role.scout.goggle.R", (0.14, -0.34, 1.0), 0.09, 0.05, lime, vertices=10, rot=(math.pi / 2, 0, 0)),
        box("role.scout.strap", (0, 0.02, 1.0), (0.62, 0.08, 0.06), black, 0),
        box("role.scout.pack", (-0.5, 0.08, 0.62), (0.26, 0.28, 0.4), olive, 0.03),
        box("role.scout.buckle", (-0.5, -0.08, 0.66), (0.1, 0.04, 0.1), lime, 0),
    ):
        skinned.append((ob, "bone.body"))
        parts.append(ob)


def main():
    reset()
    arm = build_armature()
    parts = [arm]
    skinned = []
    build_roles(arm, parts, skinned)
    for ob, bone in skinned:
        bind(ob, arm, bone)
    animate(arm)
    bpy.ops.object.select_all(action="DESELECT")
    for ob in parts:
        ob.hide_set(False)
        ob.select_set(True)
    bpy.context.view_layer.objects.active = arm
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
