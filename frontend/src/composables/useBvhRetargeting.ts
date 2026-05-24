import * as THREE from 'three';

export type IdleBoneMotion = {
  boneName: string;
  xAngles: number[];
  yAngles: number[];
  zAngles: number[];
};

type RpsPoseName = 'paper' | 'rock' | 'scissors';
type FingerEuler = { x: number; y: number; z: number };

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

const BVH_TO_SL_BONE_MAP: Map<string, string> = new Map(Object.entries(BVH_TO_SL_BONE));

/**
 * Wrist bones are excluded from BVH retargeting because the VRM hand/thumb
 * bind axes often diverge from the BVH convention, causing severe hand collapse.
 */
export const HAND_PROBLEM_BONES = new Set(['mWristLeft', 'mWristRight']);

export interface RetargetClipOptions {
  sourceMotionPath?: string;
}

const FINGER_TRACK_BONE_PATTERN =
  /(mHand(Thumb|Index|Middle|Ring|Pinky)[123](Left|Right)|[lr](Thumb|Index|Middle|Ring|Pinky)[123])/i;

const SHORT_FINGER_BONE_PATTERN = /^([lr])(Thumb|Index|Middle|Ring|Pinky)([123])$/i;

/**
 * RPS motions require wrist tracks to avoid order-dependent hand poses
 * (e.g. Paper being correct only after Scissors was played once).
 */
export function shouldAllowWristQuaternion(sourceMotionPath?: string): boolean {
  if (!sourceMotionPath) {
    return false;
  }
  const path = sourceMotionPath.toLowerCase();
  return (
    path.includes('avatar_rps_paper.bvh') ||
    path.includes('avatar_rps_scissors.bvh') ||
    path.includes('avatar_rps_rock.bvh')
  );
}

/**
 * Returns true when the source BVH clip contains explicit finger quaternion tracks.
 */
export function hasFingerQuaternionTracks(clip: THREE.AnimationClip): boolean {
  return clip.tracks.some(track => {
    const parsed = parseBvhTrack(track.name);
    return (
      !!parsed && parsed.property === 'quaternion' && FINGER_TRACK_BONE_PATTERN.test(parsed.bone)
    );
  });
}

/**
 * Resolve BVH joint names to a target skeleton bone name.
 * Supports:
 * - legacy BVH names in `BVH_TO_SL_BONE`
 * - direct SL names (e.g. mHandIndex1Left)
 * - short finger aliases (e.g. lIndex1 / rThumb3)
 */
export function resolveTargetBoneName(
  bvhBoneName: string,
  targetSkeleton: THREE.Skeleton
): string | null {
  const mapped = BVH_TO_SL_BONE_MAP.get(bvhBoneName);
  if (mapped && targetSkeleton.getBoneByName(mapped)) {
    return mapped;
  }

  if (targetSkeleton.getBoneByName(bvhBoneName)) {
    return bvhBoneName;
  }

  const shortFinger = SHORT_FINGER_BONE_PATTERN.exec(bvhBoneName);
  if (shortFinger) {
    const [, side, finger, segment] = shortFinger;
    const lr = side.toLowerCase() === 'l' ? 'Left' : 'Right';
    const candidate = `mHand${finger}${segment}${lr}`;
    if (targetSkeleton.getBoneByName(candidate)) {
      return candidate;
    }
  }

  return null;
}

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

const HAND_BONE_PATTERN =
  /^m(Wrist(Left|Right)|Hand(Thumb|Index|Middle|Ring|Pinky)[123](Left|Right))$/;

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
 * Hand and finger bones are already authored in a local frame suitable for
 * the converted skeleton in preview; applying global Ry(90) again causes
 * visible twist/curl artifacts.
 */
export function shouldApplyRy90ToBone(targetBoneName: string): boolean {
  return !HAND_BONE_PATTERN.test(targetBoneName);
}

/**
 * Retarget a BVH clip onto `targetSkeleton` using the BVH→SL bone map.
 * Returns null when no tracks could be mapped.
 */
