import * as THREE from 'three';

export type IdleBoneMotion = {
  boneName: string;
  xAngles: number[];
  yAngles: number[];
  zAngles: number[];
};

/** BVH joint name → Second Life bone name mapping. */
export const BVH_TO_SL_BONE: Record<string, string> = {
  hip: 'mPelvis',
  abdomen: 'mTorso',
  chest: 'mChest',
  neck: 'mNeck',
  head: 'mHead',
  lCollar: 'mCollarLeft',
  lShldr: 'mShoulderLeft',
  lForeArm: 'mElbowLeft',
  lHand: 'mWristLeft',
  rCollar: 'mCollarRight',
  rShldr: 'mShoulderRight',
  rForeArm: 'mElbowRight',
  rHand: 'mWristRight',
  lThigh: 'mHipLeft',
  lShin: 'mKneeLeft',
  lFoot: 'mAnkleLeft',
  rThigh: 'mHipRight',
  rShin: 'mKneeRight',
  rFoot: 'mAnkleRight'
};

/**
 * Wrist bones are excluded from BVH retargeting because the VRM hand/thumb
 * bind axes often diverge from the BVH convention, causing severe hand collapse.
 */
export const HAND_PROBLEM_BONES = new Set(['mWristLeft', 'mWristRight']);

/**
 * Parse a Three.js KeyframeTrack name into bone name and property.
 * Handles both `.bones[<name>].<prop>` and `<name>.<prop>` formats.
 */
export function parseBvhTrack(trackName: string): { bone: string; property: string } | null {
  const boneTrack = /^\.bones\[(.+?)\]\.(position|quaternion|scale)$/.exec(trackName);
  if (boneTrack) {
    const [, bone, property] = boneTrack;
    if (!bone || !property) {
      return null;
    }
    return { bone, property };
  }

  const simpleTrack = /^([^.[\]]+)\.(position|quaternion|scale)$/.exec(trackName);
  if (simpleTrack) {
    const [, bone, property] = simpleTrack;
    if (!bone || !property) {
      return null;
    }
    return { bone, property };
  }

  return null;
}

// Ry(90°) and its inverse, used to remap BVH quaternions from -Z-forward
// space (Three.js / old GLB) into the +X-forward space of the Ry(90°)-converted
// GLB skeleton.  Without this, X-axis rotations (e.g. leg forward swing) would
// appear as sideways rolls in the preview.
//
// Conjugate-transform formula: q_glb = ry90 * q_bvh * ry90_inv
const _ry90 = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 1, 0), Math.PI / 2);
const _ry90inv = _ry90.clone().invert();

/**
 * Re-express all quaternion samples in `track` in the +X-forward coordinate
 * frame of the Ry(90°)-converted GLB skeleton.
 */
export function applyRy90ToQuatTrack(track: THREE.KeyframeTrack): THREE.KeyframeTrack {
  const values = new Float32Array(track.values);
  const tmp = new THREE.Quaternion();
  for (let i = 0; i < values.length; i += 4) {
    // Use fromArray/toArray to avoid direct index access (avoids lint false positives).
    tmp.fromArray(values, i);
    tmp.premultiply(_ry90).multiply(_ry90inv);
    tmp.toArray(values, i);
  }
  // QuaternionKeyframeTrack stores values as a plain number array internally.
  return new THREE.QuaternionKeyframeTrack(track.name, Array.from(track.times), Array.from(values));
}

/**
 * Retarget a BVH clip onto `targetSkeleton` using the BVH→SL bone map.
 * Returns null when no tracks could be mapped.
 */
