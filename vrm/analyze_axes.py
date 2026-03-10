#!/usr/bin/env python3
import math

# BVH SL default skeleton: hip -> lThigh direction
bvh_hip_to_lthigh = (4.500466, -6.400484, -1.832696)
bvh_ht_len = math.sqrt(sum(x**2 for x in bvh_hip_to_lthigh))
bvh_ht_dir = tuple(x/bvh_ht_len for x in bvh_hip_to_lthigh)
print("BVH hip->lThigh:", bvh_hip_to_lthigh, "len=%.4fcm" % bvh_ht_len)
print("BVH hip->lThigh dir:", tuple("%.6f"%v for v in bvh_ht_dir))

# GLB mPelvis -> mHipLeft (local translation)
glb_ps_to_hl = (0.0892, -0.0462, -0.0043)
glb_hl_len = math.sqrt(sum(x**2 for x in glb_ps_to_hl))
glb_hl_dir = tuple(x/glb_hl_len for x in glb_ps_to_hl)
print("GLB mPelvis->mHipLeft:", glb_ps_to_hl, "len=%.4fm=%.4fcm" % (glb_hl_len, glb_hl_len*100))
print("GLB mPelvis->mHipLeft dir:", tuple("%.6f"%v for v in glb_hl_dir))

print()
print("BVH Z sign: %s" % ("positive" if bvh_hip_to_lthigh[2] > 0 else "negative"))
print("GLB Z sign: %s" % ("positive" if glb_ps_to_hl[2] > 0 else "negative"))

print()
# BVH lThigh -> lShin (child bone direction)
bvh_shin_offset = (-1.359117, -18.918689, 1.179887)
bvh_sl = math.sqrt(sum(x**2 for x in bvh_shin_offset))
bvh_sd = tuple(x/bvh_sl for x in bvh_shin_offset)
print("BVH lThigh->lShin:", bvh_shin_offset, "len=%.4fcm" % bvh_sl)
print("BVH lThigh->lShin dir:", tuple("%.6f"%v for v in bvh_sd))
print("BVH lShin Z: %s (%.4f cm)" % ("positive=toward +Z" if bvh_shin_offset[2] > 0 else "negative=toward -Z", bvh_shin_offset[2]))

# GLB mHipLeft -> mKneeLeft
glb_knee = (0, -0.4682, -0.0098)
glb_kl = math.sqrt(sum(x**2 for x in glb_knee))
glb_kd = tuple(x/glb_kl for x in glb_knee)
print()
print("GLB mHipLeft->mKneeLeft:", glb_knee, "len=%.4fm=%.4fcm" % (glb_kl, glb_kl*100))
print("GLB mHipLeft->mKneeLeft dir:", tuple("%.6f"%v for v in glb_kd))
print("GLB mKneeLeft Z: %s (%.4f cm)" % ("positive=toward +Z" if glb_knee[2] > 0 else "negative=toward -Z", glb_knee[2]*100))

print()
print("=== CRITICAL COMPARISON ===")
print("BVH child bone Z component (lShin): +%.4f cm (POSITIVE = toward +Z)" % bvh_shin_offset[2])
print("GLB child bone Z component (mKnee): -%.4f cm (NEGATIVE = toward -Z)" % abs(glb_knee[2]*100))
print()
print("=> In BVH SL skeleton: the shin points BACKWARD in Z (+Z)")
print("=> In GLB after VRM conversion: the knee points FORWARD in -Z")
print("=> This Z-flip in the bone rest orientation could NOT cause crab walk by itself")
print("   (since the thigh also has corresponding Z offset)")
print()

# Now the key question: in SL, does the BVH rotation use the JOINT rest pose or a fixed frame?
# SL uses its own default skeleton rest pose, NOT the glTF joint rest pose
# So when SL applies BVH lThigh X=31.8 deg, it rotates around the X axis of SL default rest
# But if we have uploaded a custom skeleton via glTF joint position overrides,
# SL should use OUR joint positions, but the rotation FRAME is still the BONE's local frame
# which is defined by the joint's DIRECTION in the rest pose

# The real question for SL upload: does SL use the glTF bone direction or SL default direction?
# If SL uses DEFAULT direction: BVH authored for SL default will work correctly
# If SL uses GLB direction: BVH needs retargeting

# Let's see if the crab walk could be an X/Y swap:
# In Three.js preview - the BVH quaternion is applied directly  
# Three.js BVH space: possibly different from glTF space?
