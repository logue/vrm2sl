import struct, json, math

def read_glb(path):
    with open(path, 'rb') as f:
        data = f.read()
    json_len = struct.unpack('<I', data[12:16])[0]
    gltf = json.loads(data[20:20+json_len])
    bin_off = 20 + json_len
    if bin_off % 4 != 0:
        bin_off += 4 - (bin_off % 4)
    bin_off += 8
    return gltf, data[bin_off:]

def mat4_mul(a, b):
    r = [[0]*4 for _ in range(4)]
    for i in range(4):
        for j in range(4):
            for k in range(4):
                r[i][j] += a[i][k] * b[k][j]
    return r

def trans_mat(tx, ty, tz):
    return [[1,0,0,tx],[0,1,0,ty],[0,0,1,tz],[0,0,0,1]]

def rot_x(angle_rad):
    c = math.cos(angle_rad)
    s = math.sin(angle_rad)
    return [[1,0,0,0],[0,c,-s,0],[0,s,c,0],[0,0,0,1]]

def mat4_transform(m, v):
    x = m[0][0]*v[0] + m[0][1]*v[1] + m[0][2]*v[2] + m[0][3]
    y = m[1][0]*v[0] + m[1][1]*v[1] + m[1][2]*v[2] + m[1][3]
    z = m[2][0]*v[0] + m[2][1]*v[1] + m[2][2]*v[2] + m[2][3]
    return (x, y, z)

gltf, bin_data = read_glb('/tmp/avatar_new.glb')
nodes = gltf['nodes']

def find_node(name):
    for i, n in enumerate(nodes):
        if n.get('name') == name:
            return i
    return None

def get_node_trans(node):
    return node.get('translation', [0,0,0])

def build_world_matrix(node_idx, parent_world=None):
    if parent_world is None:
        parent_world = [[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]
    n = nodes[node_idx]
    t = n.get('translation', [0,0,0])
    local = trans_mat(t[0], t[1], t[2])
    return mat4_mul(parent_world, local)

root_idx = find_node('Root')
pelvis_idx = find_node('mPelvis')
hip_left_idx = find_node('mHipLeft')
knee_left_idx = find_node('mKneeLeft')

print("Nodes:", root_idx, pelvis_idx, hip_left_idx, knee_left_idx)

world_root = [[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]
world_pelvis = build_world_matrix(pelvis_idx, world_root)
world_hip = build_world_matrix(hip_left_idx, world_pelvis)
world_knee = build_world_matrix(knee_left_idx, world_hip)

print("\nWorld positions (T-pose):")
print("  mPelvis: (%.4f, %.4f, %.4f)" % (world_pelvis[0][3], world_pelvis[1][3], world_pelvis[2][3]))
print("  mHipLeft: (%.4f, %.4f, %.4f)" % (world_hip[0][3], world_hip[1][3], world_hip[2][3]))
print("  mKneeLeft: (%.4f, %.4f, %.4f)" % (world_knee[0][3], world_knee[1][3], world_knee[2][3]))

hip_world = (world_hip[0][3], world_hip[1][3], world_hip[2][3])
ibm_hip = trans_mat(-hip_world[0], -hip_world[1], -hip_world[2])

angle = math.radians(31.8)
t_local = get_node_trans(nodes[hip_left_idx])
anim_local = mat4_mul(trans_mat(t_local[0], t_local[1], t_local[2]), rot_x(angle))
anim_world_hip = mat4_mul(world_pelvis, anim_local)

print("\nAnimated mHipLeft world pos: (%.4f, %.4f, %.4f)" % (anim_world_hip[0][3], anim_world_hip[1][3], anim_world_hip[2][3]))

skin_mat = mat4_mul(anim_world_hip, ibm_hip)
print("\nSkin matrix for mHipLeft with 31.8 deg Xrot:")
for row in skin_mat:
    print("  [%.3f, %.3f, %.3f, %.3f]" % (row[0], row[1], row[2], row[3]))

knee_world = (world_knee[0][3], world_knee[1][3], world_knee[2][3])
test_vertex = ((hip_world[0]+knee_world[0])/2, (hip_world[1]+knee_world[1])/2, (hip_world[2]+knee_world[2])/2)
print("\nTest vertex (mid-thigh): (%.4f, %.4f, %.4f)" % test_vertex)

deformed = mat4_transform(skin_mat, list(test_vertex))
print("After deformation:        (%.4f, %.4f, %.4f)" % deformed)

delta = (deformed[0]-test_vertex[0], deformed[1]-test_vertex[1], deformed[2]-test_vertex[2])
print("Delta (X, Y, Z):          (%.4f, %.4f, %.4f)" % delta)
print("\nZ delta>0 => leg moved backward (toward +Z)")
print("Z delta<0 => leg moved forward (toward -Z, char faces -Z)")
print("X delta dominates => CRAB WALK")
