#!/usr/bin/env python3
import json
import struct
import sys

target_path = sys.argv[1] if len(sys.argv) > 1 else 'vrm/output.glb'

with open(target_path, 'rb') as f:
    f.read(12)
    json_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    js = json.loads(f.read(json_len))
    bin_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    bd = f.read(bin_len)

skin = js['skins'][0]
joints = [int(v) for v in skin.get('joints', [])]
nodes = js['nodes']

name_to_slot = {nodes[j].get('name', ''): i for i, j in enumerate(joints)}
slot_l = name_to_slot.get('mEyeLeft', -1)
slot_r = name_to_slot.get('mEyeRight', -1)
print('file:', target_path)
print('slots:', name_to_slot)

prim = js['meshes'][0]['primitives'][0]
a = prim['attributes']
acc_pos = js['accessors'][a['POSITION']]
acc_j = js['accessors'][a['JOINTS_0']]
acc_w = js['accessors'][a['WEIGHTS_0']]

bv_pos = js['bufferViews'][acc_pos['bufferView']]
bv_j = js['bufferViews'][acc_j['bufferView']]
bv_w = js['bufferViews'][acc_w['bufferView']]

base_pos = bv_pos.get('byteOffset', 0) + acc_pos.get('byteOffset', 0)
base_j = bv_j.get('byteOffset', 0) + acc_j.get('byteOffset', 0)
base_w = bv_w.get('byteOffset', 0) + acc_w.get('byteOffset', 0)

stride_pos = bv_pos.get('byteStride', 12)
stride_j = bv_j.get('byteStride', 4)
stride_w = bv_w.get('byteStride', 16)
count = min(acc_pos['count'], acc_j['count'], acc_w['count'])
comp = acc_j['componentType']

sum_l = [0.0, 0.0, 0.0]
sum_r = [0.0, 0.0, 0.0]
wl = 0.0
wr = 0.0

for vi in range(count):
    op = base_pos + vi * stride_pos
    x, y, z = struct.unpack('<fff', bd[op:op + 12])

    oj = base_j + vi * stride_j
    ow = base_w + vi * stride_w

    left_w = 0.0
    right_w = 0.0
    for lane in range(4):
        if comp == 5121:
            slot = bd[oj + lane]
        else:
            slot = struct.unpack('<H', bd[oj + lane * 2: oj + lane * 2 + 2])[0]
        w = struct.unpack('<f', bd[ow + lane * 4: ow + lane * 4 + 4])[0]
        if slot == slot_l:
            left_w += w
        if slot == slot_r:
            right_w += w

    if left_w > 1e-6:
        sum_l[0] += x * left_w
        sum_l[1] += y * left_w
        sum_l[2] += z * left_w
        wl += left_w
    if right_w > 1e-6:
        sum_r[0] += x * right_w
        sum_r[1] += y * right_w
        sum_r[2] += z * right_w
        wr += right_w

left_center = [c / wl for c in sum_l] if wl > 1e-6 else [0, 0, 0]
right_center = [c / wr for c in sum_r] if wr > 1e-6 else [0, 0, 0]

print('left weighted center:', [round(v, 5) for v in left_center], 'total w', round(wl, 3))
print('right weighted center:', [round(v, 5) for v in right_center], 'total w', round(wr, 3))

for name in ['mEyeLeft', 'mEyeRight', 'mHead']:
    idx = next((i for i, n in enumerate(nodes) if n.get('name') == name), None)
    if idx is None:
        continue
    t = nodes[idx].get('translation', [0, 0, 0])
    # translation is local; print for reference.
    print(name, 'local t', [round(float(v), 5) for v in t])
