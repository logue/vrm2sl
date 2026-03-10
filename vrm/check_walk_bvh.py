#!/usr/bin/env python3
"""Check walk BVH hip rotation values"""
import math
import sys

def quat_mul(q1, q2):
    x1, y1, z1, w1 = q1
    x2, y2, z2, w2 = q2
    return (
        w1*x2 + x1*w2 + y1*z2 - z1*y2,
        w1*y2 - x1*z2 + y1*w2 + z1*x2,
        w1*z2 + x1*y2 - y1*x2 + z1*w2,
        w1*w2 - x1*x2 - y1*y2 - z1*z2
    )

def aq(ax, ay, az, a):
    s = math.sin(a / 2)
    return (ax*s, ay*s, az*s, math.cos(a/2))

def qe(q):
    x, y, z, w = q
    r = math.atan2(2*(w*x+y*z), 1-2*(x*x+y*y))
    p = math.asin(max(-1, min(1, 2*(w*y-z*x))))
    ya = math.atan2(2*(w*z+x*y), 1-2*(y*y+z*z))
    return (math.degrees(r), math.degrees(p), math.degrees(ya))

paths = sys.argv[1:] if len(sys.argv) > 1 else [
    '/Users/logue/Developer/vrm2sl/frontend/public/animations/avatar_walk.bvh',
    '/Users/logue/Developer/vrm2sl/frontend/public/animations/avatar_female_walk.bvh',
]

for path in paths:
    with open(path) as f:
        content = f.read()

    mi = content.index('MOTION')
    header = content[:mi]
    motion = content[mi:]

    channels = []
    for line in header.split('\n'):
        line = line.strip()
        if line.startswith('CHANNELS'):
            p = line.split()
            n = int(p[1])
            channels.extend(p[2:2+n])

    lines = motion.strip().split('\n')
    for i, line in enumerate(lines):
        if line.startswith('Frame Time:'):
            first = lines[i+1].split()
            break

    vals = [float(v) for v in first[:9]]
    q = (0, 0, 0, 1)
    for idx in range(3, 6):
        a = math.radians(vals[idx])
        ch = channels[idx]
        if ch == 'Xrotation':
            dq = aq(1, 0, 0, a)
        elif ch == 'Yrotation':
            dq = aq(0, 1, 0, a)
        else:
            dq = aq(0, 0, 1, a)
        q = quat_mul(q, dq)
    e = qe(q)

    print(f'{path.split("/")[-1]}:')
    print(f'  hip pos=({vals[0]:.2f},{vals[1]:.2f},{vals[2]:.2f})')
    print(f'  hip quat=({q[0]:.4f},{q[1]:.4f},{q[2]:.4f},{q[3]:.4f})')
    print(f'  hip euler_deg=({e[0]:.1f},{e[1]:.1f},{e[2]:.1f})')
    print(f'  abdomen rot=({channels[6]}={vals[6]:.2f},{channels[7]}={vals[7]:.2f},{channels[8]}={vals[8]:.2f})')

    # Check all unique bones
    print(f'  Total channels: {len(channels)}')
    print(f'  Channel sample: {channels[:6]}')
    print()
