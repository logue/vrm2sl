#!/usr/bin/env python3
"""Inspect the converted GLB to check scene roots, skeleton root, and bone transforms."""
import json
import os
import struct


def read_glb_json(path: str) -> dict:
    with open(path, "rb") as file:
        _magic = file.read(4)
        _version = struct.unpack("<I", file.read(4))[0]
        _length = struct.unpack("<I", file.read(4))[0]
        chunk_len = struct.unpack("<I", file.read(4))[0]
        _chunk_type = struct.unpack("<I", file.read(4))[0]
        json_bytes = file.read(chunk_len)
    return json.loads(json_bytes)


def main() -> None:
    out_path = "vrm/output.glb"
    if not os.path.exists(out_path):
        print(f"Output file not found: {out_path}")
        return

    out = read_glb_json(out_path)

    print("=== OUTPUT GLB ===")
    print(f"Scene root nodes: {out['scenes'][0]['nodes']}")
    print(f"Total nodes: {len(out['nodes'])}")

    print("\n--- Scene root node transforms ---")
    for index in out["scenes"][0]["nodes"]:
        node = out["nodes"][index]
        name = node.get("name", "unnamed")
        t = node.get("translation", "none")
        r = node.get("rotation", "none")
        s = node.get("scale", "none")
        m = node.get("matrix", "none")
        mesh = node.get("mesh", "none")
        skin = node.get("skin", "none")
        print(
            f"Node {index} ({name}): "
            f"t={t} r={r} s={s} m={m} mesh={mesh} skin={skin}"
        )

    print("\n--- Skins ---")
    for i, skin in enumerate(out.get("skins", [])):
        skeleton = skin.get("skeleton", "none")
        skeleton_name = (
            out["nodes"][skeleton].get("name", "")
            if isinstance(skeleton, int) and skeleton < len(out["nodes"])
            else "?"
        )
        joints = skin.get("joints", [])
        preview = [out["nodes"][j].get("name", "?") for j in joints[:5]]
        print(
            f"Skin {i}: skeleton={skeleton}({skeleton_name}) "
            f"joints={preview}... ({len(joints)} total)"
        )


if __name__ == "__main__":
    main()
