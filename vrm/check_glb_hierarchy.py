#!/usr/bin/env python3
"""Check full GLB node hierarchy"""
import struct, json, sys

path = sys.argv[1] if len(sys.argv) > 1 else '/tmp/avatar_test.glb'
with open(path, 'rb') as f:
    data = f.read()

magic = struct.unpack_from('<I', data, 0)[0]
assert magic == 0x46546c67, "Not a GLB"
json_len = struct.unpack_from('<I', data, 12)[0]
json_data = json.loads(data[20:20+json_len])

nodes = json_data.get('nodes', [])
print(f"Total nodes: {len(nodes)}")
print()

# Build parent map
parent_of = {}
for i, node in enumerate(nodes):
    for child_idx in node.get('children', []):
        parent_of[child_idx] = i

# Print root nodes
roots = [i for i in range(len(nodes)) if i not in parent_of]
print(f"Root nodes: {roots}")
for r in roots:
    print(f"  [{r}] {nodes[r].get('name','')}")
print()

# Print key nodes and their full paths
def get_path(idx):
    path_parts = []
    cur = idx
    while cur in parent_of:
        path_parts.insert(0, f"[{cur}]{nodes[cur].get('name','')}")
        cur = parent_of[cur]
    path_parts.insert(0, f"[{cur}]{nodes[cur].get('name','')}")
    return ' > '.join(path_parts)

key_names = {'mPelvis', 'mTorso', 'mChest', 'mNeck', 'mHead', 'mShoulderLeft', 'Root', 'Armature'}
print("Key node paths:")
for i, node in enumerate(nodes):
    if node.get('name', '') in key_names:
        rot = node.get('rotation', [0,0,0,1])
        print(f"  {get_path(i)}")
        print(f"    rotation: ({rot[0]:.4f},{rot[1]:.4f},{rot[2]:.4f},{rot[3]:.4f})")

print()
# Print skin info
skins = json_data.get('skins', [])
for i, skin in enumerate(skins):
    skel_idx = skin.get('skeleton')
    skel_name = nodes[skel_idx].get('name', '') if skel_idx is not None else None
    print(f"Skin {i}: skeleton={skel_idx} ({skel_name}), joints={len(skin.get('joints',[]))}")