export function buildRetargetedClip(
  bvhMotionClip: THREE.AnimationClip,
  targetSkeleton: THREE.Skeleton
): THREE.AnimationClip | null {
  const tracks: THREE.KeyframeTrack[] = [];

  for (const track of bvhMotionClip.tracks) {
    const parsed = parseBvhTrack(track.name);
    if (!parsed) {
      continue;
    }

    const targetBoneName = BVH_TO_SL_BONE[parsed.bone];
    if (!targetBoneName) {
      continue;
    }

    if (!targetSkeleton.getBoneByName(targetBoneName)) {
      continue;
    }

    // BVH wrist orientation and VRM hand/thumb bind axes are often different.
    // Keep collar/shoulder/elbow animation, but skip wrist twist to avoid
    // severe hand collapse while still animating upper body.
    if (parsed.property === 'quaternion' && HAND_PROBLEM_BONES.has(targetBoneName)) {
      continue;
    }

    // BVH root translation is authored in a different coordinate/scale space.
    // Applying position tracks directly can move the whole avatar out of view,
    // so preview uses rotation-only retargeting for deformation checks.
    if (parsed.property === 'position') {
      continue;
    }

    const nextTrack = track.clone();
    nextTrack.name = `.bones[${targetBoneName}].${parsed.property}`;

    // Re-express the BVH quaternion in the +X-forward world of the Ry(90°)-
    // converted GLB so that leg-swing and arm-swing axes match visually.
    const corrected =
      parsed.property === 'quaternion' ? applyRy90ToQuatTrack(nextTrack) : nextTrack;
    corrected.name = nextTrack.name;
    tracks.push(corrected);
  }

  if (tracks.length === 0) {
    return null;
  }

  return new THREE.AnimationClip('avatar_motion_retargeted', bvhMotionClip.duration, tracks);
}

/**
 * Build a 4-second procedural idle clip for bones present in `targetSkeleton`.
 * Returns null when none of the target bones are found in the skeleton.
 */
export function buildProceduralIdleClip(
  targetSkeleton: THREE.Skeleton
): THREE.AnimationClip | null {
  // 4秒ループ。0,2,4秒で同じ姿勢に戻して継ぎ目を消す。
  const times = [0, 1, 2, 3, 4];
  const motions: IdleBoneMotion[] = [
    {
      boneName: 'mTorso',
      xAngles: [0, 1, 0, -1, 0],
      yAngles: [0, 0.4, 0, -0.4, 0],
      zAngles: [0, 0.2, 0, -0.2, 0]
    },
    {
      boneName: 'mChest',
      xAngles: [0, 2.2, 0, -2.2, 0],
      yAngles: [0, 0.6, 0, -0.6, 0],
      zAngles: [0, 0.4, 0, -0.4, 0]
    },
    {
      boneName: 'mNeck',
      xAngles: [0, 1, 0, -1, 0],
      yAngles: [0, -1.2, 0, 1.2, 0],
      zAngles: [0, 0.5, 0, -0.5, 0]
    },
    {
      boneName: 'mHead',
      xAngles: [0, 0.7, 0, -0.7, 0],
      yAngles: [0, 1.8, 0, -1.8, 0],
      zAngles: [0, -0.8, 0, 0.8, 0]
    },
    {
      boneName: 'mCollarLeft',
      xAngles: [0, -0.9, 0, 0.9, 0],
      yAngles: [0, 0.4, 0, -0.4, 0],
      zAngles: [0, -0.7, 0, 0.7, 0]
    },
    {
      boneName: 'mCollarRight',
      xAngles: [0, -0.9, 0, 0.9, 0],
      yAngles: [0, -0.4, 0, 0.4, 0],
      zAngles: [0, 0.7, 0, -0.7, 0]
    }
  ];

  const toQuaternionValues = (xAngles: number[], yAngles: number[], zAngles: number[]) => {
    const values: number[] = [];
    for (let i = 0; i < times.length; i += 1) {
      const xAngle = xAngles.at(i) ?? 0;
      const yAngle = yAngles.at(i) ?? 0;
      const zAngle = zAngles.at(i) ?? 0;
      const q = new THREE.Quaternion().setFromEuler(
        new THREE.Euler(
          THREE.MathUtils.degToRad(xAngle),
          THREE.MathUtils.degToRad(yAngle),
          THREE.MathUtils.degToRad(zAngle),
          'XYZ'
        )
      );
      values.push(q.x, q.y, q.z, q.w);
    }
    return values;
  };

  const tracks: THREE.KeyframeTrack[] = [];
  for (const motion of motions) {
    if (!targetSkeleton.getBoneByName(motion.boneName)) {
      continue;
    }
    tracks.push(
      new THREE.QuaternionKeyframeTrack(
        `.bones[${motion.boneName}].quaternion`,
        times,
        toQuaternionValues(motion.xAngles, motion.yAngles, motion.zAngles)
      )
    );
  }

  if (tracks.length === 0) {
    return null;
  }

  return new THREE.AnimationClip('avatar_idle_synth', times.at(-1) ?? 4, tracks);
}
