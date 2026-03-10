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
node_name_to_idx = {n.get("name", ""): i for i, n in enumerate(nodes)}

mhead_idx = node_name_to_idx.get("mHead")
print(f"mHead node index: {mhead_idx}")
if mhead_idx is not None:
    mhead_node = nodes[mhead_idx]
    print(f"mHead translation: {mhead_node.get('translation', 'none')}")
    print(f"mHead rotation:    {mhead_node.get('rotation', 'none')}")

print()

for i, skin in enumerate(gltf.get("skins", [])):
    mesh_names = [nodes[n].get("name","?") for n in range(len(nodes)) if nodes[n].get("skin") == i]
    joints = skin.get("joints", [])
    joint_names = [nodes[j].get("name","?") for j in joints]

    ibm_acc_idx = skin.get("inverseBindMatrices")
    if ibm_acc_idx is None:
        continue

    acc = gltf["accessors"][ibm_acc_idx]
    bv = gltf["bufferViews"][acc["bufferView"]]
    bv_offset = bv.get("byteOffset", 0)
    acc_offset = acc.get("byteOffset", 0)
    base = bv_offset + acc_offset
    count = acc["count"]
    stride = bv.get("byteStride", 64)

    # Find mHead's slot in this skin
    mhead_slot = None
    for slot, node_idx in enumerate(joints):
        if nodes[node_idx].get("name") == "mHead":
            mhead_slot = slot
            break

    print(f"Skin {i} ({mesh_names}): count={count}, joints[0..4]={joint_names[:4]}, mHead at slot={mhead_slot}")

    if mhead_slot is not None:
        off = base + mhead_slot * stride
        mat = struct.unpack_from("<16f", bin_data, off)
        # Extract translation from IBM (column 3 in column-major = mat[12..14])
        ibm_t = (mat[12], mat[13], mat[14])
        # Rotation part (top-left 3x3 in column-major)
        # For rigid IBM = [R^T | -R^T*t], actual world pos t = -R * IBM_col3
        # Extract R from IBM: R = IBM[:3,:3]^T (since IBM[:3,:3] = R^T)
        # R^T = [[mat[0],mat[1],mat[2]], [mat[4],mat[5],mat[6]], [mat[8],mat[9],mat[10]]]
        # R   = [[mat[0],mat[4],mat[8]], [mat[1],mat[5],mat[9]], [mat[2],mat[6],mat[10]]]
        rx = (mat[0], mat[4], mat[8])
        ry = (mat[1], mat[5], mat[9])
        rz = (mat[2], mat[6], mat[10])
        # world pos = -R * IBM_col3 = -(rx.dot(ibm_t), ry.dot(ibm_t), rz.dot(ibm_t))
        tx = -(rx[0]*ibm_t[0] + rx[1]*ibm_t[1] + rx[2]*ibm_t[2])
        ty = -(ry[0]*ibm_t[0] + ry[1]*ibm_t[1] + ry[2]*ibm_t[2])
        tz = -(rz[0]*ibm_t[0] + rz[1]*ibm_t[1] + rz[2]*ibm_t[2])
        print(f"  -> mHead IBM col3 (raw): ({ibm_t[0]:.5f}, {ibm_t[1]:.5f}, {ibm_t[2]:.5f})")
        print(f"  -> mHead world pos (inv): ({tx:.5f}, {ty:.5f}, {tz:.5f})")
    print()
