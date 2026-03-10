import struct, json

with open('/tmp/avatar_new.glb', 'rb') as f:
    f.read(12)
    json_len = struct.unpack('<I', f.read(4))[0]
    f.read(4)
    j = json.loads(f.read(json_len))

nodes = j['nodes']
meshes = j.get('meshes', [])
skins = j.get('skins', [])

print(f"Meshes: {len(meshes)}, Skins: {len(skins)}")

# Check mesh primitives for skin usage
for mi, mesh in enumerate(meshes):
    for pi, prim in enumerate(mesh.get('primitives', [])):
        skin_idx = prim.get('skin')
        print(f"  Mesh {mi} ({mesh.get('name','?')}) prim {pi}: skin={skin_idx}, attrs={list(prim.get('attributes', {}).keys())}")

print()
# Check nodes that reference meshes
print("Nodes with mesh references:")
for ni, node in enumerate(nodes):
    if 'mesh' in node or 'skin' in node:
        print(f"  Node {ni} ({node.get('name','?')}): mesh={node.get('mesh')}, skin={node.get('skin')}")
