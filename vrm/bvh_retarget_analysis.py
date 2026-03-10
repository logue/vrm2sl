#!/usr/bin/env python3
import math

def axis_angle_quat(ax, ay, az, angle_deg):
    a = math.radians(angle_deg)
    s = math.sin(a/2)
    c = math.cos(a/2)
    return (ax*s, ay*s, az*s, c)

def quat_mul(q1, q2):
    x1,y1,z1,w1 = q1
    x2,y2,z2,w2 = q2
    return (
        w1*x2+x1*w2+y1*z2-z1*y2,
        w1*y2-x1*z2+y1*w2+z1*x2,
        w1*z2+x1*y2-y1*x2+z1*w2,
        w1*w2-x1*x2-y1*y2-z1*z2
    )

def rotate_vec_by_quat(v, q):
    vq = (v[0], v[1], v[2], 0)
    qc = (-q[0], -q[1], -q[2], q[3])
    t = quat_mul(q, vq)
    r = quat_mul(t, qc)
    return (r[0], r[1], r[2])

# BVH for hip frame 1: Xrot=5.556, Zrot=0, Yrot=2.016
q = (0,0,0,1)
q = quat_mul(q, axis_angle_quat(1,0,0, 5.556919))
q = quat_mul(q, axis_angle_quat(0,0,1, 0.000000))
q = quat_mul(q, axis_angle_quat(0,1,0, 2.015870))
print("BVH hip frame 1 rotation quaternion (X,Y,Z,W):", [round(x,6) for x in q])

# Total channels count
total = 6 + 3*6 + 3*4 + 3*4 + 3*3 + 3*3
print("Total channels:", total)
# lThigh starts at channel: hip(6) + body_chain(3*6) + lcollar..lhand(3*4) + rcollar..rhand(3*4)
lthigh_offset = 6 + 3*6 + 3*4 + 3*4
print("lThigh rotation starts at channel index:", lthigh_offset)

frame1 = "-0.025751 40.586620 2.968130 5.556919 0.000000 2.015870 -0.679723 1.014860 -1.052490 2.752000 0.638850 -7.299640 0.000000 0.000000 0.000000 0.407942 0.012259 -0.125621 5.754920 -1.457380 3.998490 0.000000 0.000000 0.000000 -9.241052 -2.655120 -8.994716 -71.740334 -11.761992 4.746030 -31.604246 0.001165 34.045898 -13.209469 -13.282380 0.089536 -5.574724 0.471018 -0.003654 73.497147 -23.637768 -7.243340 8.720032 0.116349 30.201418 6.934624 -2.351367 0.271411 19.708641 0.263537 -1.303549 16.376945 -5.436259 -0.534536 4.496281 1.015851 1.747118 -34.913624 -1.222353 -5.287092 -2.923005 1.339154 -2.612716 22.631870 -2.693362 0.441635"
vals = [float(x) for x in frame1.split()]
print("Total values:", len(vals))
lthigh_x = vals[lthigh_offset]
lthigh_z = vals[lthigh_offset+1]
lthigh_y = vals[lthigh_offset+2]
print("lThigh channels: X=%.4f, Z=%.4f, Y=%.4f" % (lthigh_x, lthigh_z, lthigh_y))

# lThigh quaternion (channel order: Xrotation Zrotation Yrotation)
q_lt = (0,0,0,1)
q_lt = quat_mul(q_lt, axis_angle_quat(1,0,0, lthigh_x))
q_lt = quat_mul(q_lt, axis_angle_quat(0,0,1, lthigh_z))
q_lt = quat_mul(q_lt, axis_angle_quat(0,1,0, lthigh_y))
print("lThigh quat (X,Y,Z,W):", [round(x,6) for x in q_lt])

# What does this rotation do to the bone direction?
# BVH lThigh default direction (lShin child offset): (-1.359, -18.919, 1.180)
# Normalize:
bvh_bone_dir = (-1.359117, -18.918689, 1.179887)
bvh_len = math.sqrt(sum(x**2 for x in bvh_bone_dir))
bvh_dir = tuple(x/bvh_len for x in bvh_bone_dir)
print("BVH lThigh bone direction:", [round(x,6) for x in bvh_dir])

# Apply lThigh rotation to the bone direction
rotated_bvh = rotate_vec_by_quat(bvh_dir, q_lt)
print("BVH lThigh rotated direction:", [round(x,6) for x in rotated_bvh])
print("Z change: %.4f (positive=moved toward +Z)" % rotated_bvh[2])

print()
# Now do same for GLB (mHipLeft -> mKneeLeft direction: (0, -0.4682, -0.0098))
glb_bone_dir = (0, -0.4682, -0.0098)
glb_len = math.sqrt(sum(x**2 for x in glb_bone_dir))
glb_dir = tuple(x/glb_len for x in glb_bone_dir)
print("GLB mHipLeft bone direction:", [round(x,6) for x in glb_dir])

# Apply SAME quaternion (directly copied from BVH) to GLB bone direction
rotated_glb = rotate_vec_by_quat(glb_dir, q_lt)
print("GLB mHipLeft rotated direction (if BVH quat applied directly):", [round(x,6) for x in rotated_glb])
print("Z change: %.4f (positive=moved toward +Z, negative=toward -Z)" % rotated_glb[2])
print()
print("ANALYSIS:")
print("BVH bone: Z after rotation = %.4f (should be negative for forward step)" % rotated_bvh[2])
print("GLB bone: Z after rotation = %.4f" % rotated_glb[2])
print("Both agree on direction?", (rotated_bvh[2] < 0) == (rotated_glb[2] < 0))
