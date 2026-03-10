import struct, json, sys

glb_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/avatar_test5.glb"

with open(glb_file, "rb") as f:
    f.read(12)
    chunk0_len = struct.unpack("<I", f.read(4))[0]
    f.read(4)
    json_bytes = f.read(chunk0_len)
    chunk1_len = struct.unpack("<I", f.read(4))[0]
    f.read(4)
    bin_data = f.read(chunk1_len)

gltf = json.loads(json_bytes)
nodes = gltf["nodes"]

print("=== Mesh nodes and their skin references ===")
for i, node in enumerate(nodes):
    if "mesh" in node:
        print(f"  Node {i} '{node.get('name')}': mesh={node.get('mesh')} skin={node.get('skin')}")

print()

# Find body skin index (most joints)
body_skin_idx = max(range(len(gltf["skins"])), key=lambda i: len(gltf["skins"][i].get("joints", [])))
body_skin = gltf["skins"][body_skin_idx]
body_joints = body_skin["joints"]
body_joint_names = [nodes[j].get("name","?") for j in body_joints]
mhead_slot = next((s for s, n in enumerate(body_joint_names) if n == "mHead"), None)
print(f"Body skin index: {body_skin_idx}, mHead slot: {mhead_slot}")

print()
print("=== Verifying Face/Hair vertex joint indices point to mHead slot ===")

# Find Face and Hair mesh nodes
for i, node in enumerate(nodes):
    if "mesh" not in node:
        continue
    name = node.get("name","?")
    if name not in ("Face", "Hair"):
        continue
    skin_ref = node.get("skin")
    mesh_idx = node["mesh"]
    mesh = gltf["meshes"][mesh_idx]
    for prim in mesh.get("primitives", []):
        j_acc_idx = prim.get("attributes", {}).get("JOINTS_0")
        if j_acc_idx is None:
            continue
        acc = gltf["accessors"][j_acc_idx]
        bv = gltf["bufferViews"][acc["bufferView"]]
        base = bv.get("byteOffset",0) + acc.get("byteOffset",0)
        ct = acc.get("componentType")  # 5121=ubyte, 5123=ushort
        count = acc["count"]
        stride = bv.get("byteStride", 4 if ct==5121 else 8)
        fmt = "BBBB" if ct==5121 else "HHHH"
        sz = 4 if ct==5121 else 8

        # Sample first 5 vertices
        unique_joints = set()
        for v in range(min(count, 20)):
            off = base + v * sz
            jts = struct.unpack_from(fmt, bin_data, off)
            for j in jts:
                unique_joints.add(j)
        
        first = struct.unpack_from(fmt, bin_data, base)
        print(f"  {name} mesh (skin={skin_ref}): JOINTS_0 first vertex={first}, unique joint slots in first 20 verts={sorted(unique_joints)}")
        if mhead_slot is not None and all(j == mhead_slot or j == 0 for j in unique_joints):
            if mhead_slot in unique_joints:
                print(f"    ✓ All vertices reference slot {mhead_slot} (mHead in Body skin)")
            else:
                print(f"    ✗ WARNING: no vertices reference mHead slot {mhead_slot}")
