"""Dress the five arena interiors with CC0 props.

The shell, vault door, lasers, columns, and trophy/mannequins are original.
Props come from Kenney (Furniture, Survival, Racing), the Quaternius Furniture
Pack, and Poly Haven meshes. Materials are recolored to the dusk palette.
Repeats stay linked duplicates so the glTF export can GPU-instance them.
One-off props that share a material are joined.

Sources are extracted under $CC0_PROPS (default /tmp/cc0src/props) and
$CC0_POLY (default /tmp/cc0src/poly). See docs/ASSETS.md.
"""

import math
import os
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(__file__))
import build_interiors as bi

ROOT = bi.ROOT
OUT_DIR = bi.OUT_DIR
PROPS = os.environ.get("CC0_PROPS", "/tmp/cc0src/props")
POLY = os.environ.get("CC0_POLY", "/tmp/cc0src/poly")
WOOD_SRC = os.path.join(ROOT, "apps", "web", "public", "textures", "wood-albedo.jpg")

PALETTE = {
    "wall": ((0.32, 0.26, 0.2), 0.04, 0.72),
    "floor": ((0.22, 0.2, 0.18), 0.08, 0.55),
    "trim": ((0.4, 0.36, 0.3), 0.35, 0.42),
    "metal": ((0.28, 0.29, 0.31), 0.84, 0.36),
    "gold": ((0.62, 0.46, 0.18), 0.92, 0.32),
    "wood": ((0.36, 0.24, 0.14), 0.02, 0.68),
    "cloth": ((0.24, 0.2, 0.18), 0.0, 0.84),
    "plant": ((0.14, 0.32, 0.2), 0.0, 0.78),
    "glow": ((0.72, 0.42, 0.16), 0.0, 0.45),
    "laser": ((0.55, 0.02, 0.03), 0.0, 0.35),
    "glass": ((0.08, 0.09, 0.1), 0.15, 0.18),
    "screen": ((0.08, 0.08, 0.09), 0.4, 0.45),
}


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def pbr(name, color, metallic, roughness, emission=None, strength=0.0, image=None):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = next(node for node in mat.node_tree.nodes if node.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Base Color"].default_value = (*color, 1)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    if emission is not None:
        bsdf.inputs["Emission Color"].default_value = (*emission, 1)
        bsdf.inputs["Emission Strength"].default_value = strength
    if image is not None:
        tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
        tex.image = image
        mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    return mat


def graded_wood():
    image = bpy.data.images.load(WOOD_SRC)
    image.name = "wood-dusk"
    image.scale(512, 512)
    pixels = list(image.pixels)
    for index in range(0, len(pixels), 4):
        pixels[index] *= 0.42
        pixels[index + 1] *= 0.3
        pixels[index + 2] *= 0.22
    image.pixels = pixels
    image.pack()
    return image


def make_palette():
    wood = graded_wood() if os.path.exists(WOOD_SRC) else None
    mats = {}
    for name, (color, metal, rough) in PALETTE.items():
        image = wood if name == "wood" else None
        emission = None
        strength = 0.0
        if name == "laser":
            emission = (0.85, 0.02, 0.03)
            strength = 4.0
        elif name == "glow":
            emission = (0.79, 0.29, 0.1)
            strength = 0.35
        mats[name] = pbr(name, color, metal, rough, emission, strength, image)
    return mats


def import_mesh(path):
    before = set(bpy.data.objects)
    if path.lower().endswith(".fbx"):
        bpy.ops.preferences.addon_enable(module="io_scene_fbx")
        bpy.ops.import_scene.fbx(filepath=path)
    else:
        bpy.ops.import_scene.gltf(filepath=path)
    created = [ob for ob in bpy.data.objects if ob not in before]
    for extra in [ob for ob in created if ob.type != "MESH"]:
        bpy.data.objects.remove(extra, do_unlink=True)
    meshes = [ob for ob in bpy.data.objects if ob not in before and ob.type == "MESH"]
    if not meshes:
        raise SystemExit(f"no mesh in {path}")
    bpy.ops.object.select_all(action="DESELECT")
    for ob in meshes:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if len(meshes) > 1:
        bpy.ops.object.join()
    return bpy.context.view_layer.objects.active


def assign(ob, mat):
    ob.data.materials.clear()
    ob.data.materials.append(mat)


def decimate(ob, limit):
    faces = len(ob.data.polygons)
    if faces <= limit:
        return faces
    mod = ob.modifiers.new("decimate", "DECIMATE")
    mod.ratio = limit / faces
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.modifier_apply(modifier=mod.name)
    return len(ob.data.polygons)


def fit(ob, height=None, wide=None, deep=None, long=None):
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if long:
        span = max(ob.dimensions.x, ob.dimensions.y) or 1
        scale = long / span
        ob.scale = (scale, scale, scale)
        bpy.ops.object.transform_apply(scale=True)
    if height:
        ob.scale = (height / (ob.dimensions.z or 1),) * 3
        bpy.ops.object.transform_apply(scale=True)
    if wide:
        ob.scale = (wide / (ob.dimensions.x or 1), 1, 1)
        bpy.ops.object.transform_apply(scale=True)
    if deep:
        ob.scale = (1, deep / (ob.dimensions.y or 1), 1)
        bpy.ops.object.transform_apply(scale=True)


def unlink(ob):
    for collection in list(ob.users_collection):
        collection.objects.unlink(ob)


def prepare(path, mat, limit=4000, height=None, wide=None, deep=None, long=None):
    ob = import_mesh(path)
    assign(ob, mat)
    faces = decimate(ob, limit)
    fit(ob, height=height, wide=wide, deep=deep, long=long)
    ob["proto"] = True
    unlink(ob)
    print(f"proto {os.path.basename(path)} faces {faces} size {tuple(round(v, 2) for v in ob.dimensions)}")
    return ob


def spawn(proto, x, y, z, yaw=0.0):
    ob = proto.copy()
    ob.data = proto.data
    bpy.context.collection.objects.link(ob)
    ob.location = (x, -z, y)
    ob.rotation_euler = (0, 0, yaw)
    ob["proto"] = False
    return ob


def make_ingot(mat):
    mesh = bpy.data.meshes.new("ingot")
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for vert in bm.verts:
        if vert.co.z > 0:
            vert.co.x *= 0.62
            vert.co.y *= 0.62
        vert.co.x *= 0.32
        vert.co.y *= 0.14
        vert.co.z *= 0.1
    bm.to_mesh(mesh)
    bm.free()
    ob = bpy.data.objects.new("proto.ingot", mesh)
    assign(ob, mat)
    ob["proto"] = True
    return ob


def make_block(name, mat, size):
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    sx, sy, sz = size
    for vert in bm.verts:
        vert.co.x *= sx
        vert.co.y *= sy
        vert.co.z *= sz
    bm.to_mesh(mesh)
    bm.free()
    ob = bpy.data.objects.new(name, mesh)
    assign(ob, mat)
    ob["proto"] = True
    return ob


def make_mannequin(cloth, metal):
    body = make_block("mannequin.body", cloth, (0.28, 0.16, 0.46))
    head = make_block("mannequin.head", metal, (0.14, 0.12, 0.14))
    # Join in a throwaway scene copy so the prototype is one mesh.
    bpy.context.collection.objects.link(body)
    bpy.context.collection.objects.link(head)
    body.location = (0, 0, 0.42)
    head.location = (0, 0, 0.74)
    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    head.select_set(True)
    bpy.context.view_layer.objects.active = body
    bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    bpy.ops.object.transform_apply(location=True)
    ob["proto"] = True
    unlink(ob)
    return ob


def make_trophy(gold, metal):
    bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.16, depth=0.06, location=(0, 0, 0.03))
    base = bpy.context.active_object
    assign(base, metal)
    bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=0.05, depth=0.16, location=(0, 0, 0.14))
    stem = bpy.context.active_object
    assign(stem, gold)
    bpy.ops.mesh.primitive_cone_add(vertices=12, radius1=0.12, radius2=0.07, depth=0.16, location=(0, 0, 0.3))
    cup = bpy.context.active_object
    assign(cup, gold)
    bpy.ops.object.select_all(action="DESELECT")
    for ob in (base, stem, cup):
        ob.select_set(True)
    bpy.context.view_layer.objects.active = cup
    bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    bpy.ops.object.transform_apply(location=True)
    assign(ob, gold)
    ob["proto"] = True
    unlink(ob)
    return ob


