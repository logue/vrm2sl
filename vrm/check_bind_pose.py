#!/usr/bin/env python3
"""
Check world-space bind poses from IBM (inverse bind matrix).
Also check arm angles to determine if model is in T-pose or A-pose.
"""
import struct, json, sys, math

def mat4_inv(m):
    """Invert 4x4 matrix (column-major)"""
    # Convert column-major to row-major for easier computation
    r = [[m[c*4+r] for c in range(4)] for r in range(4)]
    # 3x3 inverse (rotation/scale part)
    det = (r[0][0]*(r[1][1]*r[2][2]-r[1][2]*r[2][1])
          -r[0][1]*(r[1][0]*r[2][2]-r[1][2]*r[2][0])
          +r[0][2]*(r[1][0]*r[2][1]-r[1][1]*r[2][0]))
    if abs(det) < 1e-10:
        return None
    # Just return the translation (column 3) for world position
    return (-m[12], -m[13], -m[14])  # simplified, not true inverse

def read_mat4_from_accessor(data, accessor, buffer_views, buffers, buffer_data):
    bv_idx = accessor.get('bufferView')
    if bv_idx is None:
        return None
    bv = buffer_views[bv_idx]
    buf_offset = bv.get('byteOffset', 0) + accessor.get('byteOffset', 0)
    count = accessor['count']
    mats = []
    for i in range(count):
        offset = buf_offset + i * 64  # 16 floats * 4 bytes
        mat = struct.unpack_from('<16f', buffer_data, offset)
        mats.append(mat)
    return mats

path = sys.argv[1] if len(sys.argv) > 1 else '/tmp/avatar_test.glb'
with open(path, 'rb') as f:
    data = f.read()

magic = struct.unpack_from('<I', data, 0)[0]
assert magic == 0x46546c67, "Not a GLB"
json_len = struct.unpack_from('<I', data, 12)[0]
json_data = json.loads(data[20:20+json_len])

# Get binary chunk
bin_offset = 20 + json_len
if bin_offset % 4 != 0:
    bin_offset += 4 - (bin_offset % 4)
# Skip the 8-byte chunk header
bin_offset += 8
bin_data = data[bin_offset:]

nodes = json_data.get('nodes', [])
skins = json_data.get('skins', [])
accessors = json_data.get('accessors', [])
buffer_views = json_data.get('bufferViews', [])
buffers = json_data.get('buffers', [])

# Get main body skin (skin 1, with 47 joints)
body_skin = None
for skin in skins:
    if len(skin.get('joints', [])) > 10:
        body_skin = skin
        break

if not body_skin:
    print("No body skin found")
    sys.exit(1)

ibm_accessor_idx = body_skin.get('inverseBindMatrices')
ibm_accessor = accessors[ibm_accessor_idx]
ibm_mats = read_mat4_from_accessor(bin_data, ibm_accessor, buffer_views, buffers, bin_data)

joints = body_skin.get('joints', [])
print(f"Body skin joints: {len(joints)}")
print()

# For each joint, compute world position from IBM column 4
print("World positions from IBM (negated translation column):")
arm_bones = ['mShoulderLeft', 'mShoulderRight', 'mElbowLeft', 'mElbowRight',
             'mHipLeft', 'mHipRight', 'mKneeLeft', 'mKneeRight',
             'mPelvis', 'mTorso', 'mChest', 'mNeck', 'mHead']
for i, joint_idx in enumerate(joints):
    name = nodes[joint_idx].get('name', f'node_{joint_idx}')
    if name in arm_bones:
        mat = ibm_mats[i]
        # IBM = inverse(bindMatrix), so translation in world space = -(IBM rotation portion * IBM translation)
        # For a pure translation bind matrix, world_pos = (-mat[12], -mat[13], -mat[14])
        # But we need to apply the full inverse properly
        # World pos = -R^T * t where t is column 3 of IBM and R is rotation part
        # Simpler: world pos is where the joint was in world space at bind time
        # col4 of the joint's world matrix = -(IBM^(-1) extraction)
        # Actually: IBM * world_pos = 0 => world_pos = -IBM_rotation_T * IBM_translation
        # Since joints have identity local rotation in our case, just negate translation:
        wx = -(mat[0]*mat[12] + mat[1]*mat[13] + mat[2]*mat[14])
        wy = -(mat[4]*mat[12] + mat[5]*mat[13] + mat[6]*mat[14])
        wz = -(mat[8]*mat[12] + mat[9]*mat[13] + mat[10]*mat[14])
        print(f"  {name}: world=({wx:.4f},{wy:.4f},{wz:.4f})")

print()
# Check shoulder positions to determine T-pose vs A-pose
print("Checking for T-pose vs A-pose:")
for i, joint_idx in enumerate(joints):
    name = nodes[joint_idx].get('name', '')
    if name in ('mShoulderLeft', 'mShoulderRight'):
        mat = ibm_mats[i]
        wx = -(mat[0]*mat[12] + mat[1]*mat[13] + mat[2]*mat[14])
        wy = -(mat[4]*mat[12] + mat[5]*mat[13] + mat[6]*mat[14])
        wz = -(mat[8]*mat[12] + mat[9]*mat[13] + mat[10]*mat[14])
        # In T-pose: shoulder y ≈ chest y (arms horizontal)
        # In A-pose: shoulder y is lower (arms at ~45 degrees)
        print(f"  {name}: world pos = ({wx:.4f}, {wy:.4f}, {wz:.4f})")
