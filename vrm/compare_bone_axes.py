#!/usr/bin/env python3
import math

# BVH SL default skeleton: lThigh -> lShin direction
# lShin OFFSET (from lThigh): -1.359117, -18.918689, 1.179887
bvh_shin_vec = (-1.359117, -18.918689, 1.179887)
bvh_len = math.sqrt(sum(x**2 for x in bvh_shin_vec))
bvh_dir = tuple(x/bvh_len for x in bvh_shin_vec)
print(f'BVH lThigh->lShin direction: ({bvh_dir[0]:.6f}, {bvh_dir[1]:.6f}, {bvh_dir[2]:.6f})')
print(f'BVH length: {bvh_len:.4f} cm')
bvh_angle_from_neg_y = math.degrees(math.acos(-bvh_dir[1]))
print(f'BVH angle from -Y: {bvh_angle_from_neg_y:.4f} deg')

# GLB mHipLeft -> mKneeLeft direction
glb_knee_vec = (0, -0.4682, -0.0098)
glb_len = math.sqrt(sum(x**2 for x in glb_knee_vec))
glb_dir = tuple(x/glb_len for x in glb_knee_vec)
print()
print(f'GLB mHipLeft->mKneeLeft direction: ({glb_dir[0]:.6f}, {glb_dir[1]:.6f}, {glb_dir[2]:.6f})')
print(f'GLB length: {glb_len:.4f} m = {glb_len*100:.4f} cm')
glb_angle_from_neg_y = math.degrees(math.acos(-glb_dir[1]))
print(f'GLB angle from -Y: {glb_angle_from_neg_y:.4f} deg')

print()
print(f'BVH XZ offset from -Y: dx={bvh_dir[0]:.6f}, dz={bvh_dir[2]:.6f}')
print(f'GLB XZ offset from -Y: dx={glb_dir[0]:.6f}, dz={glb_dir[2]:.6f}')

print()
print("=== Key Analysis ===")
print(f"BVH lThigh X offset: {bvh_shin_vec[0]:.4f} cm  (lateral offset)")
print(f"BVH lThigh Z offset: {bvh_shin_vec[2]:.4f} cm  (fore-aft offset, small)")
print(f"GLB mHipLeft->mKnee Y: {glb_knee_vec[1]:.4f} m = {glb_knee_vec[1]*100:.4f} cm")
print(f"GLB mHipLeft->mKnee Z: {glb_knee_vec[2]:.4f} m = {glb_knee_vec[2]*100:.4f} cm")

# Rotation quaternion when X-rotation 31.8 deg is applied to BVH-authored rest pose vs GLB rest pose
angle = math.radians(31.8)
# For a bone pointing in -Y direction (BVH), X rotation produces movement in Z direction
# For a bone pointing slightly in -Z direction too (GLB), X rotation still mostly moves in Z direction
# Let's check the actual axis relationship

# Check if there's a crab walk issue by seeing what direction the BVH rotation moves each bone
print()
print("=== Walk frame analysis (lThigh X=31.8 deg) ===")
# In BVH: lThigh bone direction (from hip to knee) = normalized(lShin OFFSET)
# = (-0.0718, -0.9979, 0.0623)
# An X rotation rotates around X axis: y' = y*cos - z*sin, z' = y*sin + z*cos
# Applied to the bone direction vector:
bvh_y = bvh_dir[1]
bvh_z = bvh_dir[2]
new_y = bvh_y * math.cos(angle) - bvh_z * math.sin(angle)
new_z = bvh_y * math.sin(angle) + bvh_z * math.cos(angle)
print(f"BVH: after 31.8 deg X rot on thigh direction:")
print(f"  Y: {bvh_dir[1]:.6f} -> {new_y:.6f}  (change: {new_y - bvh_dir[1]:.6f})")
print(f"  Z: {bvh_dir[2]:.6f} -> {new_z:.6f}  (change: {new_z - bvh_dir[2]:.6f})")
print(f"  => Knee moves in Z (forward) by: {(new_z - bvh_dir[2]) * bvh_len:.4f} cm")