def merge_static():
    users = {}
    for ob in bpy.data.objects:
        if ob.type != "MESH" or ob.get("proto"):
            continue
        users.setdefault(ob.data, []).append(ob)
    buckets = {}
    for mesh, group in users.items():
        if len(group) >= 2:
            continue
        mat = group[0].data.materials[0].name if group[0].data.materials else "none"
        buckets.setdefault(mat, []).extend(group)
    for mat, group in buckets.items():
        if len(group) < 2:
            continue
        bpy.ops.object.select_all(action="DESELECT")
        for ob in group:
            ob.select_set(True)
        bpy.context.view_layer.objects.active = group[0]
        joined = bpy.ops.object.join()
        print("join", mat, joined, "->", bpy.context.view_layer.objects.active.name if bpy.context.view_layer.objects.active else None)


def export_room(filename):
    path = os.path.join(OUT_DIR, filename)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=False,
        export_yup=True,
        export_animations=False,
        export_skins=False,
        export_morph=False,
        export_materials="EXPORT",
        export_gpu_instances=True,
        export_image_format="WEBP",
        export_apply=False,
        export_draco_mesh_compression_enable=False,
    )
    print("wrote", filename, os.path.getsize(path))


def clear_placed():
    for ob in list(bpy.data.objects):
        if ob.get("proto"):
            continue
        bpy.data.objects.remove(ob, do_unlink=True)


