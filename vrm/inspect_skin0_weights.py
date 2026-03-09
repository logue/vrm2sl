#!/usr/bin/env python3
import json
import struct

with open('vrm/output.glb', 'rb') as f:
    f.read(12)
    json_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    js = json.loads(f.read(json_len))
    bin_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    bd = f.read(bin_len)

skin = js['skins'][0]
joints = [int(v) for v in skin.get('joints', [])]
joint_names = [js['nodes'][i].get('name', '') for i in joints]
print('skin0 joints:', joint_names)

prim = js['meshes'][0]['primitives'][0]
attrs = prim['attributes']
aj = js['accessors'][attrs['JOINTS_0']]
aw = js['accessors'][attrs['WEIGHTS_0']]

bvj = js['bufferViews'][aj['bufferView']]
bvw = js['bufferViews'][aw['bufferView']]

base_j = bvj.get('byteOffset', 0) + aj.get('byteOffset', 0)
base_w = bvw.get('byteOffset', 0) + aw.get('byteOffset', 0)
stride_j = bvj.get('byteStride', 4)
stride_w = bvw.get('byteStride', 16)
count = min(aj['count'], aw['count'])
comp = aj['componentType']

acc = [0.0 for _ in range(len(joints))]
non_head_vertices = 0

for vi in range(count):
    oj = base_j + vi * stride_j
    ow = base_w + vi * stride_w
    has_non_head = False
    for lane in range(4):
        if comp == 5121:
            slot = bd[oj + lane]
        else:
            slot = struct.unpack('<H', bd[oj + lane * 2: oj + lane * 2 + 2])[0]
        weight = struct.unpack('<f', bd[ow + lane * 4: ow + lane * 4 + 4])[0]
        if slot < len(acc):
            acc[slot] += weight
            if slot != 0 and weight > 1e-6:
                has_non_head = True
    if has_non_head:
        non_head_vertices += 1

print('accumulated weights:', [(joint_names[i], round(w, 3)) for i, w in enumerate(acc)])
print('vertices with non-head influence:', non_head_vertices, 'of', count)
