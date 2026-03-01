#!/usr/bin/env python3
"""Compare original VRM IBMs with output GLB IBMs."""
import json
import math
import struct
import sys


def read_glb(path):
    with open(path, "rb") as f:
        magic = f.read(4)
        version, total_len = struct.unpack("<II", f.read(8))
        json_len = struct.unpack("<I", f.read(4))[0]
        json_type = struct.unpack("<I", f.read(4))[0]
        json_bytes = f.read(json_len)
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


def read_mat4(bin_data, meta, index):
    off = meta["base"] + index * meta["stride"]
    vals = []
    for i in range(16):
        b = bin_data[off + i * 4: off + i * 4 + 4]
        vals.append(struct.unpack("<f", b)[0])
    # column-major to row-major
    m = [[vals[col * 4 + row] for col in range(4)] for row in range(4)]
    return m


def read_vec3(bin_data, meta, index):
    off = meta["base"] + index * meta["stride"]
    vals = []
    for i in range(3):
        b = bin_data[off + i * 4: off + i * 4 + 4]
        vals.append(struct.unpack("<f", b)[0])
    return vals


def mat4_translation(m):
    return [m[0][3], m[1][3], m[2][3]]


def mat4_rotation_magnitude(m):
    d = 0
    for r in range(3):
        for c in range(3):
            expected = 1.0 if r == c else 0.0
            d += (m[r][c] - expected) ** 2
    return math.sqrt(d)


def node_to_local_matrix(node):
    if "matrix" in node:
        vals = node["matrix"]
        return [[vals[col * 4 + row] for col in range(4)] for row in range(4)]
    
    t = node.get("translation", [0, 0, 0])
    r = node.get("rotation", [0, 0, 0, 1])
    s = node.get("scale", [1, 1, 1])
    
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


def mat4_multiply(a, b):
    c = [[0]*4 for _ in range(4)]
    for i in range(4):
        for j in range(4):
            for k in range(4):
                c[i][j] += a[i][k] * b[k][j]
    return c


