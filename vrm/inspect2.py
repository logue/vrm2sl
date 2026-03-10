import json
import struct

# Read the GLB
with open('/tmp/output_test.glb', 'rb') as f:
    data = f.read()

# Parse JSON chunk
json_len = struct.unpack_from('<I', data, 12)[0]
json_data = data[20:20+json_len]
gltf = json.loads(json_data)

# Parse BIN chunk (4-byte aligned after JSON)
bin_json_pad = (json_len + 3) & ~3
bin_start = 20 + bin_json_pad
bin_chunk_len = struct.unpack_from('<I', data, bin_start)[0]
bin_data = data[bin_start+8:bin_start+8+bin_chunk_len]

accessors = gltf['accessors']
buffer_views = gltf['bufferViews']

# Find special nodes
def find_node(name):
    return next((i for i, n in enumerate(gltf['nodes']) if n.get('name') == name), None)

mhead_idx = find_node('mHead')
mpelvis_idx = find_node('mPelvis')
print(f"mHead node index: {mhead_idx}")
print(f"mPelvis node index: {mpelvis_idx}")
if mhead_idx is not None:
    n = gltf['nodes'][mhead_idx]
    print(f"mHead: t={n.get('translation')}, r={n.get('rotation')}")
if mpelvis_idx is not None:
    n = gltf['nodes'][mpelvis_idx]
    print(f"mPelvis: t={n.get('translation')}, r={n.get('rotation')}")

print(f"\nRoot node (0): name={gltf['nodes'][0].get('name')}, t={gltf['nodes'][0].get('translation')}, r={gltf['nodes'][0].get('rotation')}")

print(f"\n=== Scene root nodes ===")
for scene in gltf.get('scenes', []):
    print(f"  {scene.get('nodes', [])}")

def read_ibm_col4(accessor_idx, slot):
    acc = accessors[accessor_idx]
    bv = buffer_views[acc['bufferView']]
    bv_offset = bv.get('byteOffset', 0)
    acc_offset = acc.get('byteOffset', 0)
    stride = bv.get('byteStride', 64)
    base = bv_offset + acc_offset + slot * stride
    # Column-major MAT4: 4th column is vals[12..16]
    vals = struct.unpack_from('<16f', bin_data, base)
    return vals

print(f"\n=== Skins ===")
for si, skin in enumerate(gltf.get('skins', [])):
    joints = skin.get('joints', [])
    bound = [n.get('name','?') for n in gltf['nodes'] if n.get('skin') == si]
    print(f"Skin {si}: {len(joints)} joints, meshes: {bound}, skeleton={skin.get('skeleton')}")

print(f"\n=== mHead IBM per skin ===")
for si, skin in enumerate(gltf.get('skins', [])):
    joints = skin.get('joints', [])
    if mhead_idx is None or mhead_idx not in joints:
        print(f"Skin {si}: mHead NOT in joints")
        continue
    slot = joints.index(mhead_idx)
    ibm_acc_idx = skin.get('inverseBindMatrices')
    if ibm_acc_idx is None:
        print(f"Skin {si}: no IBM accessor")
        continue
    vals = read_ibm_col4(ibm_acc_idx, slot)
    # Since all bones have identity rotation, IBM = [I | -p]
    # column-major: vals[12]=-px, vals[13]=-py, vals[14]=-pz
    # world pos = -vals[12], -vals[13], -vals[14]
    # But verify by checking if rotation part is ~identity
    print(f"Skin {si}: mHead slot={slot}, IBM col4=({vals[12]:.5f}, {vals[13]:.5f}, {vals[14]:.5f})")
    print(f"          world_pos (from -col4)=({-vals[12]:.5f}, {-vals[13]:.5f}, {-vals[14]:.5f})")
    # Print IBM[0,0] IBM[1,1] IBM[2,2] to verify rotation
    print(f"          IBM diag=({vals[0]:.4f}, {vals[5]:.4f}, {vals[10]:.4f})")

print(f"\n=== All joint names in each skin having mHead + its IBM vs expected world pos ===")
# Let's also compute the world position of mHead from the node hierarchy
def compute_world_pos(node_idx):
    # Build a parent map
    parent = {}
    for i, n in enumerate(gltf['nodes']):
        for c in n.get('children', []):
            parent[c] = i
    # Walk from node to root
    chain = []
    cur = node_idx
    while cur is not None:
        chain.append(cur)
        cur = parent.get(cur)
    chain.reverse()
    # Accumulate translation (rotation is identity after normalization)
    t = [0.0, 0.0, 0.0]
    for idx in chain:
        n = gltf['nodes'][idx]
        nt = n.get('translation', [0, 0, 0])
        t = [t[i] + nt[i] for i in range(3)]
    return t

if mhead_idx is not None:
    wp = compute_world_pos(mhead_idx)
    print(f"\nmHead world pos from node hierarchy: [{wp[0]:.5f}, {wp[1]:.5f}, {wp[2]:.5f}]")
    print(f"  (note: this assumes identity rotations on all ancestors, which may not be true)")
