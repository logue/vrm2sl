"""
Check JOINTS_0 data for Body mesh vertices and verify correct bone assignments.
"""
import struct, json

with open('/tmp/avatar_new.glb', 'rb') as f:
    f.read(12)
    json_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    j = json.loads(f.read(json_len))
    bin_start = 12 + 8 + json_len + 8
    f.seek(bin_start)
    buf = f.read()

def get_accessor_data_f32(acc_idx):
    acc = j['accessors'][acc_idx]
    bv = j['bufferViews'][acc['bufferView']]
    bv_offset = bv.get('byteOffset', 0)
    acc_offset = acc.get('byteOffset', 0)
    offset = bv_offset + acc_offset
    typ = acc['type']
    count = acc['count']
    
    if typ == 'VEC3':
        n, fmt, item_size = 3, '<3f', 12
    elif typ == 'VEC4':
        n, fmt, item_size = 4, '<4f', 16
    else:
        return None
    
    stride = bv.get('byteStride', 0) or item_size
    data = []
    for i in range(count):
        vals = struct.unpack_from(fmt, buf, offset + i * stride)
        data.append(vals)
    return data

def get_joints_accessor(acc_idx):
    acc = j['accessors'][acc_idx]
    bv = j['bufferViews'][acc['bufferView']]
    bv_offset = bv.get('byteOffset', 0)
    acc_offset = acc.get('byteOffset', 0)
    offset = bv_offset + acc_offset
    count = acc['count']
    ct = acc['componentType']
    
    if ct == 5121:  # UNSIGNED_BYTE
        fmt, item_size = '<4B', 4
    elif ct == 5123:  # UNSIGNED_SHORT
        fmt, item_size = '<4H', 8
    else:
        return None
    
    stride = bv.get('byteStride', 0) or item_size
    data = []
    for i in range(count):
        vals = struct.unpack_from(fmt, buf, offset + i * stride)
        data.append(vals)
    return data

# Get Body node (node 92), mesh=1, skin=1
body_node = j['nodes'][92]
print(f"Body node: name={body_node.get('name')}, mesh={body_node.get('mesh')}, skin={body_node.get('skin')}")

skin = j['skins'][1]  # skin 1
joints = skin['joints']
nodes = j['nodes']

# Find mHipLeft joint index
hip_left_idx = None
for ji, joint_node in enumerate(joints):
    if nodes[joint_node].get('name') == 'mHipLeft':
        hip_left_idx = ji
        break

print(f"mHipLeft is joint index {hip_left_idx} in skin 1")

# Get Body mesh (mesh 1) first primitive
mesh = j['meshes'][1]
prim = mesh['primitives'][0]
attrs = prim['attributes']

pos_data = get_accessor_data_f32(attrs['POSITION'])
joints_data = get_joints_accessor(attrs['JOINTS_0'])
weights_data = get_accessor_data_f32(attrs['WEIGHTS_0'])

n_vertices = j['accessors'][attrs['POSITION']]['count']
print(f"\nBody mesh prim 0: {n_vertices} vertices")

# Find vertices strongly influenced by mHipLeft
hip_influenced = []
for vi in range(n_vertices):
    jts = joints_data[vi]
    wts = weights_data[vi]
    for ii in range(4):
        if jts[ii] == hip_left_idx and wts[ii] > 0.3:
            hip_influenced.append((vi, jts, wts, pos_data[vi]))

print(f"Vertices with >30% mHipLeft influence: {len(hip_influenced)}")

if hip_influenced:
    y_vals = [pos[1] for _, _, _, pos in hip_influenced]
    z_vals = [pos[2] for _, _, _, pos in hip_influenced]
    x_vals = [pos[0] for _, _, _, pos in hip_influenced]
    print(f"X range: {min(x_vals):.3f} to {max(x_vals):.3f}")
    print(f"Y range: {min(y_vals):.3f} to {max(y_vals):.3f}")
    print(f"Z range: {min(z_vals):.3f} to {max(z_vals):.3f}")
    print(f"\nFirst 5 vertices:")
    for vi, jts, wts, pos in hip_influenced[:5]:
        named_joints = [nodes[joints[jt]].get('name', '?') for jt in jts]
        print(f"  V{vi}: pos=({pos[0]:.3f},{pos[1]:.3f},{pos[2]:.3f}) joints={named_joints[:2]} wts={[round(w,2) for w in wts[:2]]}")
    
    # Expected: mHipLeft vertices should be on the left side (positive X, since avatar faces -Z)
    # and in the thigh area (Y between ~0.6 and ~1.1 meters)
    print(f"\nSanity check:")
    print(f"  mHipLeft vertices X > 0: {sum(1 for _, _, _, pos in hip_influenced if pos[0] > 0)}/{len(hip_influenced)}")
    print(f"  (Expected: mostly positive X, as mHipLeft is at +X=0.089 in world space)")

# Also check mKneeLeft
knee_left_idx = None
for ji, joint_node in enumerate(joints):
    if nodes[joint_node].get('name') == 'mKneeLeft':
        knee_left_idx = ji
        break

knee_influenced = []
for vi in range(n_vertices):
    jts = joints_data[vi]
    wts = weights_data[vi]
    for ii in range(4):
        if jts[ii] == knee_left_idx and wts[ii] > 0.3:
            knee_influenced.append((vi, jts, wts, pos_data[vi]))

if knee_influenced:
    y_vals = [pos[1] for _, _, _, pos in knee_influenced]
    print(f"\nmKneeLeft influenced vertices (>30%): {len(knee_influenced)}")
    print(f"  Y range: {min(y_vals):.3f} to {max(y_vals):.3f}")
    print(f"  (Expected: knee area ~0.1 to ~0.6 meters from ground)")
