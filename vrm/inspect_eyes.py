#!/usr/bin/env python3
import json
import struct
import sys

path = sys.argv[1] if len(sys.argv) > 1 else "vrm/output.glb"
with open(path, "rb") as f:
    f.read(12)
    json_len = struct.unpack("<I", f.read(4))[0]
    f.read(4)
    js = json.loads(f.read(json_len))

nodes = js.get("nodes", [])
meshes = js.get("meshes", [])
mats = js.get("materials", [])

print(f"file: {path}")
print(f"nodes={len(nodes)} meshes={len(meshes)} materials={len(mats)}")

print("\n[Nodes eye/face/head]")
for i, n in enumerate(nodes):
    name = n.get("name", "")
    low = name.lower()
    if any(k in low for k in ["eye", "face", "head"]):
        print(i, name, "mesh", n.get("mesh"), "skin", n.get("skin"))

print("\n[Meshes eye/face/head]")
for i, m in enumerate(meshes):
    name = m.get("name", f"mesh{i}")
    low = name.lower()
    if any(k in low for k in ["eye", "face", "head"]):
        print("mesh", i, name)
        for pi, p in enumerate(m.get("primitives", [])):
            mi = p.get("material")
            print(" prim", pi, "material", mi, "attrs", list(p.get("attributes", {}).keys()))
            if isinstance(mi, int) and 0 <= mi < len(mats):
                mat = mats[mi]
                pbr = mat.get("pbrMetallicRoughness", {})
                print(
                    "  mat",
                    mat.get("name"),
                    "alphaMode",
                    mat.get("alphaMode", "OPAQUE"),
                    "alphaCutoff",
                    mat.get("alphaCutoff"),
                    "doubleSided",
                    mat.get("doubleSided"),
                    "baseColorFactor",
                    pbr.get("baseColorFactor"),
                )

print("\n[Thumb bones]")
for i, n in enumerate(nodes):
    name = n.get("name", "")
    if "thumb" in name.lower() or "handthumb" in name:
        print(i, name, "children", n.get("children", []))
