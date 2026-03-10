#!/usr/bin/env python3
"""BVH animation data checker"""
import math

def quat_multiply(q1, q2):
    x1, y1, z1, w1 = q1
    x2, y2, z2, w2 = q2
    return (
        w1*x2 + x1*w2 + y1*z2 - z1*y2,
        w1*y2 - x1*z2 + y1*w2 + z1*x2,
        w1*z2 + x1*y2 - y1*x2 + z1*w2,
        w1*w2 - x1*x2 - y1*y2 - z1*z2
    )

def axis_angle_to_quat(ax, ay, az, angle_rad):
    s = math.sin(angle_rad / 2)
    return (ax * s, ay * s, az * s, math.cos(angle_rad / 2))

def quat_to_euler_deg(q):
    x, y, z, w = q
    # roll (x), pitch (y), yaw (z)
    sinr_cosp = 2 * (w * x + y * z)
    cosr_cosp = 1 - 2 * (x * x + y * y)
    roll = math.atan2(sinr_cosp, cosr_cosp)

    sinp = 2 * (w * y - z * x)
    sinp = max(-1, min(1, sinp))
    pitch = math.asin(sinp)

    siny_cosp = 2 * (w * z + x * y)
    cosy_cosp = 1 - 2 * (y * y + z * z)
    yaw = math.atan2(siny_cosp, cosy_cosp)

    return (math.degrees(roll), math.degrees(pitch), math.degrees(yaw))

with open('/Users/logue/Developer/vrm2sl/frontend/public/animations/avatar_stand_1.bvh', 'r') as f:
    content = f.read()

motion_idx = content.index('MOTION')
header = content[:motion_idx]
motion = content[motion_idx:]

# チャンネル順序を取得
channels = []
for line in header.split('\n'):
    line = line.strip()
    if line.startswith('CHANNELS'):
        parts = line.split()
        count = int(parts[1])
        channels.extend(parts[2:2+count])

print(f'Total channels: {len(channels)}')
print(f'First 10 channels: {channels[:10]}')

lines = motion.strip().split('\n')
for i, line in enumerate(lines):
    if line.startswith('Frame Time:'):
        first_frame = lines[i+1].split()
        break

values = [float(v) for v in first_frame[:6]]
print(f'Hip position: ({values[0]:.4f}, {values[1]:.4f}, {values[2]:.4f})')
print(f'Hip rotation (raw): {channels[3]}={values[3]:.4f}, {channels[4]}={values[4]:.4f}, {channels[5]}={values[5]:.4f} deg')

# BVHLoaderがやる処理を再現:
# チャンネル順序に従って順次乗算
ch_map = {channels[3]: values[3], channels[4]: values[4], channels[5]: values[5]}
q = (0, 0, 0, 1)
for ch_name in [channels[3], channels[4], channels[5]]:
    angle = math.radians(ch_map[ch_name])
    if ch_name == 'Xrotation':
        dq = axis_angle_to_quat(1, 0, 0, angle)
    elif ch_name == 'Yrotation':
        dq = axis_angle_to_quat(0, 1, 0, angle)
    elif ch_name == 'Zrotation':
        dq = axis_angle_to_quat(0, 0, 1, angle)
    q = quat_multiply(q, dq)

print(f'Hip quaternion (BVHLoader): xyzw=({q[0]:.4f}, {q[1]:.4f}, {q[2]:.4f}, {q[3]:.4f})')
euler = quat_to_euler_deg(q)
print(f'Hip euler (degrees): roll={euler[0]:.2f}, pitch={euler[1]:.2f}, yaw={euler[2]:.2f}')
print()

# hip以外の最初のボーン (abdomen) の回転も確認
# abdomenはインデックス6-8のチャンネル
values_ab = [float(v) for v in first_frame[6:9]]
print(f'Abdomen rotation (raw): {channels[6]}={values_ab[0]:.4f}, {channels[7]}={values_ab[1]:.4f}, {channels[8]}={values_ab[2]:.4f} deg')
