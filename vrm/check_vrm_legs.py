#!/usr/bin/env python3
"""Check VRM source bone hips rotation and leg bones."""
import json
import struct
import sys

vrm_path = "/Users/logue/Developer/vrm2sl/vrm/AvatarSample_A.vrm"
with open(vrm_path, 'rb') as f:
    magic = f.read(4)
    version = struct.unpack('<I', f.read(4))[0]
    length = struct.unpack('<I', f.read(4))[0]
    chunk0_len = struct.unpack('<I', f.read(4))[0]
    chunk0_type = f.read(4)
    json_data = json.loads(f.read(chunk0_len))

nodes = json_data.get('nodes', [])
print("VRM Hips and leg bones:")
for i, n in enumerate(nodes):
    name = n.get('name', '')
    if any(k in name for k in ['Hips','UpperLeg','LowerLeg','Foot','Bip_L_Up','Bip_L_Lo','Bip_R_Up','Bip_R_Lo']):
        t = n.get('translation', 'none')
        r = n.get('rotation', 'none')
        print("  Node %d (%s): t=%s r=%s" % (i, name, t, r))

print()
# Find VRM humanoid bone mapping
exts = json_data.get('extensions', {})
vrmc = exts.get('VRMC_vrm', {})
humanoid = vrmc.get('humanBones', {})
print("VRM humanoid bone mapping (key bones):")
for bone_name in ['hips', 'leftUpperLeg', 'leftLowerLeg', 'leftFoot', 'rightUpperLeg', 'rightLowerLeg', 'rightFoot']:
    if bone_name in humanoid:
        node_idx = humanoid[bone_name].get('node', 'N/A')
        if isinstance(node_idx, int) and node_idx < len(nodes):
            node = nodes[node_idx]
            print("  %s -> Node %d (%s): t=%s r=%s" % (
                bone_name, node_idx, node.get('name','?'),
                node.get('translation','none'), node.get('rotation','none')))