def main():
    vrm_path = "vrm/AvatarSample_A.vrm"
    out_path = "vrm/output.glb"
    
    vrm_js, vrm_bin = read_glb(vrm_path)
    out_js, out_bin = read_glb(out_path)
    
    vrm_nodes = vrm_js["nodes"]
    out_nodes = out_js["nodes"]
    
    # VRM bone name mapping (from humanoid extension)
    vrm_humanoid = {}
    if "extensions" in vrm_js and "VRM" in vrm_js["extensions"]:
        for bone in vrm_js["extensions"]["VRM"]["humanoid"]["humanBones"]:
            vrm_humanoid[bone["bone"]] = bone["node"]
    
    # SL bone name mapping
    bone_map = {
        "hips": "mPelvis", "spine": "mTorso", "chest": "mChest",
        "neck": "mNeck", "head": "mHead",
        "leftShoulder": "mCollarLeft", "leftUpperArm": "mShoulderLeft",
        "leftLowerArm": "mElbowLeft", "leftHand": "mWristLeft",
        "rightShoulder": "mCollarRight", "rightUpperArm": "mShoulderRight",
        "rightLowerArm": "mElbowRight", "rightHand": "mWristRight",
        "leftUpperLeg": "mHipLeft", "leftLowerLeg": "mKneeLeft",
        "leftFoot": "mAnkleLeft",
        "rightUpperLeg": "mHipRight", "rightLowerLeg": "mKneeRight",
        "rightFoot": "mAnkleRight",
    }
    
    # Compute world matrices
    vrm_worlds = compute_world_matrices(vrm_nodes)
    out_worlds = compute_world_matrices(out_nodes)
    
    # Read VRM IBMs (skin 1 for body)
    vrm_skin1 = vrm_js["skins"][1] if len(vrm_js.get("skins", [])) > 1 else vrm_js["skins"][0]
    vrm_joints = vrm_skin1["joints"]
    vrm_ibm_meta = read_accessor_meta(vrm_js, vrm_skin1["inverseBindMatrices"])
    
    out_skin1 = out_js["skins"][1] if len(out_js.get("skins", [])) > 1 else out_js["skins"][0]
    out_joints = out_skin1["joints"]
    out_ibm_meta = read_accessor_meta(out_js, out_skin1["inverseBindMatrices"])
    
    # Map VRM joint indices to bone names
    vrm_idx_to_name = {}
    for vrm_name, node_idx in vrm_humanoid.items():
        vrm_idx_to_name[node_idx] = vrm_name
    
    out_idx_to_name = {}
    for i, node in enumerate(out_nodes):
        name = node.get("name", "")
        if name:
            out_idx_to_name[i] = name
    
    print("=== VRM Original IBM Analysis ===")
    print(f"VRM Skin joints: {len(vrm_joints)}, IBM count: {vrm_ibm_meta['count']}")
    
    # For each VRM bone that maps to an SL bone, compare
    print("\n=== Comparison: VRM original vs Output ===")
    print(f"{'Bone':<20} | {'VRM IBM rot_dist':>16} | {'VRM world_t':>30} | {'Out IBM rot_dist':>16} | {'Out world_t':>30}")
    print("-" * 130)
    
    scale = 1.2836  # from the conversion output
    
    for vrm_name, sl_name in sorted(bone_map.items()):
        vrm_node = vrm_humanoid.get(vrm_name)
        if vrm_node is None:
            continue
        
        # Find VRM joint index in skin
        vrm_ji = None
        for ji, jn in enumerate(vrm_joints):
            if jn == vrm_node:
                vrm_ji = ji
                break
        
        # Find output joint index
        out_node = None
        for i, n in enumerate(out_nodes):
            if n.get("name") == sl_name:
                out_node = i
                break
        out_ji = None
        if out_node is not None:
            for ji, jn in enumerate(out_joints):
                if jn == out_node:
                    out_ji = ji
                    break
        
        vrm_ibm_rot = "N/A"
        vrm_wt = "N/A"
        if vrm_ji is not None:
            vrm_ibm = read_mat4(vrm_bin, vrm_ibm_meta, vrm_ji)
            vrm_ibm_rot = f"{mat4_rotation_magnitude(vrm_ibm):.6f}"
            wt = mat4_translation(vrm_worlds[vrm_node])
            vrm_wt = f"[{wt[0]:7.4f},{wt[1]:7.4f},{wt[2]:7.4f}]"
        
        out_ibm_rot = "N/A"
        out_wt = "N/A"
        if out_ji is not None and out_node is not None:
            out_ibm = read_mat4(out_bin, out_ibm_meta, out_ji)
            out_ibm_rot = f"{mat4_rotation_magnitude(out_ibm):.6f}"
            wt = mat4_translation(out_worlds[out_node])
            out_wt = f"[{wt[0]:7.4f},{wt[1]:7.4f},{wt[2]:7.4f}]"
        
        print(f"{sl_name:<20} | {vrm_ibm_rot:>16} | {vrm_wt:>30} | {out_ibm_rot:>16} | {out_wt:>30}")
    
    # Now the critical test: apply IBM to a vertex and see if the result is different
    print("\n=== Skinning Test: Apply IBM to head vertex ===")
    
    # Find the mesh/primitive for body (skin 1)
    out_mesh = out_js["meshes"][1]
    prim = out_mesh["primitives"][0]
    pos_meta = read_accessor_meta(out_js, prim["attributes"]["POSITION"])
    jnt_meta = read_accessor_meta(out_js, prim["attributes"]["JOINTS_0"])
    wgt_meta = read_accessor_meta(out_js, prim["attributes"]["WEIGHTS_0"])
    
    # Find a vertex near the head (high Y)
    max_y = -999
    max_y_idx = 0
    for vi in range(pos_meta['count']):
        pos = read_vec3(out_bin, pos_meta, vi)
        if pos[1] > max_y:
            max_y = pos[1]
            max_y_idx = vi
    
    pos = read_vec3(out_bin, pos_meta, max_y_idx)
    print(f"Head vertex {max_y_idx}: pos=[{pos[0]:.4f},{pos[1]:.4f},{pos[2]:.4f}]")
    
    # Read joints and weights for this vertex
    jnts = []
    wgts = []
    for lane in range(4):
        off = jnt_meta["base"] + max_y_idx * jnt_meta["stride"]
        if jnt_meta["comp_type"] == 5121:
            j = out_bin[off + lane]
        else:
            j = struct.unpack("<H", out_bin[off + lane*2:off + lane*2 + 2])[0]
        w_off = wgt_meta["base"] + max_y_idx * wgt_meta["stride"] + lane * 4
        w = struct.unpack("<f", out_bin[w_off:w_off+4])[0]
        jnts.append(j)
        wgts.append(w)
    
    print(f"Joints: {jnts}, Weights: {[f'{w:.4f}' for w in wgts]}")
    for j, w in zip(jnts, wgts):
        if w > 0.001 and j < len(out_joints):
            node_idx = out_joints[j]
            name = out_nodes[node_idx].get("name", "?")
            ibm = read_mat4(out_bin, out_ibm_meta, j)
            ibm_t = mat4_translation(ibm)
            
            # Compute IBM * vertex
            pos_h = [pos[0], pos[1], pos[2], 1.0]
            result = [0]*4
            for r in range(4):
                for c in range(4):
                    result[r] += ibm[r][c] * pos_h[c]
            
            print(f"  {name}: weight={w:.4f}, IBM*v=[{result[0]:.4f},{result[1]:.4f},{result[2]:.4f}]")
            print(f"    IBM_t=[{ibm_t[0]:.4f},{ibm_t[1]:.4f},{ibm_t[2]:.4f}]")
            print(f"    vertex - bone_pos = [{pos[0]-(-ibm_t[0]):.4f},{pos[1]-(-ibm_t[1]):.4f},{pos[2]-(-ibm_t[2]):.4f}]")


if __name__ == "__main__":
    main()
