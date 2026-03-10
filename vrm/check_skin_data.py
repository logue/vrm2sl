"""
Check if JOINTS_0 data correctly references mHipLeft for leg vertices.
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

def get_accessor_data(acc_idx, buf, j):
    acc = j['accessors'][acc_idx]
    bv = j['bufferViews'][acc['bufferView']]
    bv_offset = bv.get('byteOffset', 0)
    acc_offset = acc.get('byteOffset', 0)
    offset = bv_offset + acc_offset
    stride = bv.get('byteStride', 0)
    count = acc['count']
    
    component_type = acc['componentType']
    typ = acc['type']
    
    if typ == 'VEC4' and component_type == 5121:  # UNSIGNED_BYTE
        n = 4
        fmt = '<4B'
        item_size = 4
    elif typ == 'VEC4' and component_type == 5123:  # UNSIGNED_SHORT
        n = 4
        fmt = '<4H'
        item_size = 8
    elif typ == 'VEC4' and component_type == 5126:  # FLOAT
        n = 4
        fmt = '<4f'
        item_size = 16
    elif typ == 'VEC3' and component_type == 5126:  # FLOAT
        n = 3
        fmt = '<3f'
        item_size = 12
    else:
        return None
    
    actual_stride = stride if stride > 0 else item_size
    data = []
    for i in range(count):
        vals = struct.unpack_from(fmt, buf, offset + i * actual_stride)
        data.append(vals)
    return data

# Find skin 1 (Body) mesh
# Find mHipLeft joint index in skin 1
skin = j['skins'][1]
joints = skin['joints']
nodes = j['nodes']

hip_left_idx = None
for ji, joint_node in enumerate(joints):
    if nodes[joint_node].get('name') == 'mHipLeft':
        hip_left_idx = ji
        break

print(f"mHipLeft joint index in skin 1: {hip_left_idx}")

# Find a mesh that uses skin 1
target_primitives = []
for mi, mesh in enumerate(j.get('meshes', [])):
    for pi, prim in enumerate(mesh.get('primitives', [])):
        if prim.get('skin') == 1:
            target_primitives.append((mi, pi, mesh.get('name', '?'), prim))

print(f"Meshes using skin 1: {[(m, p, n) for m, p, n, _ in target_primitives]}")

if target_primitives:
    mi, pi, mesh_name, prim = target_primitives[0]
    
    attrs = prim.get('attributes', {})
    joints_acc = attrs.get('JOINTS_0')
    weights_acc = attrs.get('WEIGHTS_0')
    pos_acc = attrs.get('POSITION')
    
    if joints_acc is None or weights_acc is None or pos_acc is None:
        print("Missing attributes!")
    else:
        joints_data = get_accessor_data(joints_acc, buf, j)
        weights_data = get_accessor_data(weights_acc, buf, j)
        pos_data = get_accessor_data(pos_acc, buf, j)
        
        n_vertices = j['accessors'][joints_acc]['count']
        print(f"\nMesh '{mesh_name}' has {n_vertices} vertices")
        
        # Find vertices most influenced by mHipLeft
        hip_influenced = []
        for vi in range(n_vertices):
            jts = joints_data[vi]
            wts = weights_data[vi]
            for ii in range(4):
                if jts[ii] == hip_left_idx and wts[ii] > 0.1:
                    hip_influenced.append((vi, jts, wts, pos_data[vi]))
        
        print(f"Vertices strongly influenced by mHipLeft (idx={hip_left_idx}): {len(hip_influenced)}")
        
        if hip_influenced:
            print("\nFirst 5 hip-left influenced vertices (pos, joints, weights):")
            for vi, jts, wts, pos in hip_influenced[:5]:
                named_joints = [nodes[joints[j]].get('name', '?') for j in jts]
                print(f"  Vertex {vi}: pos=({pos[0]:.3f},{pos[1]:.3f},{pos[2]:.3f})")
                print(f"    joints={named_joints}")
                print(f"    weights={[round(w,3) for w in wts]}")
        
        # Check if mHipLeft influenced vertices are in the expected Y range
        # (thigh area: Y between ~0.6 and ~1.1 meters)
        if hip_influenced:
            y_vals = [pos[1] for _, _, _, pos in hip_influenced]
            print(f"\nmHipLeft-influenced vertex Y range: {min(y_vals):.3f} to {max(y_vals):.3f}")
            print(f"Expected Y range for thigh area: ~0.6 to ~1.1 meters")
