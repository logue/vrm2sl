import struct, json, sys

glb_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/avatar_test2.glb"

with open(glb_file, "rb") as f:
    f.read(12)  # magic, version, length
    chunk0_len = struct.unpack("<I", f.read(4))[0]
    f.read(4)  # chunk type
    json_bytes = f.read(chunk0_len)
    chunk1_len = struct.unpack("<I", f.read(4))[0]
    f.read(4)  # chunk type
    bin_data = f.read(chunk1_len)

gltf = json.loads(json_bytes)
nodes = gltf["nodes"]

for i, skin in enumerate(gltf.get("skins", [])):
    mesh_names = [nodes[n].get("name","?") for n in range(len(nodes)) if nodes[n].get("skin") == i]
    joints = skin.get("joints", [])
    joint_names = [nodes[j].get("name","?") for j in joints]
    ibm_acc_idx = skin.get("inverseBindMatrices")

    if ibm_acc_idx is None:
        print(f"Skin {i} ({mesh_names}): no IBM")
        continue

    acc = gltf["accessors"][ibm_acc_idx]
    bv_idx = acc["bufferView"]
    bv = gltf["bufferViews"][bv_idx]
    bv_offset = bv.get("byteOffset", 0)
    acc_offset = acc.get("byteOffset", 0)
    base = bv_offset + acc_offset
    count = acc["count"]

    # Read IBM[0] - column-major MAT4, translation at [12],[13],[14]
    mat0 = struct.unpack_from("<16f", bin_data, base)
    tx, ty, tz = mat0[12], mat0[13], mat0[14]

    print(f"Skin {i} ({mesh_names}): joints={joint_names}, IBM count={count}, IBM[0] pos=({tx:.4f}, {ty:.4f}, {tz:.4f})")
