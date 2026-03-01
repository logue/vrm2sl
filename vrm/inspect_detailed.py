#!/usr/bin/env python3
"""Detailed inspection of the converted GLB: IBMs, bone world matrices, vertex data."""
import json
import math
import struct
import sys


def read_glb(path):
    with open(path, "rb") as f:
        magic = f.read(4)
        version, total_len = struct.unpack("<II", f.read(8))
        # JSON chunk
        json_len = struct.unpack("<I", f.read(4))[0]
        json_type = struct.unpack("<I", f.read(4))[0]
        json_bytes = f.read(json_len)
        # BIN chunk
        bin_data = b""
        if f.tell() < total_len:
            bin_len = struct.unpack("<I", f.read(4))[0]
            bin_type = struct.unpack("<I", f.read(4))[0]
            bin_data = f.read(bin_len)
    return json.loads(json_bytes), bin_data


def read_accessor_meta(js, acc_idx):
    acc = js["accessors"][acc_idx]
    bv = js["bufferViews"][acc["bufferView"]]
    comp_type = acc["componentType"]
    acc_type = acc["type"]
    elem_count = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[acc_type]
    comp_size = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}[comp_type]
    view_off = bv.get("byteOffset", 0)
    acc_off = acc.get("byteOffset", 0)
    default_stride = elem_count * comp_size
    stride = bv.get("byteStride", default_stride)
    return {
        "base": view_off + acc_off,
        "stride": stride,
        "count": acc["count"],
        "comp_type": comp_type,
        "acc_type": acc_type,
    }


def read_floats(bin_data, offset, count):
    vals = []
    for i in range(count):
        b = bin_data[offset + i * 4 : offset + i * 4 + 4]
        vals.append(struct.unpack("<f", b)[0])
    return vals


def read_mat4(bin_data, meta, index):
    """Read a MAT4 (column-major) from binary."""
    off = meta["base"] + index * meta["stride"]
    vals = read_floats(bin_data, off, 16)
    # column-major → 4x4 matrix
    m = [[vals[col * 4 + row] for col in range(4)] for row in range(4)]
    return m


def read_vec3(bin_data, meta, index):
    off = meta["base"] + index * meta["stride"]
    return read_floats(bin_data, off, 3)


def mat4_translation(m):
    return [m[0][3], m[1][3], m[2][3]]


def mat4_rotation_magnitude(m):
    """How far is the upper-left 3x3 from identity? Returns angle in degrees."""
    # Frobenius distance from identity for the 3x3 rotation part
    d = 0
    for r in range(3):
        for c in range(3):
            expected = 1.0 if r == c else 0.0
            d += (m[r][c] - expected) ** 2
    return math.sqrt(d)


def mat4_multiply(a, b):
    c = [[0]*4 for _ in range(4)]
    for i in range(4):
        for j in range(4):
            for k in range(4):
                c[i][j] += a[i][k] * b[k][j]
    return c


def mat4_inverse(m):
    """Compute inverse of a 4x4 matrix (basic implementation)."""
    import copy
    n = 4
    aug = [m[i][:] + [1 if i == j else 0 for j in range(n)] for i in range(n)]
    for i in range(n):
        max_row = i
        for k in range(i + 1, n):
            if abs(aug[k][i]) > abs(aug[max_row][i]):
                max_row = k
        aug[i], aug[max_row] = aug[max_row], aug[i]
        pivot = aug[i][i]
        if abs(pivot) < 1e-12:
            return None
        for j in range(2 * n):
            aug[i][j] /= pivot
        for k in range(n):
            if k != i:
                factor = aug[k][i]
                for j in range(2 * n):
                    aug[k][j] -= factor * aug[i][j]
    return [aug[i][n:] for i in range(n)]


def node_to_local_matrix(node):
    if "matrix" in node:
        vals = node["matrix"]
        # column-major in glTF
        return [[vals[col * 4 + row] for col in range(4)] for row in range(4)]
    
    t = node.get("translation", [0, 0, 0])
    r = node.get("rotation", [0, 0, 0, 1])  # [x, y, z, w]
    s = node.get("scale", [1, 1, 1])
    
    # Quaternion to rotation matrix
    x, y, z, w = r
    m = [[0]*4 for _ in range(4)]
    m[0][0] = (1 - 2*(y*y + z*z)) * s[0]
    m[0][1] = (2*(x*y - z*w)) * s[1]
    m[0][2] = (2*(x*z + y*w)) * s[2]
    m[1][0] = (2*(x*y + z*w)) * s[0]
    m[1][1] = (1 - 2*(x*x + z*z)) * s[1]
    m[1][2] = (2*(y*z - x*w)) * s[2]
    m[2][0] = (2*(x*z - y*w)) * s[0]
    m[2][1] = (2*(y*z + x*w)) * s[1]
    m[2][2] = (1 - 2*(x*x + y*y)) * s[2]
    m[0][3] = t[0]
    m[1][3] = t[1]
    m[2][3] = t[2]
    m[3][3] = 1.0
    return m


