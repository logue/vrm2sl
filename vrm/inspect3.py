import json
import struct

with open('/tmp/output_test.glb', 'rb') as f:
    data = f.read()

json_len = struct.unpack_from('<I', data, 12)[0]
json_data = data[20:20+json_len]
gltf = json.loads(json_data)

nodes = gltf['nodes']

# Build parent map
parent = {}
for i, n in enumerate(nodes):
    for c in n.get('children', []):
        parent[c] = i

# Print scene nodes
print("=== Scene ===")
for scene in gltf.get('scenes', []):
    print(f"  scene nodes: {scene.get('nodes', [])}")

# Print chain from mHead to root
mhead_idx = next((i for i, n in enumerate(nodes) if n.get('name') == 'mHead'), None)
print(f"\n=== Chain from mHead (idx={mhead_idx}) to root ===")
cur = mhead_idx
world_t = [0.0, 0.0, 0.0]
while cur is not None:
    n = nodes[cur]
    t = n.get('translation', [0, 0, 0])
    r = n.get('rotation', [0, 0, 0, 1])
    world_t = [world_t[i] + t[i] for i in range(3)]
    print(f"  Node {cur} ({n.get('name','?')}): t={[f'{v:.5f}' for v in t]}, r={[f'{v:.4f}' for v in r]}")
    cur = parent.get(cur)

print(f"\n  World pos (sum of translations, valid only if all rotations are identity): {[f'{v:.5f}' for v in world_t]}")

# Print all SL bones
print(f"\n=== All SL bones (mXxx names) ===")
for i, n in enumerate(nodes):
    name = n.get('name', '')
    if name.startswith('m'):
        t = n.get('translation', [0, 0, 0])
        r = n.get('rotation', [0, 0, 0, 1])
        p = parent.get(i, 'root')
        parent_name = nodes[p].get('name', '?') if isinstance(p, int) else 'root'
        print(f"  Node {i} ({name}): parent={parent_name}({p}), t=[{t[0]:.5f}, {t[1]:.5f}, {t[2]:.5f}], r=[{r[0]:.4f},{r[1]:.4f},{r[2]:.4f},{r[3]:.4f}]")

# Print skins summary
print(f"\n=== Skins joint details ===")
for si, skin in enumerate(gltf.get('skins', [])):
    joints = skin.get('joints', [])
    joint_names = [nodes[j].get('name', '?') for j in joints]
    print(f"Skin {si}: joints = {joint_names}")
