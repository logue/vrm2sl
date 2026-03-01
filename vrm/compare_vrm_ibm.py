#!/usr/bin/env python3
"""Compare original VRM IBM rotation components with the output."""
import json
import math
import struct


def read_glb(path):
    with open(path, "rb") as f:
        f.read(4)
        f.read(8)
        jlen = struct.unpack("<I", f.read(4))[0]
        f.read(4)
        js = json.loads(f.read(jlen))
        blen = struct.unpack("<I", f.read(4))[0]
        f.read(4)
        bd = f.read(blen)
    return js, bd


def read_mat4(bd, base, stride, idx):
    off = base + idx * stride
    vals = [struct.unpack("<f", bd[off + i * 4 : off + i * 4 + 4])[0] for i in range(16)]
    return [[vals[c * 4 + r] for c in range(4)] for r in range(4)]


def mat4_rot_dist(m):
    d = 0
    for r in range(3):
        for c in range(3):
            d += (m[r][c] - (1.0 if r == c else 0.0)) ** 2
    return math.sqrt(d)


def node_to_local(node):
    if "matrix" in node:
        vals = node["matrix"]
        return [[vals[c * 4 + r] for c in range(4)] for r in range(4)]
    t = node.get("translation", [0, 0, 0])
    rot = node.get("rotation", [0, 0, 0, 1])
    s = node.get("scale", [1, 1, 1])
    x, y, z, w = rot
    m = [[0] * 4 for _ in range(4)]
    m[0][0] = (1 - 2 * (y * y + z * z)) * s[0]
    m[0][1] = (2 * (x * y - z * w)) * s[1]
    m[0][2] = (2 * (x * z + y * w)) * s[2]
    m[1][0] = (2 * (x * y + z * w)) * s[0]
    m[1][1] = (1 - 2 * (x * x + z * z)) * s[1]
    m[1][2] = (2 * (y * z - x * w)) * s[2]
    m[2][0] = (2 * (x * z - y * w)) * s[0]
    m[2][1] = (2 * (y * z + x * w)) * s[1]
    m[2][2] = (1 - 2 * (x * x + y * y)) * s[2]
    m[0][3] = t[0]
    m[1][3] = t[1]
    m[2][3] = t[2]
    m[3][3] = 1.0
    return m


def mat4_mul(a, b):
    c = [[0] * 4 for _ in range(4)]
    for i in range(4):
        for j in range(4):
            for k in range(4):
                c[i][j] += a[i][k] * b[k][j]
    return c


def compute_worlds(nodes):
    parent_map = {}
    for i, n in enumerate(nodes):
        for ch in n.get("children", []):
            parent_map[ch] = i
    locals_ = [node_to_local(n) for n in nodes]
    worlds = [None] * len(nodes)

    def resolve(idx):
        if worlds[idx] is not None:
            return worlds[idx]
        if idx in parent_map:
            pw = resolve(parent_map[idx])
            worlds[idx] = mat4_mul(pw, locals_[idx])
        else:
            worlds[idx] = locals_[idx]
        return worlds[idx]

    for i in range(len(nodes)):
        resolve(i)
    return worlds


def main():
    js, bd = read_glb("vrm/AvatarSample_A.vrm")
    hbones = js["extensions"]["VRMC_vrm"]["humanoid"]["humanBones"]
    nodes = js["nodes"]
    worlds = compute_worlds(nodes)

    bone_map = {
        "hips": "mPelvis",
        "spine": "mTorso",
        "chest": "mChest",
        "neck": "mNeck",
        "head": "mHead",
        "leftShoulder": "mCollarLeft",
        "leftUpperArm": "mShoulderLeft",
        "leftLowerArm": "mElbowLeft",
        "leftHand": "mWristLeft",
        "rightShoulder": "mCollarRight",
        "rightUpperArm": "mShoulderRight",
        "rightLowerArm": "mElbowRight",
        "rightHand": "mWristRight",
        "leftUpperLeg": "mHipLeft",
        "leftLowerLeg": "mKneeLeft",
        "leftFoot": "mAnkleLeft",
        "rightUpperLeg": "mHipRight",
        "rightLowerLeg": "mKneeRight",
        "rightFoot": "mAnkleRight",
    }

    skin = js["skins"][1]
    joints = skin["joints"]
    ibm_acc = js["accessors"][skin["inverseBindMatrices"]]
    bv = js["bufferViews"][ibm_acc["bufferView"]]
    base = bv.get("byteOffset", 0) + ibm_acc.get("byteOffset", 0)
    stride = bv.get("byteStride", 64)

    print("=== Original VRM: IBM rotation & world rotation ===")
    for vrm_name, sl_name in sorted(bone_map.items()):
        if vrm_name not in hbones:
            continue
        node_idx = hbones[vrm_name]["node"]
        ji = None
        for i, j in enumerate(joints):
            if j == node_idx:
                ji = i
                break
        if ji is None:
            print(f"  {sl_name:20s}: NOT in skin")
            continue
        ibm = read_mat4(bd, base, stride, ji)
        ibm_rd = mat4_rot_dist(ibm)
        w = worlds[node_idx]
        w_rd = mat4_rot_dist(w)
        wt = [w[0][3], w[1][3], w[2][3]]
        ibm_t = [ibm[0][3], ibm[1][3], ibm[2][3]]

        # Check world * IBM ~ identity
        prod = mat4_mul(w, ibm)
        id_dist = 0
        for r in range(4):
            for c in range(4):
                id_dist += (prod[r][c] - (1.0 if r == c else 0.0)) ** 2
        id_dist = math.sqrt(id_dist)

        print(
            f"  {sl_name:20s}: "
            f"IBM_rot={ibm_rd:.4f} world_rot={w_rd:.4f} "
            f"wt=[{wt[0]:7.4f},{wt[1]:7.4f},{wt[2]:7.4f}] "
            f"world*IBM_err={id_dist:.6f}"
        )


if __name__ == "__main__":
    main()
