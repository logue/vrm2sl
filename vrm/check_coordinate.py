import struct, json, math, copy

# -90 degree X rotation: q = (sin(-45°), 0, 0, cos(-45°)) = (-0.7071, 0, 0, 0.7071)
# But wait: rotating -90 around X maps Y-up to Z-up:
# Y axis (0,1,0) → becomes Z axis (0,0,1) after -90° X rotation
# Z axis (0,0,1) → becomes -Y axis (0,-1,0)

# Actually for Y-up to Z-up: rotate +90 around X
# q = (sin(45°), 0, 0, cos(45°)) = (0.7071, 0, 0, 0.7071)
# But test: does SL actually need this?

# Let me just print the current Root rotation and what it would look like with -90X applied
with open('/tmp/avatar_new.glb', 'rb') as f:
    f.read(12)
    json_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    j = json.loads(f.read(json_len))
    bin_start = 12 + 8 + json_len + 8
    f.seek(bin_start)
    buf = f.read()

nodes = j['nodes']
scene_nodes = j['scenes'][0]['nodes']

print("Scene root nodes:")
for ni in scene_nodes:
    n = nodes[ni]
    print(f"  Node {ni} ({n.get('name','?')}): t={n.get('translation',[0,0,0])}, r={n.get('rotation',[0,0,0,1])}")
    for ci in n.get('children', []):
        cn = nodes[ci]
        print(f"    Child {ci} ({cn.get('name','?')}): t={cn.get('translation',[0,0,0])}, r={cn.get('rotation',[0,0,0,1])}")

# Check if SL needs Z-up by reading what axis the mHipLeft-mKneeLeft direction is
# Hip left world pos
def world_pos(idx):
    pos = [0.0, 0.0, 0.0]
    cur = idx
    chain = []
    while cur is not None:
        chain.append(cur)
        parent_map_local = {}
        for i, node in enumerate(nodes):
            for child in node.get('children', []):
                parent_map_local[child] = i
        break
    
    # Build parent map
    parent_map = {}
    for i, node in enumerate(nodes):
        for child in node.get('children', []):
            parent_map[child] = i
    
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

# Find node indices for key bones
bone_idx = {}
for i, n in enumerate(nodes):
    name = n.get('name', '')
    if name in ('mHipLeft', 'mKneeLeft', 'mAnkleLeft', 'mPelvis', 'mTorso', 'mHead'):
        bone_idx[name] = i

print("\nKey bone world positions:")
for name in ['mPelvis', 'mTorso', 'mHead', 'mHipLeft', 'mKneeLeft', 'mAnkleLeft']:
    if name in bone_idx:
        wp = world_pos(bone_idx[name])
        print(f"  {name}: ({wp[0]:.4f}, {wp[1]:.4f}, {wp[2]:.4f})")

# If SL uses Z-up internally:
# glTF Y → SL Z (height)
# glTF Z → SL -Y
# glTF X → SL X
# So the avatar's height would be in Z in SL's reference.
# The BVH hip at Y=40 (cm) would be Z=40 in SL.
# But SL internally stores avatar at Z=0 ground, not Z=40.
# This calculation shows the reference frame relationship.

print("\n--- IF SL applies Y-up to Z-up conversion to the glTF: ---")
print("Then glTF bone positions would be reinterpreted as:")
for name in ['mPelvis', 'mHipLeft', 'mKneeLeft']:
    if name in bone_idx:
        wp = world_pos(bone_idx[name])
        x, y, z = wp
        # Y-up to Z-up: new_x = x, new_y = -z, new_z = y
        print(f"  {name}: glTF=({x:.4f},{y:.4f},{z:.4f}) → SL_Zup=({x:.4f},{-z:.4f},{y:.4f})")

print("\n--- SL avatar_skeleton.xml DEFAULT bone positions (from SL wiki) ---")
# Known SL default positions (in SL coordinate space, Z-up):
sl_defaults = {
    'mPelvis': (0, 0, 1.045),  # Z=height
    'mHipLeft': (-0.0850, 0, 0),  # relative to mPelvis, X=-left
    'mKneeLeft': (0, 0, -0.4380),  # relative to mHipLeft
}
print("(Approximate SL default skeleton Z-up positions)")
for name, pos in sl_defaults.items():
    print(f"  {name}: {pos}")
