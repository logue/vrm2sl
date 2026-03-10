import struct, json, sys

vrm_file = sys.argv[1] if len(sys.argv) > 1 else "/Users/logue/Developer/vrm2sl/vrm/AvatarSample_A.vrm"

with open(vrm_file, "rb") as f:
    f.read(12)
    chunk0_len = struct.unpack("<I", f.read(4))[0]
    f.read(4)
    json_bytes = f.read(chunk0_len)

gltf = json.loads(json_bytes)
nodes = gltf["nodes"]

print("=== Original VRM Skins (Face/Hair related) ===")
for i, skin in enumerate(gltf.get("skins", [])):
    joints = skin.get("joints", [])
    joint_names = [nodes[j].get("name", "?") for j in joints]
    mesh_nodes = [n for n in nodes if n.get("skin") == i]
    mesh_names = [n.get("name","?") for n in mesh_nodes]
    if any("face" in m.lower() or "hair" in m.lower() for m in mesh_names):
        print(f"Skin {i}: mesh={mesh_names}, joints({len(joints)}): {joint_names}")
        ibm_acc_idx = skin.get("inverseBindMatrices")
        if ibm_acc_idx:
            acc = gltf["accessors"][ibm_acc_idx]
            bv = gltf["bufferViews"][acc["bufferView"]]
            print(f"  IBM: accessor={ibm_acc_idx} count={acc['count']} bufferView={acc['bufferView']} byteOffset={acc.get('byteOffset',0)}")
            print(f"  BV:  byteLength={bv.get('byteLength')} byteOffset={bv.get('byteOffset',0)} byteStride={bv.get('byteStride','none')}")
        print()
