/**
 * Simulate BVH retargeting in Node.js to verify correctness.
 * Tests if direct quaternion copy from BVH to GLB skeleton produces correct motion.
 */

import {
  Quaternion,
  Vector3,
  Matrix4,
  Bone,
  Skeleton,
  Euler,
  MathUtils,
} from "/Users/logue/Developer/vrm2sl/node_modules/.pnpm/three@0.183.2/node_modules/three/src/Three.js";
import { readFileSync } from "fs";
const fs = { readFileSync };

// Parse BVH manually (matching BVHLoader behavior)
function parseBvhSimple(text) {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);

  const bones = [];
  const boneStack = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    if (line.startsWith("ROOT") || line.startsWith("JOINT")) {
      const name = line.split(/\s+/)[1];
      const bone = {
        name,
        channels: [],
        parent: boneStack.length > 0 ? boneStack[boneStack.length - 1] : null,
        frames: [],
      };
      bones.push(bone);
      boneStack.push(bone);
    } else if (line.startsWith("OFFSET")) {
      const parts = line.split(/\s+/);
      boneStack[boneStack.length - 1].offset = [
        parseFloat(parts[1]),
        parseFloat(parts[2]),
        parseFloat(parts[3]),
      ];
    } else if (line.startsWith("CHANNELS")) {
      const parts = line.split(/\s+/);
      const n = parseInt(parts[1]);
      boneStack[boneStack.length - 1].channels = parts.slice(2, 2 + n);
    } else if (line === "}") {
      boneStack.pop();
    } else if (line === "MOTION") {
      break;
    }
    i++;
  }

  // Parse motion data
  let n_frames = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.startsWith("Frames:")) {
      n_frames = parseInt(line.split(":")[1].trim());
    } else if (line.startsWith("Frame Time:")) {
      i++;
      break;
    }
    i++;
  }

  const frames = [];
  while (i < lines.length && frames.length < n_frames) {
    const line = lines[i];
    if (line.length > 0) {
      frames.push(line.split(/\s+/).map(parseFloat));
    }
    i++;
  }

  return { bones, frames };
}

// Compute quaternion for a bone at a given frame (matching BVHLoader)
function computeBoneQuat(bone, frame_data, offset) {
  const q = new Quaternion(0, 0, 0, 1);
  const vx = new Vector3(1, 0, 0);
  const vy = new Vector3(0, 1, 0);
  const vz = new Vector3(0, 0, 1);
  const tmp = new Quaternion();

  let pos = offset;
  for (const ch of bone.channels) {
    const val = frame_data[pos++];
    const rad = (val * Math.PI) / 180;
    switch (ch) {
      case "Xrotation":
        tmp.setFromAxisAngle(vx, rad);
        q.multiply(tmp);
        break;
      case "Yrotation":
        tmp.setFromAxisAngle(vy, rad);
        q.multiply(tmp);
        break;
      case "Zrotation":
        tmp.setFromAxisAngle(vz, rad);
        q.multiply(tmp);
        break;
    }
  }
  return q;
}

// Get quaternion for specific bone at specific frame
function getBoneQuat(bones, frames, bvh_bone_name, frame_idx) {
  let ch_offset = 0;
  for (const bone of bones) {
    if (bone.name === bvh_bone_name) {
      return computeBoneQuat(bone, frames[frame_idx], ch_offset);
    }
    ch_offset += bone.channels.length;
  }
  return null;
}

// Test: apply lThigh quaternion at each frame and see which direction the shin moves
const bvhText = fs.readFileSync(
  "/Users/logue/Developer/vrm2sl/frontend/public/animations/avatar_walk.bvh",
  "utf8",
);
const { bones, frames } = parseBvhSimple(bvhText);

console.log("BVH bones:", bones.map((b) => b.name).join(", "));
console.log("Frames:", frames.length);
console.log();

// lThigh OFFSET (relative to hip)
const lThigh = bones.find((b) => b.name === "lThigh");
const lShin = bones.find((b) => b.name === "lShin");
console.log("lThigh OFFSET:", lThigh.offset);
console.log("lShin OFFSET:", lShin.offset);

// Normalize lShin direction
const shinDir = new Vector3(...lShin.offset).normalize();
console.log(
  "lShin direction (normalized):",
  shinDir.x.toFixed(4),
  shinDir.y.toFixed(4),
  shinDir.z.toFixed(4),
);

// GLB mKneeLeft direction (from previous analysis)
const glbKneeDir = new Vector3(0, -0.9998, -0.0209); // normalized

console.log("\n--- Per-frame analysis ---");
console.log(
  "Frame | lThigh_X | Shin after rotation (BVH basis) | Shin (GLB basis) | Z (forward)",
);

for (let fi = 0; fi < frames.length; fi++) {
  const lThighQuat = getBoneQuat(bones, frames, "lThigh", fi);

  // Apply rotation to shin direction (BVH basis)
  const bvhShinRotated = shinDir.clone().applyQuaternion(lThighQuat);

  // Apply rotation to GLB knee direction
  const glbShinRotated = glbKneeDir.clone().applyQuaternion(lThighQuat);

  // Get X rotation channel value
  let ch_offset = 0;
  for (const bone of bones) {
    if (bone.name === "lThigh") {
      const xrotIdx = bone.channels.indexOf("Xrotation");
      if (xrotIdx >= 0) {
        const xrot = frames[fi][ch_offset + xrotIdx];
        console.log(
          `  ${fi.toString().padStart(2)}: Xrot=${xrot.toFixed(1).padStart(7)} | BVH shin Z=${bvhShinRotated.z.toFixed(3).padStart(6)} | GLB shin Z=${glbShinRotated.z.toFixed(3).padStart(6)}`,
        );
      }
      break;
    }
    ch_offset += bone.channels.length;
  }
}

console.log("\nNote: Positive Z = backwards (+Z is behind the avatar in BVH)");
console.log("       Negative Z = forwards (-Z is in front)");
console.log(
  "       Walking should alternate: shin swings from back to front = Z decreasing",
);
