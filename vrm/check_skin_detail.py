import struct, json, sys

glb_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/avatar_test2.glb"

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

print("=== Scene Nodes (mesh + skin) ===")
for i, node in enumerate(nodes):
    if "mesh" in node or "skin" in node:
        n = node.copy()
        print(f"  Node {i} name={n.get('name')} mesh={n.get('mesh')} skin={n.get('skin')} "
              f"T={n.get('translation')} R={n.get('rotation')}")

print()
print("=== Skins ===")
for i, skin in enumerate(gltf.get("skins", [])):
    mesh_nodes = [n for n_idx, n in enumerate(nodes) if n.get("skin") == i]
    mesh_names = [n.get("name", "?") for n in mesh_nodes]
    ibm_acc_idx = skin.get("inverseBindMatrices")
    skeleton = skin.get("skeleton")
    joints = skin.get("joints", [])
    joint_names = [nodes[j].get("name", "?") for j in joints]

    acc = gltf["accessors"][ibm_acc_idx]
    bv = gltf["bufferViews"][acc["bufferView"]]
    print(f"Skin {i}: name={skin.get('name')} mesh={mesh_names} skeleton={skeleton}")
    print(f"  joints ({len(joints)}): {joint_names}")
    print(f"  IBM accessor {ibm_acc_idx}: count={acc['count']} type={acc.get('type')} bufferView={acc['bufferView']} byteOffset={acc.get('byteOffset',0)}")
    print(f"  BufferView {acc['bufferView']}: byteOffset={bv.get('byteOffset',0)} byteLength={bv.get('byteLength')} byteStride={bv.get('byteStride','none')}")

    # Actual bytes accessible
    bv_offset = bv.get("byteOffset", 0)
    acc_offset = acc.get("byteOffset", 0)
    base = bv_offset + acc_offset
    stride = bv.get("byteStride", 64)

    # Read ALL IBM entries (up to count)
    for slot in range(acc["count"]):
        off = base + slot * stride
        if off + 64 > len(bin_data):
            print(f"  IBM[{slot}]: OUT OF BOUNDS (offset={off}, bin_len={len(bin_data)})")
            continue
        mat = struct.unpack_from("<16f", bin_data, off)
        ibm_t = (mat[12], mat[13], mat[14])
        # compute world pos
        rx = (mat[0], mat[4], mat[8])
        ry = (mat[1], mat[5], mat[9])
        rz = (mat[2], mat[6], mat[10])
        tx = -(rx[0]*ibm_t[0] + rx[1]*ibm_t[1] + rx[2]*ibm_t[2])
        ty = -(ry[0]*ibm_t[0] + ry[1]*ibm_t[1] + ry[2]*ibm_t[2])
        tz = -(rz[0]*ibm_t[0] + rz[1]*ibm_t[1] + rz[2]*ibm_t[2])
        joint_name = joint_names[slot] if slot < len(joint_names) else "?"
        print(f"  IBM[{slot}] joint={joint_name}: raw_col3=({ibm_t[0]:.4f},{ibm_t[1]:.4f},{ibm_t[2]:.4f}) world=({tx:.4f},{ty:.4f},{tz:.4f})")
    print()
