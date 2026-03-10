#!/usr/bin/env python3
"""Check GLB node rotations and skin data"""
import struct, json, sys, math

def quat_to_euler_deg(q):
    x, y, z, w = q
    sinr_cosp = 2 * (w * x + y * z)
    cosr_cosp = 1 - 2 * (x * x + y * y)
    roll = math.atan2(sinr_cosp, cosr_cosp)
    sinp = max(-1, min(1, 2 * (w * y - z * x)))
    pitch = math.asin(sinp)
    siny_cosp = 2 * (w * z + x * y)
    cosy_cosp = 1 - 2 * (y * y + z * z)
    yaw = math.atan2(siny_cosp, cosy_cosp)
    return (math.degrees(roll), math.degrees(pitch), math.degrees(yaw))

path = sys.argv[1] if len(sys.argv) > 1 else '/tmp/avatar_test.glb'
with open(path, 'rb') as f:
    data = f.read()

magic = struct.unpack_from('<I', data, 0)[0]
assert magic == 0x46546c67, "Not a GLB"
json_len = struct.unpack_from('<I', data, 12)[0]
json_data = json.loads(data[20:20+json_len])

nodes = json_data.get('nodes', [])
print("=== Key bone rotations (local, from node.rotation) ===")
sl_bones = {'mPelvis','mTorso','mChest','mNeck','mHead','mShoulderLeft','mShoulderRight','mHipLeft','mHipRight'}
for i, node in enumerate(nodes):
    name = node.get('name','')
    if name in sl_bones:
        rot = node.get('rotation', [0,0,0,1])
        trans = node.get('translation', [0,0,0])
        euler = quat_to_euler_deg(rot)
        print(f"  {name}: euler=({euler[0]:.1f},{euler[1]:.1f},{euler[2]:.1f}) trans=({trans[0]:.4f},{trans[1]:.4f},{trans[2]:.4f})")

skins = json_data.get('skins', [])
print(f"\n=== Skins ({len(skins)}) ===")
for i, skin in enumerate(skins):
    print(f"  Skin {i}: skeleton={skin.get('skeleton')}, joints={len(skin.get('joints',[]))}")
