#!/usr/bin/env python3
"""
Investigate the axis system to understand the crab walk issue.
"""
import math

# Walk BVH hip positions across frames:
frames_pos = [
    (-0.025751, 40.586620, 2.968130),  # frame 1
    (-0.042716, 39.998322, 2.968130),  # frame 2
    (-0.049000, 40.603870, 2.968130),  # frame 3
    (-0.037075, 41.810562, 2.968130),  # frame 4
    (-0.049278, 42.520454, 2.968130),  # frame 5
    (-0.114791, 42.307247, 2.968130),  # frame 6
    (-0.113669, 41.299294, 2.968130),  # frame 7
    (-0.025751, 40.586620, 2.968130),  # frame 8 (loop)
]
print("Hip positions across walk frames:")
for i, pos in enumerate(frames_pos):
    print("  Frame %d: X=%.4f Y=%.4f Z=%.4f" % (i+1, pos[0], pos[1], pos[2]))

print()
print("Z is CONSTANT -> walk animation is in-place, no root translation")
print()

# In BVH: figure was exported facing which direction?
# Left shoulder (lCollar->lShldr): OFFSET 6.421198 0.010146 -0.332128
# Right shoulder: OFFSET -rX (mirror), so LEFT shoulder is at +X
# This means: avatar's LEFT side is +X
# With Y-up: LEFT=+X, UP=+Y, therefore FORWARD = -Z (right-hand rule: X cross Y = Z)
# Wait: X cross Y = (1,0,0) cross (0,1,0) = (0,0,1) = +Z
# So LEFT(+X) x UP(+Y) = +Z -> avatar's FORWARD would be +Z? No...
# If left hand is +X and up is +Y, the forward direction (cross of -left and up)...
# Actually: if character faces toward viewer (standard), with left=+X and up=+Y:
#   forward = +X cross +Y = nope
# Standard character orientation: face toward +Z with left on +X, up on +Y
#   OR face toward -Z with right on +X, up on +Y

# SL BVH lCollar (LEFT collar): OFFSET  0.599237 8.316447 0.784897 from chest
# lShldr (LEFT shoulder): OFFSET  6.421198 0.010146 -0.332128 from lCollar
# lForeArm (LEFT elbow): OFFSET  10.552783 0.025574 0.125508 from lShldr
# So LEFT arm extends in +X direction
# In typical character setup (facing -Z, right-hand):
#   right hand is +X, left hand is -X
#   But here LEFT is +X???

# Let's check right side:
# rCollar: OFFSET  -0.599237 8.316447 0.784897 (NEGATIVE X!)
# rShldr: depends on BVH...
# If rCollar is at -X and lCollar is at +X, then character's LEFT is +X
# In right-hand Y-up: if LEFT=+X, UP=+Y, then FORWARD = +Z cross +Y = ... 
# Actually: forward = LEFT cross UP = +X cross +Y = +Z  -> NO

# More directly: Standard human anatomy with Y-up, facing +Z:
#   Right hand = +X, Left hand = -X
# Standard human anatomy with Y-up, facing -Z:
#   Right hand = -X, Left hand = +X

# BVH lCollar is at +X -> character's LEFT side is at +X
# Therefore: character FACES -Z (like VRM/glTF!)

print("SL BVH character orientation analysis:")
print("  lCollar at +X = character's left side is at +X")
print("  => Character faces -Z (same as VRM/glTF!)")
print()
print("KEY RESULT: SL BVH and glTF use the SAME forward direction (-Z)")
print("  => Direct quaternion copy should work for left/right and forward/backward")
print("  => If crab walk occurs, it means X/Z axes are SWAPPED somehow")
print()

# Let's double check by looking at leg positions:
# lThigh OFFSET from hip: (4.500466, -6.400484, -1.832696) 
# Character is left-handed in X direction (+X = left side)
# lThigh is at +X (4.5cm), -Y (6.4cm down), -Z (1.83cm behind)
# This makes sense: left leg is at +X (left side of body)

print("lThigh (LEFT leg) is at +X = %.4f cm (character faces -Z)" % 4.500466)
print("=> LEFT thigh is at +X: CORRECT for -Z facing character")
print()

# Now the critical question: is there a Y-axis 180-degree issue?
# Some exporters/importers have Y-up vs Z-up coordinate mismatch
# glTF is Y-up right-hand
# BVH in SL is Y-up right-hand
# If they match, direct quaternion copy should work

# However, Three.js BVHLoader processes rotations using WORLD axes
# The BVH assumes: X=right (from char perspective = -X from world?)
# No, wait. The BVH is in WORLD space

# Actually, BVHLoader uses vx=(1,0,0), vy=(0,1,0), vz=(0,0,1) 
# These are WORLD axes, independent of bone orientation
# The resulting quaternion q = Rx * Rz * Ry (in that order for hip)
# represents the LOCAL rotation of the bone relative to its parent

# In BVH, ALL parent transforms are accumulated from root
# Actually in BVH, each bone's rotation is expressed as a LOCAL rotation
# applied after the parent's accumulated rotation

# For a hip bone (root): no parent, so rotation IS the world rotation
# For lThigh: rotation is relative to hip's LOCAL frame

# THIS is the key: when BVH says lThigh Xrotation=31.8,
# it means rotate 31.8 degrees around WORLD X axis IN LOCAL SPACE
# which is equivalent to: the LOCAL rotation expressed as world-space angles

# When we apply this to Three.js GLB model:
# The THREE.Bone for mHipLeft has the SAME local space as the BVH lThigh
# ONLY if they both have identity parent rotations AND the same parent hierarchy

# BUT: Three.js GLTFLoader loads the GLB and the mHipLeft bone's local transform
# is defined by the GLB's node transform

# The mHipLeft bone in GLB has: rotation=[0,0,0,1] (identity) and translation=[0.089, -0.046, -0.004]
# The parent is mPelvis with: rotation=[0,0,0,1] and translation=[0, 1.149, 0.006]
# So mHipLeft bone has IDENTITY LOCAL ROTATION in GLB

# In Three.js AnimationMixer: when quaternion track is applied to bone,
# it sets bone.quaternion directly (overrides the bone's local rotation)
# So bone.quaternion = BVH_quaternion

# This means: the BVH quaternion is interpreted as the bone's LOCAL rotation
# For the BVH lThigh: the *local* rotation is computed assuming lThigh's parent (hip) 
# has already applied its rotation. So lThigh's local rotation IS what Three.js needs.

# THE PROBLEM: In BVH, lThigh's local frame has vx=(1,0,0), vy=(0,1,0), vz=(0,0,1) in WORLD space
# In GLB, mHipLeft's local frame is also vx=(1,0,0), vy=(0,1,0), vz=(0,0,1) in WORLD space
# (since all parent rotations are identity)
# THEREFORE: the BVH quaternion SHOULD work directly with GLB bones!

print("ANALYSIS OF DIRECT QUATERNION APPLICATION:")
print("  BVH lThigh local frame: (1,0,0), (0,1,0), (0,0,1) (world aligned, identity parent)")
print("  GLB mHipLeft local frame: identity rotation, world aligned")
print("  => Direct quaternion copy SHOULD be mathematically correct!")
print()
print("If crab walk occurs in Three.js preview, the cause is NOT the quaternion values")
print("but rather the SKELETON SCALE or BONE LENGTH mismatch affecting visual appearance")
print()
print("Alternative: The issue may be in SL's custom animation upload, not the preview")