def vault_door(room):
    room.cyl("metal", (0, 1.9, -5.68), 1.48, 0.28, axis="z", segments=28)
    room.cyl("gold", (0, 1.9, -5.5), 1.18, 0.06, axis="z", segments=28)
    room.cyl("metal", (0, 1.9, -5.44), 0.78, 0.08, axis="z", segments=20)
    room.box("metal", (0, 1.9, -5.38), (1.55, 0.06, 0.05))
    room.box("metal", (0, 1.9, -5.38), (0.06, 1.55, 0.05))
    room.cyl("gold", (0, 1.9, -5.32), 0.2, 0.08, axis="z", segments=12)
    for bolt_y in (1.15, 2.65):
        room.cyl("metal", (-1.62, bolt_y, -5.55), 0.07, 0.32, axis="y", segments=8)
        room.box("metal", (-1.78, bolt_y, -5.55), (0.18, 0.22, 0.16))
    for index in range(12):
        angle = index * math.pi / 6
        room.ball("gold", (math.cos(angle) * 1.32, 1.9 + math.sin(angle) * 1.32, -5.52), 0.05, segments=6)


def lasers(room):
    for height in (0.9, 1.35, 1.8):
        room.box("laser", (0.1, height, -1.6), (4.2, 0.008, 0.008))


def shell_for(kind):
    room = bi.Room()
    for name, (color, metal, rough) in PALETTE.items():
        emission = None
        strength = 0.0
        if name == "laser":
            emission, strength = (0.85, 0.02, 0.03), 4.0
        elif name == "glow":
            emission, strength = (0.79, 0.29, 0.1), 0.35
        room.mat(name, color, metal, rough, emission, strength)
    ceiling = "metal" if kind in {"war", "stadium"} else "trim"
    bi.shell(room, "wall", "floor", "trim", ceiling)
    bi.dress(room)
    if kind == "vault":
        vault_door(room)
        lasers(room)
    if kind == "war":
        room.box("glow", (0, 0.82, -0.4), (1.35, 0.03, 1.35))
    if kind == "stadium":
        for x in (-3.4, 3.4):
            room.cone("metal", (x, 3.55, -1.4), 0.32, 0.08, 0.55, axis="y", segments=10)
            room.ball("glow", (x, 3.22, -1.4), 0.07, segments=6)
    return room


