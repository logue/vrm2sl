import json
import struct

with open('/tmp/fixed_output.glb', 'rb') as f:
    data = f.read()

jlen = struct.unpack_from('<I', data, 12)[0]
gltf = json.loads(data[20:20+jlen])
nodes = gltf['nodes']

# Face/Body/Hair mesh nodes
print("=== Mesh nodes ===")
for i, n in enumerate(nodes):
    name = n.get('name', '?')
    if name in ('Face', 'Body', 'Hair'):
        print(f"Node {i} ({name}): t={n.get('translation')}, r={n.get('rotation')}, s={n.get('scale')}")
        print(f"  mesh={n.get('mesh')}, skin={n.get('skin')}")

print()

# Check face skin vertex positions (first few vertices)
accs = gltf['accessors']
bvs = gltf['bufferViews']
meshes = gltf['meshes']

bin_start = 20 + ((jlen + 3) & ~3)
blen = struct.unpack_from('<I', data, bin_start)[0]
bin_data = data[bin_start+8:bin_start+8+blen]

def read_accessor_vec3(acc_idx, count=3):
    acc = accs[acc_idx]
    bv = bvs[acc['bufferView']]
    base = bv.get('byteOffset', 0) + acc.get('byteOffset', 0)
    stride = bv.get('byteStride', 12)
    results = []
    for i in range(min(count, acc.get('count', 0))):
        off = base + i * stride
        v = struct.unpack_from('<3f', bin_data, off)
        results.append(v)
    return results

print("=== Face mesh vertex positions (first 3) ===")
for node in nodes:
    if node.get('name') == 'Face' and node.get('mesh') is not None:
        mesh = meshes[node['mesh']]
        for pi, prim in enumerate(mesh.get('primitives', [])):
            pos_acc = prim.get('attributes', {}).get('POSITION')
            if pos_acc is not None:
                verts = read_accessor_vec3(pos_acc, 3)
                print(f"  prim {pi}: first 3 vertices = {verts}")
        break

print()
print("=== Body mesh vertex positions near head (check Y range) ===")
for node in nodes:
    if node.get('name') == 'Body' and node.get('mesh') is not None:
        mesh = meshes[node['mesh']]
        for pi, prim in enumerate(mesh.get('primitives', [])):
            pos_acc = prim.get('attributes', {}).get('POSITION')
            if pos_acc is not None:
                acc = accs[pos_acc]
                bv = bvs[acc['bufferView']]
                base = bv.get('byteOffset', 0) + acc.get('byteOffset', 0)
                stride = bv.get('byteStride', 12)
                count = acc.get('count', 0)
                # Find max Y vertex (head area)
                max_y = -1e10
                max_pos = None
                for i in range(count):
                    off = base + i * stride
                    v = struct.unpack_from('<3f', bin_data, off)
                    if v[1] > max_y:
                        max_y = v[1]
                        max_pos = v
                print(f"  prim {pi}: max-Y vertex (head top) = {max_pos}, Y={max_y:.4f}")
        break
