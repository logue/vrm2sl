"""
GLBのRootノードにX軸回転を適用したバリアントを作成する。
SLがY-upかZ-upどちらを期待しているかをテストするため。

variant A: Root rotation = -90度X (0.7071, 0, 0, 0.7071) wait no:
  -90 degrees around X: quaternion = (sin(-45°), 0, 0, cos(-45°)) = (-0.7071, 0, 0, 0.7071)
  This rotates Y-up model so that Z becomes up in the new space.
  
variant B: Root rotation = +90度X (0.7071, 0, 0, 0.7071) = sin(45°) = 0.7071
  This rotates Z-up model to Y-up. For a Y-up model, this would tilt it forward.

Actually, for SL which is Z-up:
  If SL interprets ingested glTF naively (no coordinate conversion), then a Y-up avatar
  would appear lying on its back in SL's Z-up world (since SL Z=up, glTF Y=up → body points sideways).
  
  To make a Y-up glTF appear upright in a Z-up world, rotate +90 around X:
  quat = (sin(45°), 0, 0, cos(45°)) = (0.70710678, 0, 0, 0.70710678)
  This maps:
    glTF Y-axis (0,1,0) → world Z-axis (0,0,1)  (so body stays upright)
    glTF Z-axis (0,0,1) → world -Y-axis (0,-1,0) (so front of avatar points in -Y direction)

But SL BVH files are Y-up (hip at Y~40). If SL applies Y-up to Z-up transform to BVH too, both
would be consistently transformed. The question is whether SL does this for glTF uploads too.
"""
import struct, json, math, copy

def create_variant(input_path, output_path, root_rotation):
    """Create GLB variant with given root rotation applied."""
    with open(input_path, 'rb') as f:
        magic = f.read(4)
        version = struct.unpack('<I', f.read(4))[0]
        length = struct.unpack('<I', f.read(4))[0]
        
        json_len = struct.unpack('<I', f.read(4))[0]
        json_type = f.read(4)
        json_data = json.loads(f.read(json_len))
        
        bin_len = struct.unpack('<I', f.read(4))[0]
        bin_type = f.read(4)
        bin_data = f.read(bin_len)
    
    # Find root node (node 0, named "Root")
    nodes = json_data['nodes']
    scene_nodes = json_data['scenes'][0]['nodes']
    
    print(f"Scene nodes: {scene_nodes}")
    for ni in scene_nodes:
        n = nodes[ni]
        print(f"  Node {ni} ({n.get('name','?')}): current r={n.get('rotation',[0,0,0,1])}")
    
    # Apply rotation to Root node (first scene node)
    root_node_idx = scene_nodes[0]
    root_node = nodes[root_node_idx]
    print(f"\nApplying rotation {root_rotation} to Root node {root_node_idx} ({root_node.get('name','?')})")
    root_node['rotation'] = list(root_rotation)
    
    # Serialize new JSON
    new_json_str = json.dumps(json_data, separators=(',', ':'))
    new_json_bytes = new_json_str.encode('utf-8')
    # Pad to 4-byte boundary
    while len(new_json_bytes) % 4 != 0:
        new_json_bytes += b' '
    new_json_len = len(new_json_bytes)
    
    # Calculate new total length
    # header(12) + json_chunk_header(8) + json_data + bin_chunk_header(8) + bin_data
    new_total = 12 + 8 + new_json_len + 8 + bin_len
    
    with open(output_path, 'wb') as f:
        # GLB header
        f.write(b'glTF')
        f.write(struct.pack('<I', 2))  # version 2
        f.write(struct.pack('<I', new_total))  # total length
        
        # JSON chunk
        f.write(struct.pack('<I', new_json_len))
        f.write(b'JSON')
        f.write(new_json_bytes)
        
        # BIN chunk
        f.write(struct.pack('<I', bin_len))
        f.write(b'BIN\x00')
        f.write(bin_data)
    
    print(f"Written to {output_path} ({new_total} bytes)")

# Variant A: +90 degrees around X axis (for Y-up to Z-up)
# This is what a Z-up world would need to display a Y-up glTF upright
rot_90x = (0.7071067811865476, 0.0, 0.0, 0.7071067811865476)  # (x, y, z, w)
create_variant('/tmp/avatar_new.glb', '/tmp/avatar_rot_plus90x.glb', rot_90x)

print()

# Variant B: -90 degrees around X axis
rot_neg90x = (-0.7071067811865476, 0.0, 0.0, 0.7071067811865476)  # (x, y, z, w)
create_variant('/tmp/avatar_new.glb', '/tmp/avatar_rot_minus90x.glb', rot_neg90x)

print()
print("Files created:")
print("  /tmp/avatar_rot_plus90x.glb  - Root rotated +90° around X")
print("  /tmp/avatar_rot_minus90x.glb - Root rotated -90° around X")
print()
print("Test in SL by uploading these files. If avatar stands upright in SL,")
print("that variant indicates SL's expected coordinate system.")