def prop(name):
    path = os.path.join(PROPS, name)
    if not os.path.exists(path):
        raise SystemExit(path)
    return path


def poly(name):
    folder = os.path.join(POLY, name)
    gltfs = [os.path.join(folder, entry) for entry in os.listdir(folder) if entry.endswith(".gltf")]
    if not gltfs:
        raise SystemExit(folder)
    return gltfs[0]


def build():
    reset()
    mats = make_palette()
    cache = {}

    def proto(key, path, role, limit=4000, height=None, wide=None, deep=None, long=None):
        if key not in cache:
            cache[key] = prepare(path, mats[role], limit, height, wide, deep, long)
        return cache[key]

    ingot = make_ingot(mats["gold"])
    mannequin = make_mannequin(mats["cloth"], mats["metal"])
    trophy = make_trophy(mats["gold"], mats["metal"])

    desk = proto("desk", prop("desk.glb"), "wood", height=0.76)
    chair = proto("chair", prop("chairDesk.glb"), "cloth", height=0.92)
    q_chair = proto("qchair", prop("q-Chair.fbx"), "cloth", height=0.9)
    q_sofa = proto("qsofa", prop("loungeSofa.glb"), "cloth", height=0.7, wide=2.15, deep=0.82)
    q_plant = proto("qplant", prop("q-Plant.fbx"), "plant", height=1.15)
    q_case = proto("qcase", prop("q-BookCase.fbx"), "wood", height=1.7)
    q_table = proto("qtable", prop("q-Table.fbx"), "wood", height=0.5)
    screen = proto("screen", prop("computerScreen.glb"), "glass", height=0.38)
    keys = proto("keys", prop("computerKeyboard.glb"), "metal", height=0.04)
    mouse = proto("mouse", prop("computerMouse.glb"), "metal", height=0.03)
    crate = proto("crate", prop("cardboardBoxClosed.glb"), "wood", height=0.42)
    crate_open = proto("crate-open", prop("cardboardBoxOpen.glb"), "wood", height=0.36)
    chest = proto("chest", prop("chest.glb"), "wood", height=0.48)
    barrel = proto("barrel", prop("barrel.glb"), "wood", height=0.7)
    pallet = proto("pallet", prop("resource-planks.glb"), "wood", wide=1.25, deep=0.72, height=0.1)
    locker = proto("locker", prop("bookcaseClosed.glb"), "metal", height=0.58, wide=0.44, deep=0.26)
    plant = proto("plant", prop("pottedPlant.glb"), "plant", height=1.15)
    rug = proto("rug", prop("rugRectangle.glb"), "cloth", wide=3.4, deep=2.2, height=0.03)
    bar = proto("bar", prop("kitchenBar.glb"), "wood", height=1.05)
    coffee = proto("coffee", prop("kitchenCoffeeMachine.glb"), "metal", height=0.32)
    stool = proto("stool", prop("stoolBar.glb"), "metal", height=0.72)
    table = proto("table", prop("tableGlass.glb"), "glass", height=0.74)
    side = proto("side", prop("sideTable.glb"), "wood", height=0.52)
    bench = proto("bench", prop("bench.glb"), "wood", height=0.42, wide=0.72, deep=0.38)
    lamp = proto("lamp", prop("lampSquareFloor.glb"), "metal", height=1.7)
    door = proto("door", prop("doorwayOpen.glb"), "trim", height=2.05, wide=1.35)
    speaker = proto("speaker", prop("speakerSmall.glb"), "metal", height=0.28)
    books = proto("books", prop("books.glb"), "cloth", height=0.08)
    barrier = proto("barrier", prop("barrierWhite.glb"), "metal", height=0.95)
    flag = proto("flag", prop("flagRed.glb"), "cloth", height=1.35)
    camera = proto("camera", poly("security_camera_01"), "metal", limit=2800, height=0.26)
    rack = proto("rack", poly("worn_metal_rack"), "metal", limit=3200, height=1.85)
    cart = proto("cart", poly("tool_cart"), "metal", limit=3500, height=0.95)
    cable = proto("cable", poly("modular_electric_cables"), "metal", limit=1800, wide=1.4, deep=0.35, height=0.08)

    def gold_stack(x, z):
        spawn(pallet, x, 0, z)
        for layer in range(3):
            for col in range(3):
                spawn(ingot, x - 0.28 + col * 0.28, 0.18 + layer * 0.12, z, yaw=0.04 * layer)

    def room_vault():
        for x in (-3.5, 0.4, 3.2):
            gold_stack(x, -3.5)
        # North wall, flanking the door, under the boards. yaw 0 faces the camera.
        for side in (-1, 1):
            for index in range(9):
                x = side * (1.9 + index * 0.48)
                spawn(locker, x, 0, -5.72)
                spawn(locker, x, 0.6, -5.72)
        spawn(desk, 4.7, 0, 1.5, yaw=math.pi)
        spawn(chair, 4.7, 0, 2.45, yaw=0)
        spawn(screen, 4.7, 0.78, 1.25, yaw=math.pi)
        spawn(keys, 4.55, 0.78, 1.6)
        spawn(books, 5.05, 0.78, 1.55)
        spawn(cart, -4.7, 0, 1.6, yaw=0.4)
        spawn(crate, 5.3, 0, -0.6)
        spawn(crate, 4.5, 0.42, -0.45)
        spawn(crate_open, -5.2, 0, 3.4)
        spawn(chest, -3.6, 0, 3.6)
        spawn(barrel, 2.4, 0, 3.5)
        spawn(plant, -5.8, 0, -4.6)
        spawn(plant, 6.0, 0, 4.2)
        spawn(rug, 0.2, 0, 0.6)
        for x, z in ((-6.2, -4.8), (6.2, -4.8), (-6.2, 4.0)):
            spawn(camera, x, 3.35, z, yaw=0.6 if x < 0 else -0.6)
        for height in (0.9, 1.35, 1.8):
            spawn(speaker, -2.35, height, -1.6, yaw=math.pi / 2)
            spawn(speaker, 2.35, height, -1.6, yaw=-math.pi / 2)

    def room_market():
        for index, x in enumerate((-3.8, 0.0, 3.6)):
            spawn(desk, x, 0, -2.7, yaw=math.pi)
            spawn(chair, x, 0, -1.7)
            spawn(screen, x - 0.28, 0.78, -2.95, yaw=math.pi)
            spawn(screen, x + 0.32, 0.78, -2.95, yaw=math.pi)
            spawn(keys, x, 0.78, -2.5)
            spawn(mouse, x + 0.34, 0.78, -2.45)
        spawn(bar, -5.3, 0, 3.2, yaw=math.pi / 2)
        spawn(coffee, -5.3, 1.02, 3.0)
        spawn(stool, -4.2, 0, 2.6)
        spawn(stool, -4.2, 0, 3.7, yaw=0.4)
        spawn(rack, 6.25, 0, -1.2, yaw=math.pi / 2)
        spawn(rack, 6.25, 0, -3.3, yaw=math.pi / 2)
        spawn(q_plant, -6.0, 0, -4.4)
        spawn(plant, 5.6, 0, 4.0)
        spawn(q_plant, 0.4, 0, 3.6)
        spawn(crate, -5.6, 0, -1.2)
        spawn(rug, 0, 0, 0.4)
        spawn(lamp, -6.1, 0, 0.4)

    def room_war():
        spawn(rack, -6.2, 0, -1.4, yaw=-math.pi / 2)
        spawn(rack, -6.2, 0, 1.2, yaw=-math.pi / 2)
        spawn(rack, 6.2, 0, -2.4, yaw=math.pi / 2)
        spawn(desk, -3.4, 0, 1.8, yaw=math.pi)
        spawn(desk, 3.2, 0, 1.6, yaw=math.pi)
        spawn(chair, -3.4, 0, 2.8)
        spawn(q_chair, 3.2, 0, 2.6)
        spawn(screen, -3.6, 0.78, 1.5, yaw=math.pi)
        spawn(screen, -3.0, 0.78, 1.5, yaw=math.pi)
        spawn(screen, 3.2, 0.78, 1.3, yaw=math.pi)
        spawn(keys, -3.3, 0.78, 1.9)
        spawn(table, 0, 0, -0.5)
        spawn(cable, -4.6, 0, -0.2, yaw=0.4)
        spawn(cable, 4.8, 0, 0.4, yaw=-0.5)
        for x in (-4.4, 0.0, 4.4):
            spawn(side, x, 0, -4.55)
            spawn(flag, x, 0.52, -4.55)
        spawn(q_plant, 6.0, 0, 4.0)
        spawn(chest, -5.4, 0, 3.6)

    def room_lobby():
        spawn(desk, 0, 0, 1.7, yaw=math.pi)
        spawn(q_chair, 0.7, 0, 2.5, yaw=0)
        spawn(q_sofa, -2.4, 0, -0.6, yaw=math.pi)
        spawn(q_sofa, 2.4, 0, -0.6, yaw=0)
        spawn(q_table, 0.1, 0, -0.5)
        spawn(bench, -4.6, 0, -2.6, yaw=math.pi / 2)
        spawn(bench, 1.2, 0, 2.2, yaw=math.pi / 2)
        spawn(rug, 0, 0, -0.7)
        spawn(rug, 3.6, 0, -3.3, yaw=math.pi / 2)
        spawn(q_plant, -4.6, 0, -2.4)
        spawn(plant, 2.0, 0, 1.6)
        spawn(q_case, 5.3, 0, -4.5, yaw=-math.pi / 2)
        for z in (-4.05, -3.2, -2.35):
            spawn(side, 3.7, 0, z)
            spawn(mannequin, 3.7, 0.52, z, yaw=-0.4)
        for x in (-4.7, -1.6, 1.6, 4.7):
            spawn(door, x, 0, -5.75)
        spawn(lamp, -5.2, 0, -3.6)
        spawn(books, 0.25, 0.78, 1.45)

    def room_stadium():
        spawn(side, 0, 0, -3.4)
        spawn(trophy, 0, 0.55, -3.4)
        for x in (-1.6, 1.6):
            spawn(barrier, x, 0, -2.2, yaw=math.pi / 2)
        spawn(barrier, 0, 0, -1.7)
        for row, z in enumerate((0.4, 1.15, 1.9)):
            for seat in range(8):
                spawn(bench, -2.8 + seat * 0.8, 0, z, yaw=math.pi)
        spawn(lamp, -5.8, 0, -4.2)
        spawn(lamp, 5.8, 0, -4.2)
        spawn(flag, -2.2, 0, -4.4)
        spawn(flag, 2.2, 0, -4.4)
        spawn(q_plant, -6.1, 0, 4.2)
        spawn(plant, 6.1, 0, 4.2)
        spawn(barrier, -5.6, 0, 1.2, yaw=math.pi / 2)
        spawn(barrier, 5.6, 0, 1.2, yaw=math.pi / 2)

    rooms = {
        "vault": ("interior-vault.glb", room_vault),
        "market": ("interior-market.glb", room_market),
        "war": ("interior-war.glb", room_war),
        "lobby": ("interior-forge.glb", room_lobby),
        "stadium": ("interior-stadium.glb", room_stadium),
    }
    for kind, (filename, place) in rooms.items():
        clear_placed()
        shell = shell_for(kind)
        shell.finish(f"room.{kind}")
        place()
        merge_static()
        export_room(filename)


if __name__ == "__main__":
    build()
