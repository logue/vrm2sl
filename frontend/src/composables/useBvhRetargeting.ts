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
 * Build a "finger-only" clip from a hand-pose BVH (e.g. the SL stock
 * `Hand_L Paper;,...;L.bvh` / `Hand_R Scissors;,...;L.bvh` files).
 *
 * The source BVH carries the full SL Bento skeleton, but for preview we only
 * want the `mHand*` finger tracks so that the upper-body / wrist motion of
 * the active RPS clip continues to drive the avatar.  Wrist itself and all
 * non-finger bones are dropped here.
 *
 * The SL stock hand BVHs include BOTH left and right finger bones (the
 * inactive side has identity rotations). Playing the left- and right-hand
 * clips simultaneously would therefore produce two competing tracks for
 * every finger bone, which AnimationMixer blends and renders as a high-
 * frequency vibration. Pass `sideFilter` ('left' | 'right') to keep only
 * the matching side's tracks and avoid the conflict.
 *
 * Returns null when the source BVH has no usable finger tracks for the target
 * skeleton.
 */
export function buildFingerOnlyRetargetedClip(
  sourceClip: THREE.AnimationClip,
  targetSkeleton: THREE.Skeleton,
  clipName = 'avatar_finger_pose',
  sideFilter?: 'left' | 'right'
): THREE.AnimationClip | null {
  const tracks: THREE.KeyframeTrack[] = [];
  const bindTmp = new THREE.Quaternion();
  const sampleTmp = new THREE.Quaternion();
  const resultTmp = new THREE.Quaternion();

  for (const track of sourceClip.tracks) {
    const parsed = parseBvhTrack(track.name);
    if (!parsed) {
      continue;
    }
    if (parsed.property !== 'quaternion') {
      continue;
    }
    if (!FINGER_TRACK_BONE_PATTERN.test(parsed.bone)) {
      continue;
    }

    const targetBoneName = resolveTargetBoneName(parsed.bone, targetSkeleton);
    if (!targetBoneName) {
      continue;
    }

    // Drop tracks for the opposite side so left/right clips do not double
    // up on the same finger bones.
    if (sideFilter) {
      const isLeft =
        /left$/i.test(targetBoneName) || /^l(Thumb|Index|Middle|Ring|Pinky)/i.test(parsed.bone);
      const isRight =
        /right$/i.test(targetBoneName) || /^r(Thumb|Index|Middle|Ring|Pinky)/i.test(parsed.bone);
      if (sideFilter === 'left' && !isLeft) continue;
      if (sideFilter === 'right' && !isRight) continue;
    }

    const targetBone = targetSkeleton.getBoneByName(targetBoneName);
    if (!targetBone) {
      continue;
    }

    // SL stock hand BVHs are authored as a 2-frame clip where frame 1 is the
    // rest (identity) pose and frame 2 is the actual hand pose. Playing both
    // frames through AnimationMixer makes the fingers oscillate rapidly
    // between bind pose and the target curl, which visually reads as
    // multiple animations running on top of each other. Sample ONLY the
    // last frame and emit it as a single static keyframe.
    const srcValues = track.values;
    const lastIndex = srcValues.length - 4;
    if (lastIndex < 0) {
      continue;
    }
    sampleTmp.fromArray(srcValues, lastIndex);

    // The GLB SL skeleton inherits the VRM authored bind-pose rotation on
    // each `mHand*` bone, which is generally NOT identity. Compose
    // `q_bind * q_bvh` so identity BVH input leaves the bone at its VRM
    // bind pose while non-identity samples apply the curl as a delta in
    // the bind frame.
    bindTmp.copy(targetBone.quaternion);
    resultTmp.copy(bindTmp).multiply(sampleTmp);

    const staticValues = new Float32Array(4);
    resultTmp.toArray(staticValues, 0);

    const cloned = new THREE.QuaternionKeyframeTrack(
      `.bones[${targetBoneName}].quaternion`,
      [0],
      Array.from(staticValues)
    );
    tracks.push(cloned);
  }

  if (tracks.length === 0) {
    return null;
  }

  // Use a short non-zero duration so AnimationMixer treats this as a clip;
  // with a single keyframe the pose stays static regardless of duration.
  return new THREE.AnimationClip(clipName, 1, tracks);
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

// Removed: buildProceduralRpsHandClip / detectRpsPoseName
// These were preview-only finger-pose injectors that masked the real bind-pose
// problem.  Preview must reflect the exact behavior of the exported GLB in
// Second Life; the actual fix is performed in backend/src/convert/skeleton/finger.rs
// (finger bone bind-pose normalization with companion mesh vertex correction).