export function buildRetargetedClip(
  bvhMotionClip: THREE.AnimationClip,
  targetSkeleton: THREE.Skeleton,
  options: RetargetClipOptions = {}
): THREE.AnimationClip | null {
  const allowWristQuaternion =
    shouldAllowWristQuaternion(options.sourceMotionPath) &&
    hasFingerQuaternionTracks(bvhMotionClip);

  const tracks: THREE.KeyframeTrack[] = [];

  for (const track of bvhMotionClip.tracks) {
    const parsed = parseBvhTrack(track.name);
    if (!parsed) {
      continue;
    }

    const targetBoneName = resolveTargetBoneName(parsed.bone, targetSkeleton);
    if (!targetBoneName) {
      continue;
    }

    // BVH wrist orientation and VRM hand/thumb bind axes are often different.
    // Keep collar/shoulder/elbow animation, but skip wrist twist to avoid
    // severe hand collapse while still animating upper body.
    if (
      parsed.property === 'quaternion' &&
      HAND_PROBLEM_BONES.has(targetBoneName) &&
      !allowWristQuaternion
    ) {
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
      parsed.property === 'quaternion' && shouldApplyRy90ToBone(targetBoneName)
        ? applyRy90ToQuatTrack(nextTrack)
        : nextTrack;
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

/**
 * Detect RPS pose name from a motion path.
 */
export function detectRpsPoseName(sourceMotionPath?: string): RpsPoseName | null {
  if (!sourceMotionPath) {
    return null;
  }
  const path = sourceMotionPath.toLowerCase();
  if (path.includes('rps_paper')) {
    return 'paper';
  }
  if (path.includes('rps_rock')) {
    return 'rock';
  }
  if (path.includes('rps_scissors')) {
    return 'scissors';
  }
  return null;
}

/**
 * Build a static right-hand finger pose clip for RPS preview.
 * This is used when official RPS BVH files do not include finger tracks.
 */
export function buildProceduralRpsHandClip(
  targetSkeleton: THREE.Skeleton,
  sourceMotionPath?: string
): THREE.AnimationClip | null {
  const pose = detectRpsPoseName(sourceMotionPath);
  if (!pose) {
    return null;
  }

  const times = [0, 1];
  const closed: FingerEuler = { x: 75, y: 0, z: 0 };
  const halfClosed: FingerEuler = { x: 45, y: 0, z: 0 };
  const open: FingerEuler = { x: 0, y: 0, z: 0 };
  const thumbCurl1: FingerEuler = { x: 4, y: 14, z: -26 };
  const thumbCurl2: FingerEuler = { x: 7, y: 10, z: -20 };
  const thumbCurl3: FingerEuler = { x: 6, y: 7, z: -14 };
  const thumbSemiCurl1: FingerEuler = { x: -4, y: 12, z: -24 };
  const thumbSemiCurl2: FingerEuler = { x: -2, y: 9, z: -18 };
  const thumbSemiCurl3: FingerEuler = { x: -1, y: 6, z: -13 };

  const byPose: Record<RpsPoseName, Record<string, FingerEuler>> = {
    paper: {
      mHandIndex1Right: open,
      mHandIndex2Right: open,
      mHandIndex3Right: open,
      mHandMiddle1Right: open,
      mHandMiddle2Right: open,
      mHandMiddle3Right: open,
      mHandRing1Right: open,
      mHandRing2Right: open,
      mHandRing3Right: open,
      mHandPinky1Right: open,
      mHandPinky2Right: open,
      mHandPinky3Right: open
    },
    rock: {
      mHandThumb1Right: thumbCurl1,
      mHandThumb2Right: thumbCurl2,
      mHandThumb3Right: thumbCurl3,
      mHandIndex1Right: closed,
      mHandIndex2Right: closed,
      mHandIndex3Right: halfClosed,
      mHandMiddle1Right: closed,
      mHandMiddle2Right: closed,
      mHandMiddle3Right: halfClosed,
      mHandRing1Right: closed,
      mHandRing2Right: closed,
      mHandRing3Right: halfClosed,
      mHandPinky1Right: closed,
      mHandPinky2Right: closed,
      mHandPinky3Right: halfClosed
    },
    scissors: {
      mHandThumb1Right: thumbSemiCurl1,
      mHandThumb2Right: thumbSemiCurl2,
      mHandThumb3Right: thumbSemiCurl3,
      mHandIndex1Right: open,
      mHandIndex2Right: open,
      mHandIndex3Right: open,
      mHandMiddle1Right: open,
      mHandMiddle2Right: open,
      mHandMiddle3Right: open,
      mHandRing1Right: closed,
      mHandRing2Right: closed,
      mHandRing3Right: halfClosed,
      mHandPinky1Right: closed,
      mHandPinky2Right: closed,
      mHandPinky3Right: halfClosed
    }
  };

  const toValues = (euler: FingerEuler): number[] => {
    const q = new THREE.Quaternion().setFromEuler(
      new THREE.Euler(
        THREE.MathUtils.degToRad(euler.x),
        THREE.MathUtils.degToRad(euler.y),
        THREE.MathUtils.degToRad(euler.z),
        'XYZ'
      )
    );
    return [q.x, q.y, q.z, q.w, q.x, q.y, q.z, q.w];
  };

  const tracks: THREE.KeyframeTrack[] = [];
  let targetPose: Record<string, FingerEuler>;
  if (pose === 'paper') {
    targetPose = byPose.paper;
  } else if (pose === 'rock') {
    targetPose = byPose.rock;
  } else {
    targetPose = byPose.scissors;
  }
  for (const [boneName, euler] of Object.entries(targetPose)) {
    if (!targetSkeleton.getBoneByName(boneName)) {
      continue;
    }
    tracks.push(
      new THREE.QuaternionKeyframeTrack(`.bones[${boneName}].quaternion`, times, toValues(euler))
    );
  }

  if (tracks.length === 0) {
    return null;
  }

  return new THREE.AnimationClip(`avatar_rps_hand_${pose}`, 1, tracks);
}
