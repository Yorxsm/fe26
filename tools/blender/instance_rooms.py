"""Turn repeated glTF nodes into EXT_mesh_gpu_instancing.

Blender 4.0 writes a node per copy even when the mesh is shared. This pass
keeps one node per repeated mesh and stores the transforms in the extension
three.js already draws as a single instanced mesh.
"""

import json
import os
import struct
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "apps", "web", "public", "models")
ROOMS = [
    "interior-vault.glb",
    "interior-market.glb",
    "interior-war.glb",
    "interior-forge.glb",
    "interior-stadium.glb",
]


def read_glb(path):
    data = open(path, "rb").read()
    _magic, _version, _length = struct.unpack_from("<III", data, 0)
    offset = 12
    document = None
    binary = b""
    while offset < len(data):
        chunk_len, chunk_type = struct.unpack_from("<II", data, offset)
        chunk = data[offset + 8 : offset + 8 + chunk_len]
        if chunk_type == 0x4E4F534A:
            document = json.loads(chunk)
        elif chunk_type == 0x004E4942:
            binary = chunk
        offset += 8 + chunk_len
    return document, binary


def write_glb(path, document, binary):
    text = json.dumps(document, separators=(",", ":")).encode("utf-8")
    text += b" " * ((4 - len(text) % 4) % 4)
    binary += b"\x00" * ((4 - len(binary) % 4) % 4)
    total = 12 + 8 + len(text) + 8 + len(binary)
    blob = struct.pack("<III", 0x46546C67, 2, total)
    blob += struct.pack("<II", len(text), 0x4E4F534A) + text
    blob += struct.pack("<II", len(binary), 0x004E4942) + binary
    open(path, "wb").write(blob)


def vec(node, key, default):
    value = node.get(key, default)
    return [float(item) for item in value]


def instance_file(path):
    document, binary = read_glb(path)
    binary = binary[: document["buffers"][0]["byteLength"]]
    scene = document["scenes"][document.get("scene", 0)]
    groups = {}
    for index in scene["nodes"]:
        node = document["nodes"][index]
        if "mesh" not in node:
            continue
        groups.setdefault(node["mesh"], []).append(index)
    extra = bytearray()
    base = len(binary)
    kept = []
    made = 0

    def add_view(values, components):
        nonlocal base
        raw = b"".join(struct.pack("<" + "f" * components, *row) for row in values)
        pad = (4 - (base % 4)) % 4
        extra.extend(b"\x00" * pad)
        offset = base + pad
        extra.extend(raw)
        base = offset + len(raw)
        view = {"buffer": 0, "byteOffset": offset, "byteLength": len(raw)}
        document["bufferViews"].append(view)
        view_index = len(document["bufferViews"]) - 1
        flat = [item for row in values for item in row]
        mins = [min(flat[i::components]) for i in range(components)]
        maxs = [max(flat[i::components]) for i in range(components)]
        accessor = {
            "bufferView": view_index,
            "componentType": 5126,
            "count": len(values),
            "type": "VEC3" if components == 3 else "VEC4",
            "min": mins,
            "max": maxs,
        }
        document["accessors"].append(accessor)
        return len(document["accessors"]) - 1

    for index in scene["nodes"]:
        node = document["nodes"][index]
        members = groups.get(node.get("mesh"), [index])
        if len(members) < 2 or index != members[0]:
            if len(members) < 2:
                kept.append(index)
            continue
        translations = [vec(document["nodes"][item], "translation", [0, 0, 0]) for item in members]
        rotations = [vec(document["nodes"][item], "rotation", [0, 0, 0, 1]) for item in members]
        scales = [vec(document["nodes"][item], "scale", [1, 1, 1]) for item in members]
        node.pop("translation", None)
        node.pop("rotation", None)
        node.pop("scale", None)
        node.pop("matrix", None)
        node.setdefault("extensions", {})["EXT_mesh_gpu_instancing"] = {
            "attributes": {
                "TRANSLATION": add_view(translations, 3),
                "ROTATION": add_view(rotations, 4),
                "SCALE": add_view(scales, 3),
            }
        }
        kept.append(index)
        made += 1
    scene["nodes"] = kept
    used = document.setdefault("extensionsUsed", [])
    required = document.setdefault("extensionsRequired", [])
    if made and "EXT_mesh_gpu_instancing" not in used:
        used.append("EXT_mesh_gpu_instancing")
    if made and "EXT_mesh_gpu_instancing" not in required:
        required.append("EXT_mesh_gpu_instancing")
    document["buffers"][0]["byteLength"] = len(binary) + len(extra)
    write_glb(path, document, binary + extra)
    print(os.path.basename(path), "instances", made, "nodes", len(kept), "bytes", os.path.getsize(path))


if __name__ == "__main__":
    targets = sys.argv[1:] or [os.path.join(OUT, name) for name in ROOMS]
    for target in targets:
        instance_file(target)
