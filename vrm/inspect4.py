import json
import struct

fname = '/tmp/debug_output.glb'
with open(fname, 'rb') as f:
    data = f.read()

json_len = struct.unpack_from('<I', data, 12)[0]
gltf = json.loads(data[20:20+json_len])

bin_json_pad = (json_len + 3) & ~3
bin_start = 20 + bin_json_pad
bin_chunk_len = struct.unpack_from('<I', data, bin_start)[0]
bin_data = data[bin_start+8:bin_start+8+bin_chunk_len]

nodes = gltf['nodes']
accessors = gltf['accessors']
buffer_views = gltf['bufferViews']

def find_node(name):
    return next((i for i, n in enumerate(nodes) if n.get('name') == name), None)

mhead_idx = find_node('mHead')

# Print skins
print("=== Skins ===")
for si, skin in enumerate(gltf.get('skins', [])):
    joints = skin.get('joints', [])
    bound = [n.get('name','?') for n in nodes if n.get('skin') == si]
    joint_names = [nodes[j].get('name', '?') for j in joints]
    print(f"Skin {si}: skeleton={skin.get('skeleton')}, joints={joint_names}, meshes={bound}")

# Read IBM for each skin's mHead entry
print("\n=== mHead IBM per skin ===")
for si, skin in enumerate(gltf.get('skins', [])):
    joints = skin.get('joints', [])
    if mhead_idx not in joints:
        continue
    slot = joints.index(mhead_idx)
    ibm_acc_idx = skin.get('inverseBindMatrices')
    if ibm_acc_idx is None:
        continue
    acc = accessors[ibm_acc_idx]
    bv = buffer_views[acc['bufferView']]
    bv_offset = bv.get('byteOffset', 0)
    acc_offset = acc.get('byteOffset', 0)
    stride = bv.get('byteStride', 64)
    base = bv_offset + acc_offset + slot * stride
    vals = struct.unpack_from('<16f', bin_data, base)
    # For identity rotation: IBM = [I | -p], col4 = (-px, -py, -pz, 1)
    print(f"Skin {si}: mHead IBM slot={slot}")
    print(f"  IBM col4 = ({vals[12]:.6f}, {vals[13]:.6f}, {vals[14]:.6f})")
    print(f"  world_pos = ({-vals[12]:.6f}, {-vals[13]:.6f}, {-vals[14]:.6f})")
    print(f"  IBM diagonal (rotation check) = ({vals[0]:.4f}, {vals[5]:.4f}, {vals[10]:.4f})")
    # Print full IBM
    print(f"  Full IBM:")
    for row in range(4):
        r = [vals[row + col*4] for col in range(4)]  # row-major reading from column-major storage
        print(f"    [{r[0]:.4f} {r[1]:.4f} {r[2]:.4f} {r[3]:.4f}]")
    print()

# Also check accessor counts and buffer view overlaps
print("=== IBM Accessor details ===")
for si, skin in enumerate(gltf.get('skins', [])):
    ibm_acc_idx = skin.get('inverseBindMatrices')
    if ibm_acc_idx is None:
        continue
    acc = accessors[ibm_acc_idx]
    bv_idx = acc.get('bufferView')
    bv = buffer_views[bv_idx] if bv_idx is not None else {}
    bv_offset = bv.get('byteOffset', 0)
    acc_offset = acc.get('byteOffset', 0)
    stride = bv.get('byteStride', 64)
    count = acc.get('count', 0)
    base = bv_offset + acc_offset
    end = base + count * stride
    print(f"Skin {si}: IBM acc={ibm_acc_idx}, bv={bv_idx}, bv_offset={bv_offset}, acc_offset={acc_offset}, count={count}, stride={stride}")
    print(f"  Data range: [{base}, {end})")
