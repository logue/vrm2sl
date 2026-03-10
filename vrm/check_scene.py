import struct, json

with open('/tmp/avatar_new.glb', 'rb') as f:
    f.read(12)
    json_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    j = json.loads(f.read(json_len))

# シーン構造を確認
scene = j['scenes'][j.get('scene', 0)]
print('Scene nodes:', scene['nodes'])
for ni in scene['nodes']:
    n = j['nodes'][ni]
    t = n.get('translation', [0,0,0])
    r = n.get('rotation', [0,0,0,1])
    print(f'  Node {ni} ({n.get("name","?")}): t={[round(x,4) for x in t]}, r={[round(x,4) for x in r]}')
    for ci in n.get('children', [])[:8]:
        cn = j['nodes'][ci]
        ct = cn.get('translation', [0,0,0])
        cr = cn.get('rotation', [0,0,0,1])
        print(f'    Child {ci} ({cn.get("name","?")}): t={[round(x,4) for x in ct]}, r={[round(x,4) for x in cr]}')

print()
for si, skin in enumerate(j['skins']):
    sk = skin.get('skeleton')
    print(f'Skin {si}: skeleton={sk}, joints[0]={skin["joints"][0]}')
    if sk is not None:
        sn = j['nodes'][sk]
        st = sn.get('translation', [0,0,0])
        sr = sn.get('rotation', [0,0,0,1])
        print(f'  skeleton node ({sn.get("name","?")}): t={[round(x,4) for x in st]}, r={[round(x,4) for x in sr]}')
