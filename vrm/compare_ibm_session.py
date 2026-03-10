import struct, json, sys

glb_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/avatar_test4.glb"

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

# Print ALL skins with IBM raw bytes for mHead slot
for i, skin in enumerate(gltf.get("skins", [])):
    joints = skin.get("joints", [])
    joint_names = [nodes[j].get("name","?") for j in joints]
    mesh_names = [nodes[n].get("name","?") for n in range(len(nodes)) if nodes[n].get("skin") == i]

    ibm_acc_idx = skin.get("inverseBindMatrices")
    if ibm_acc_idx is None:
        continue
    acc = gltf["accessors"][ibm_acc_idx]
    bv = gltf["bufferViews"][acc["bufferView"]]
    bv_offset = bv.get("byteOffset", 0)
    acc_offset = acc.get("byteOffset", 0)
    base = bv_offset + acc_offset
    stride = bv.get("byteStride", 64)

    # find mHead slot
    mhead_slot = next((s for s, j in enumerate(joints) if nodes[j].get("name") == "mHead"), None)

    print(f"Skin {i} ({mesh_names}): joints={joint_names}")
    print(f"  IBM acc={ibm_acc_idx} count={acc['count']} BV={acc['bufferView']} bv_byteOffset={bv_offset} bv_byteLength={bv.get('byteLength')} skeleton={skin.get('skeleton')}")

    if mhead_slot is not None:
        off = base + mhead_slot * stride
        raw = bin_data[off:off+64]
        mat = struct.unpack_from("<16f", bin_data, off)
        print(f"  mHead slot={mhead_slot} offset={off}")
        print(f"  IBM col3 (translation): ({mat[12]:.6f}, {mat[13]:.6f}, {mat[14]:.6f})")
        print(f"  IBM raw [48..64]: {raw[48:64].hex()}")
        # Also show rotation part
        print(f"  IBM[0..4] (col0): {mat[0]:.6f} {mat[1]:.6f} {mat[2]:.6f} {mat[3]:.6f}")
    print()

# Compare Face and Body IBM bytes directly
print("=== Byte comparison: Face IBM[0] vs Body IBM[4] ===")
# Face
face_skin = gltf["skins"][0]
face_acc = gltf["accessors"][face_skin["inverseBindMatrices"]]
face_bv = gltf["bufferViews"][face_acc["bufferView"]]
face_base = face_bv.get("byteOffset",0) + face_acc.get("byteOffset",0)
face_bytes = bin_data[face_base:face_base+64]

# Body
body_skin = None
for i, s in enumerate(gltf["skins"]):
    joint_names = [nodes[j].get("name","?") for j in s.get("joints",[])]
    if "mPelvis" in joint_names and len(joint_names) > 10:
        body_skin = s
        body_skin_idx = i
        break

body_mhead_slot = next((s for s, j in enumerate(body_skin["joints"]) if nodes[j].get("name") == "mHead"), None)
body_acc = gltf["accessors"][body_skin["inverseBindMatrices"]]
body_bv = gltf["bufferViews"][body_acc["bufferView"]]
body_base = body_bv.get("byteOffset",0) + body_acc.get("byteOffset",0)
body_mhead_off = body_base + body_mhead_slot * 64
body_bytes = bin_data[body_mhead_off:body_mhead_off+64]

print(f"Face IBM[0]     (hex): {face_bytes[:32].hex()}")
print(f"Body IBM[{body_mhead_slot}] mHead (hex): {body_bytes[:32].hex()}")
print(f"Match: {face_bytes == body_bytes}")
if face_bytes != body_bytes:
    diffs = [(i, face_bytes[i], body_bytes[i]) for i in range(64) if face_bytes[i] != body_bytes[i]]
    print(f"Differences at bytes: {diffs}")