def compute_world_matrices(nodes):
    """Compute world matrices for all nodes."""
    parent_map = {}
    for i, node in enumerate(nodes):
        for c in node.get("children", []):
            parent_map[c] = i
    
    locals_ = [node_to_local_matrix(n) for n in nodes]
    worlds = [None] * len(nodes)
    
    def resolve(idx):
        if worlds[idx] is not None:
            return worlds[idx]
        if idx in parent_map:
            pw = resolve(parent_map[idx])
            worlds[idx] = mat4_multiply(pw, locals_[idx])
        else:
            worlds[idx] = locals_[idx]
        return worlds[idx]
    
    for i in range(len(nodes)):
        resolve(i)
    return worlds


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "vrm/output.glb"
    js, bin_data = read_glb(path)
    nodes = js["nodes"]
    
    print(f"=== Inspecting: {path} ===")
    print(f"Nodes: {len(nodes)}, Bin: {len(bin_data)} bytes")
    
    # Compute world matrices from node transforms
    worlds = compute_world_matrices(nodes)
    
    # Check key bones
    key_bones = ["mPelvis", "mTorso", "mChest", "mNeck", "mHead",
                 "mCollarLeft", "mShoulderLeft", "mElbowLeft", "mWristLeft",
                 "mCollarRight", "mShoulderRight", "mElbowRight", "mWristRight",
                 "mHipLeft", "mKneeLeft", "mAnkleLeft",
                 "mHipRight", "mKneeRight", "mAnkleRight"]
    
    name_to_idx = {}
    for i, n in enumerate(nodes):
        name = n.get("name", "")
        if name:
            name_to_idx[name] = i
    
    print("\n=== Key Bone Transforms (Node TRS) ===")
    for bname in key_bones:
        idx = name_to_idx.get(bname)
        if idx is None:
            continue
        n = nodes[idx]
        t = n.get("translation", [0,0,0])
        r = n.get("rotation", [0,0,0,1])
        s = n.get("scale", [1,1,1])
        
        # Check if rotation is identity
        rot_is_identity = all(abs(r[i]) < 1e-6 for i in range(3)) and abs(r[3] - 1.0) < 1e-6
        
        w = worlds[idx]
        wt = mat4_translation(w)
        rot_mag = mat4_rotation_magnitude(w)
        
        print(f"  {bname:20s} (node {idx:3d}): "
              f"local_t=[{t[0]:8.4f},{t[1]:8.4f},{t[2]:8.4f}] "
              f"rot_id={rot_is_identity} "
              f"world_t=[{wt[0]:8.4f},{wt[1]:8.4f},{wt[2]:8.4f}] "
              f"world_rot_dist={rot_mag:.6f}")
    
    # Check IBMs for skin 1 (Body - the main skin with 47 joints)
    print("\n=== IBM Analysis (Skin 1 - Body) ===")
    skin = js["skins"][1]
    joints = skin["joints"]
    ibm_acc_idx = skin["inverseBindMatrices"]
    ibm_meta = read_accessor_meta(js, ibm_acc_idx)
    
    print(f"  IBM accessor: count={ibm_meta['count']}, stride={ibm_meta['stride']}")
    
    for ji, joint_idx in enumerate(joints[:20]):  # First 20 joints
        bname = nodes[joint_idx].get("name", "?")
        ibm = read_mat4(bin_data, ibm_meta, ji)
        
        # IBM should be inverse(world). Check by multiplying world * IBM -> should be ~identity
        w = worlds[joint_idx]
        product = mat4_multiply(w, ibm)
        
        # How far is product from identity?
        identity_dist = 0
        for r in range(4):
            for c in range(4):
                expected = 1.0 if r == c else 0.0
                identity_dist += (product[r][c] - expected) ** 2
        identity_dist = math.sqrt(identity_dist)
        
        ibm_t = mat4_translation(ibm)
        ibm_rot_mag = mat4_rotation_magnitude(ibm)
        
        print(f"  Joint {ji:2d} ({bname:20s}): "
              f"IBM_t=[{ibm_t[0]:8.4f},{ibm_t[1]:8.4f},{ibm_t[2]:8.4f}] "
              f"IBM_rot_dist={ibm_rot_mag:.6f} "
              f"world*IBM_dist={identity_dist:.6f}")
    
    # Check vertex positions for skin 1
    print("\n=== Vertex Position Sample (Skin 1 - Body, mesh 1) ===")
    mesh = js["meshes"][1]
    for pi, prim in enumerate(mesh["primitives"][:1]):
        attrs = prim["attributes"]
        if "POSITION" not in attrs:
            continue
        pos_meta = read_accessor_meta(js, attrs["POSITION"])
        
        # Check joints and weights
        jnt_meta = read_accessor_meta(js, attrs["JOINTS_0"]) if "JOINTS_0" in attrs else None
        wgt_meta = read_accessor_meta(js, attrs["WEIGHTS_0"]) if "WEIGHTS_0" in attrs else None
        
        print(f"  Primitive {pi}: {pos_meta['count']} vertices")
        
        # Sample a few vertices that are likely head/neck area (high Y)
        # First, find the range
        max_y = -999
        min_y = 999
        max_y_idx = 0
        for vi in range(min(pos_meta['count'], 50000)):
            pos = read_vec3(bin_data, pos_meta, vi)
            if pos[1] > max_y:
                max_y = pos[1]
                max_y_idx = vi
            if pos[1] < min_y:
                min_y = pos[1]
        
        print(f"  Y range: [{min_y:.4f}, {max_y:.4f}]")
        
        # Show some vertices with their joint assignments
        sample_indices = [0, 1, 2, max_y_idx, pos_meta['count'] // 2]
        for vi in sample_indices:
            if vi >= pos_meta['count']:
                continue
            pos = read_vec3(bin_data, pos_meta, vi)
            
            jnt_info = ""
            if jnt_meta and wgt_meta:
                jnts = []
                wgts = []
                for lane in range(4):
                    off = jnt_meta["base"] + vi * jnt_meta["stride"]
                    if jnt_meta["comp_type"] == 5121:  # UNSIGNED_BYTE
                        j = bin_data[off + lane]
                    else:  # UNSIGNED_SHORT
                        j = struct.unpack("<H", bin_data[off + lane*2:off + lane*2 + 2])[0]
                    
                    w_off = wgt_meta["base"] + vi * wgt_meta["stride"] + lane * 4
                    w = struct.unpack("<f", bin_data[w_off:w_off+4])[0]
                    jnts.append(j)
                    wgts.append(w)
                
                joint_names = []
                for j, w in zip(jnts, wgts):
                    if w > 0.001 and j < len(joints):
                        jname = nodes[joints[j]].get("name", "?")
                        joint_names.append(f"{jname}:{w:.3f}")
                jnt_info = " | " + ", ".join(joint_names)
            
            print(f"    v{vi}: pos=[{pos[0]:8.4f},{pos[1]:8.4f},{pos[2]:8.4f}]{jnt_info}")
    
    # Also check: for the original VRM, what rotations do the bones have?
    print("\n=== Verifying: All SL bones have identity rotation ===")
    non_identity = []
    for bname in key_bones:
        idx = name_to_idx.get(bname)
        if idx is None:
            continue
        r = nodes[idx].get("rotation", [0,0,0,1])
        if not (all(abs(r[i]) < 1e-6 for i in range(3)) and abs(r[3] - 1.0) < 1e-6):
            non_identity.append((bname, r))
    if non_identity:
        print(f"  WARNING: {len(non_identity)} bones have non-identity rotation:")
        for bname, r in non_identity:
            print(f"    {bname}: r={r}")
    else:
        print("  OK: All key SL bones have identity rotation.")
    
    # Check: do IBMs have pure-translation form (3x3 = identity)?
    print("\n=== Checking IBM rotation component (should be identity for SL) ===")
    non_pure_ibm = []
    for ji, joint_idx in enumerate(joints):
        bname = nodes[joint_idx].get("name", "?")
        ibm = read_mat4(bin_data, ibm_meta, ji)
        rot_mag = mat4_rotation_magnitude(ibm)
        if rot_mag > 0.01:
            non_pure_ibm.append((bname, ji, rot_mag))
    if non_pure_ibm:
        print(f"  WARNING: {len(non_pure_ibm)} IBMs have significant rotation component:")
        for bname, ji, mag in non_pure_ibm[:10]:
            ibm = read_mat4(bin_data, ibm_meta, ji)
            print(f"    {bname} (joint {ji}): rot_dist={mag:.6f}")
            for row in range(3):
                print(f"      [{ibm[row][0]:8.5f} {ibm[row][1]:8.5f} {ibm[row][2]:8.5f}]")
    else:
        print("  OK: All IBMs have pure translation (3x3 = identity).")


if __name__ == "__main__":
    main()
