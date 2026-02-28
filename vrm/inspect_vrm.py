#!/usr/bin/env python3
import json
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
    data = read_glb_json("vrm/AvatarSample_A.vrm")

    print("=== Scenes ===")
    print(json.dumps(data.get("scenes", []), indent=2))

    print("\n=== Root node(s) ===")
    for scene in data.get("scenes", []):
        for root_index in scene.get("nodes", []):
            node = data["nodes"][root_index]
            print(f"Node {root_index}: " + json.dumps(node, indent=2)[:500])

    print("\n=== First 5 nodes ===")
    for i, node in enumerate(data["nodes"][:5]):
        name = node.get("name", "unnamed")
        t = node.get("translation", "none")
        r = node.get("rotation", "none")
        s = node.get("scale", "none")
        m = node.get("matrix", "none")
        children = node.get("children", [])
        print(f"Node {i} ({name}): t={t} r={r} s={s} m={m} children={children}")

    print("\n=== Mesh nodes (91-93) ===")
    for i in [91, 92, 93]:
        if i < len(data["nodes"]):
            node = data["nodes"][i]
            name = node.get("name", "unnamed")
            t = node.get("translation", "none")
            r = node.get("rotation", "none")
            s = node.get("scale", "none")
            m = node.get("matrix", "none")
            print(f"Node {i} ({name}): t={t} r={r} s={s} m={m}")

    print("\n=== Skins ===")
    for i, skin in enumerate(data.get("skins", [])):
        print(
            f"Skin {i}: skeleton={skin.get('skeleton', 'none')} "
            f"joints_count={len(skin.get('joints', []))}"
        )

    print("\n=== Looking for hips/root/armature ===")
    for i, node in enumerate(data["nodes"]):
        name = node.get("name", "")
        if (
            "hips" in name.lower()
            or "hip" in name
            or "Root" in name
            or "Armature" in name
        ):
            t = node.get("translation", "none")
            r = node.get("rotation", "none")
            s = node.get("scale", "none")
            children = node.get("children", [])
            print(f"Node {i} ({name}): t={t} r={r} s={s} children={children[:5]}...")


if __name__ == "__main__":
    main()
