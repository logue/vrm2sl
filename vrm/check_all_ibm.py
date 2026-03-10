import struct, json

with open('/tmp/avatar_new.glb', 'rb') as f:
    f.read(12)
    json_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    j = json.loads(f.read(json_len))
    bin_start = 12 + 8 + json_len + 8
    f.seek(bin_start)
    buf = f.read()

skin = j['skins'][1]
joints = skin['joints']
nodes = j['nodes']

ibm_acc = j['accessors'][skin['inverseBindMatrices']]
ibm_bv = j['bufferViews'][ibm_acc['bufferView']]
bv_offset = ibm_bv.get('byteOffset', 0)
acc_offset = ibm_acc.get('byteOffset', 0)
stride = ibm_bv.get('byteStride', 64)
count = ibm_acc['count']

print(f"IBM accessor count={count}, joints count={len(joints)}")
print(f"Buffer view offset={bv_offset}, byteLength={ibm_bv.get('byteLength')}, stride={stride}")
print()
print("All joints and IBM positions:")
for i, ji in enumerate(joints):
    name = nodes[ji].get('name', '?')
    offset = bv_offset + acc_offset + i * stride
    m = struct.unpack_from('<16f', buf, offset)
    # column-major MAT4: position is column 3 = m[12], m[13], m[14]
    px, py, pz = m[12], m[13], m[14]
    print(f"  {i:2d} {name:<30} IBM pos=({px:.4f}, {py:.4f}, {pz:.4f})")

# Also print node world positions for comparison
print()
print("Node world positions (computed by traversing hierarchy):")

def get_world_pos(node_idx, nodes, parent_pos=(0,0,0)):
    node = nodes[node_idx]
    local_t = node.get('translation', [0,0,0])
    world_t = (parent_pos[0]+local_t[0], parent_pos[1]+local_t[1], parent_pos[2]+local_t[2])
    return world_t

# Build parent map
parent_map = {}
for i, node in enumerate(nodes):
    for child in node.get('children', []):
        parent_map[child] = i

def world_pos(idx):
    pos = [0.0, 0.0, 0.0]
    cur = idx
    chain = []
    while cur is not None:
        chain.append(cur)
        cur = parent_map.get(cur)
    for n in reversed(chain):
        t = nodes[n].get('translation', [0, 0, 0])
        pos[0] += t[0]
        pos[1] += t[1]
        pos[2] += t[2]
    return pos

for i, ji in enumerate(joints[:10]):
    name = nodes[ji].get('name', '?')
    wp = world_pos(ji)
    offset = bv_offset + acc_offset + i * stride
    m = struct.unpack_from('<16f', buf, offset)
    px, py, pz = m[12], m[13], m[14]
    match = abs(px+wp[0]) < 0.001 and abs(py+wp[1]) < 0.001 and abs(pz+wp[2]) < 0.001
    print(f"  {name:<30} world=({wp[0]:.4f},{wp[1]:.4f},{wp[2]:.4f}) IBM_pos=({px:.4f},{py:.4f},{pz:.4f}) {'OK' if match else 'MISMATCH'}")
