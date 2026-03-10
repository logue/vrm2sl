import json
import struct
import numpy as np

# Read the GLB
with open('/tmp/output_test.glb', 'rb') as f:
    data = f.read()

# Parse JSON chunk
json_len = struct.unpack_from('<I', data, 12)[0]
json_data = data[20:20+json_len]
gltf = json.loads(json_data)

# Parse BIN chunk
bin_json_pad = (json_len + 3) & ~3
bin_start = 20 + bin_json_pad
bin_chunk_len = struct.unpack_from('<I', data, bin_start)[0]
bin_data = data[bin_start+8:bin_start+8+bin_chunk_len]

accessors = gltf['accessors']
buffer_views = gltf['bufferViews']

# Find mHead
mhead_idx = next((i for i, n in enumerate(gltf['nodes']) if n.get('name') == 'mHead'), None)
print(f"mHead node index: {mhead_idx}")
if mhead_idx is not None:
    print(f"mHead node: {json.dumps(gltf['nodes'][mhead_idx], indent=2)}")

# Find mPelvis
mpelvis_idx = next((i for i, n in enumerate(gltf['nodes']) if n.get('name') == 'mPelvis'), None)
print(f"\nmPelvis node index: {mpelvis_idx}")
if mpelvis_idx is not None:
    print(f"mPelvis node: {json.dumps(gltf['nodes'][mpelvis_idx], indent=2)}")

# Root node
print(f"\nRoot node (0): {json.dumps(gltf['nodes'][0], indent=2)}")

# Skin info
print(f"\n=== Skins ===")
for si, skin in enumerate(gltf.get('skins', [])):
    joints = skin.get('joints', [])
    bound_nodes = [n.get('name','?') for n in gltf['nodes'] if n.get('skin') == si]
    print(f"Skin {si}: {len(joints)} joints, bound to mesh nodes: {bound_nodes}")

def read_ibm(accessor_idx, slot):
    acc = accessors[accessor_idx]
    bv = buffer_views[acc['bufferView']]
    bv_offset = bv.get('byteOffset', 0)
    acc_offset = acc.get('byteOffset', 0)
    stride = bv.get('byteStride', 64)
    base = bv_offset + acc_offset + slot * stride
    vals = struct.unpack_from('<16f', bin_data, base)
    return vals

print(f"\n=== mHead IBM world position per skin ===")
for si, skin in enumerate(gltf.get('skins', [])):
    joints = skin.get('joints', [])
    if mhead_idx not in joints:
        print(f"Skin {si}: mHead NOT in joints")
        continue
    slot = joints.index(mhead_idx)
    ibm_acc_idx = skin.get('inverseBindMatrices')
    if ibm_acc_idx is None:
        print(f"Skin {si}: no IBM accessor")
        continue
    ibm_vals = read_ibm(ibm_acc_idx, slot)
    ibm = np.array(ibm_vals).reshape(4, 4, order='F')  # column-major
    inv = np.linalg.inv(ibm)
    world_pos = inv[:3, 3]
    print(f"Skin {si}: mHead slot={slot}, world_pos=[{world_pos[0]:.5f}, {world_pos[1]:.5f}, {world_pos[2]:.5f}]")

# Print scene nodes
print(f"\n=== Scene root nodes ===")
for scene in gltf.get('scenes', []):
    print(f"  {scene.get('nodes', [])}")
