// The i18n keys used in this composable are defined in the calling component's
// scoped <i18n> block (VrmPreview.vue). The @intlify plugin only knows about
// global resources and therefore reports the keys as missing. The keys are
// correct at runtime; the plugin cannot resolve component-scoped i18n blocks.
import { ref, watch, type Ref } from 'vue';

import * as THREE from 'three';
import { BVHLoader } from 'three/examples/jsm/loaders/BVHLoader.js';

import {
  buildRetargetedClip,
  buildProceduralIdleClip,
  buildFingerOnlyRetargetedClip
} from './useBvhRetargeting';

import type { MotionMode } from './useVrmFile';

import { formatPreviewMotionTitle } from '@/constants/previewAnimations';

// Accept any string key to work with component-scoped <i18n> blocks that are
// not visible to the vue-i18n TypeScript plugin from inside an external composable.
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- component-scoped i18n blocks are not visible to the type plugin
type TFunction = (key: any, params?: any) => string;

/**
 * Finger test-pose settings forwarded from ConfigStore.fingers.
 * `enabled`: whether the test-pose overlay is active in preview.
 * `test_pose`: 'open' | 'fist' | 'scissors' | 'paper'.
 *   - 'open' leaves fingers driven by the active motion clip
 *   - others apply the matching SL stock hand-pose BVH on top of the motion
 */
export type FingerTestPose = 'open' | 'fist' | 'scissors' | 'paper';

export interface UseAvatarAnimationOptions {
  modelRoot: Ref<THREE.Object3D | null>;
  animationEnabled: Ref<boolean>;
  selectedMotionMode: Ref<MotionMode>;
  currentMotionPath: Ref<string>;
  fingerTestSettings?: Ref<{ enabled: boolean; test_pose: string }>;
  t: TFunction;
}

export function useAvatarAnimation({
  modelRoot,
  animationEnabled,
  selectedMotionMode,
  currentMotionPath,
  fingerTestSettings,
  t
}: UseAvatarAnimationOptions) {
  const animationStatus = ref('');

  let bvhMotionClip: THREE.AnimationClip | null = null;
  let mixer: THREE.AnimationMixer | null = null;
  const bvhClipCache: Map<string, THREE.AnimationClip> = new Map();

  // Cache for finger-pose companion BVHs (left/right) keyed by their public path.
  let fingerLeftClip: THREE.AnimationClip | null = null;
  let fingerRightClip: THREE.AnimationClip | null = null;

  /**
   * Resolve the companion finger-pose BVH paths.
   *
   * Priority:
   * 1. If the active motion is an RPS BVH, use the matching RPS pose.
   * 2. Otherwise, if finger-test overlay is enabled and the pose is not
   *    'open', use the matching test pose.
   * 3. Returns null when no finger overlay is desired.
   */
  const resolveFingerCompanionPaths = (
    motionPath: string
  ): { left: string; right: string } | null => {
    const lower = motionPath.toLowerCase();
    let pose: 'paper' | 'rock' | 'scissors' | null = null;
    if (lower.includes('avatar_rps_paper')) {
      pose = 'paper';
    } else if (lower.includes('avatar_rps_rock')) {
      pose = 'rock';
    } else if (lower.includes('avatar_rps_scissors')) {
      pose = 'scissors';
    }

    if (!pose && fingerTestSettings?.value.enabled) {
      const tp = fingerTestSettings.value.test_pose;
      if (tp === 'fist') pose = 'rock';
      else if (tp === 'scissors') pose = 'scissors';
      else if (tp === 'paper') pose = 'paper';
    }

    if (!pose) return null;
    return {
      left: `/animations/finger_${pose}_left.bvh`,
      right: `/animations/finger_${pose}_right.bvh`
    };
  };

  const collectSkinnedMeshes = (root: THREE.Object3D): THREE.SkinnedMesh[] => {
    const meshes: THREE.SkinnedMesh[] = [];
    root.traverse(object => {
      if (object instanceof THREE.SkinnedMesh && object.skeleton) {
        meshes.push(object);
      }
    });
    return meshes;
  };

  const resetSkinnedMeshesToBindPose = () => {
    if (!modelRoot.value) {
      return;
    }
    for (const skinnedMesh of collectSkinnedMeshes(modelRoot.value)) {
      skinnedMesh.pose();
      skinnedMesh.skeleton.update();
    }
    modelRoot.value.updateMatrixWorld(true);
  };

  const disposeMixer = () => {
    if (mixer) {
      mixer.stopAllAction();
      mixer.uncacheRoot(mixer.getRoot());
      mixer = null;
    }
  };

  const applyIdleAnimation = () => {
    if (!modelRoot.value || !animationEnabled.value) {
      return;
    }

    const allowProceduralIdle = selectedMotionMode.value === 'idle';
    if (!bvhMotionClip && !allowProceduralIdle) {
      animationStatus.value = t('status_waiting');
      return;
    }

    const skinnedMeshes = collectSkinnedMeshes(modelRoot.value);
    if (skinnedMeshes.length === 0) {
      animationStatus.value = t('status_no_skinned_mesh');
      return;
    }

    // Clear previous clip residue (especially fingers) so clips without
    // full hand tracks do not inherit Paper/Scissors poses.
    resetSkinnedMeshesToBindPose();

    disposeMixer();
    mixer = new THREE.AnimationMixer(modelRoot.value);
    const activeMixer = mixer;

    let appliedMeshCount = 0;
    let appliedTrackCount = 0;
    let maxKeyframes = 0;
    let proceduralApplied = false;

    for (const skinnedMesh of skinnedMeshes) {
      const retargeted = bvhMotionClip
        ? buildRetargetedClip(bvhMotionClip, skinnedMesh.skeleton, {
            sourceMotionPath: currentMotionPath.value
          })
        : null;
      const proceduralIdle = allowProceduralIdle
        ? buildProceduralIdleClip(skinnedMesh.skeleton)
        : null;
      const fingerLeftSource = fingerLeftClip;
      const fingerLeft = fingerLeftSource
        ? (() => {
            try {
              return buildFingerOnlyRetargetedClip(
                fingerLeftSource,
                skinnedMesh.skeleton,
                'avatar_finger_pose_left',
                'left'
              );
            } catch (e) {
              console.error('[finger overlay] left build failed', e);
              return null;
            }
          })()
        : null;
      const fingerRightSource = fingerRightClip;
      const fingerRight = fingerRightSource
        ? (() => {
            try {
              return buildFingerOnlyRetargetedClip(
                fingerRightSource,
                skinnedMesh.skeleton,
                'avatar_finger_pose_right',
                'right'
              );
            } catch (e) {
              console.error('[finger overlay] right build failed', e);
              return null;
            }
          })()
        : null;

      if (!retargeted && !proceduralIdle && !fingerLeft && !fingerRight) {
        continue;
      }

      appliedMeshCount += 1;

      const playClip = (clip: THREE.AnimationClip, weight: number) => {
        appliedTrackCount += clip.tracks.length;
        for (const track of clip.tracks) {
          maxKeyframes = Math.max(maxKeyframes, track.times.length);
        }
        const action = activeMixer.clipAction(clip, skinnedMesh);
        action.setLoop(THREE.LoopRepeat, Infinity);
        action.clampWhenFinished = false;
        action.enabled = true;
        action.setEffectiveWeight(weight);
        action.play();
      };

      if (retargeted) {
        playClip(retargeted, proceduralIdle ? 0.85 : 1);
      }
      if (proceduralIdle) {
        proceduralApplied = true;
        playClip(proceduralIdle, retargeted ? 0.35 : 1);
      }
      // Finger-pose companions override only mHand* finger bones; play at full
      // weight so they cleanly drive the hand pose alongside the main clip.
      if (fingerLeft) {
        playClip(fingerLeft, 1);
      }
      if (fingerRight) {
        playClip(fingerRight, 1);
      }
    }

    if (appliedMeshCount === 0) {
      animationStatus.value = t('status_no_matching_bones');
      return;
    }

    let modeLabel = formatPreviewMotionTitle(currentMotionPath.value);
    if (selectedMotionMode.value === 'walk') {
      modeLabel = t('motion_walk');
    } else if (selectedMotionMode.value === 'idle') {
      modeLabel = t('motion_idle');
    }
    const synthSuffix = proceduralApplied ? ' + synth idle' : '';

    if (maxKeyframes <= 1) {
      animationStatus.value =
        t('status_pose_applied', {
          mode: modeLabel,
          meshes: appliedMeshCount,
          tracks: appliedTrackCount
        }) + synthSuffix;
      return;
    }

    animationStatus.value =
      t('status_motion_playing', {
        mode: modeLabel,
        meshes: appliedMeshCount,
        tracks: appliedTrackCount
      }) + synthSuffix;
  };

  const stopIdleAnimation = () => {
    if (!mixer) {
      resetSkinnedMeshesToBindPose();
      animationStatus.value = t('status_stopped');
      return;
    }
    mixer.stopAllAction();
    mixer.setTime(0);
    mixer = null;
    resetSkinnedMeshesToBindPose();
    animationStatus.value = t('status_stopped');
  };

  const loadFingerCompanionClips = async (motionPath: string) => {
    const paths = resolveFingerCompanionPaths(motionPath);
    if (!paths) {
      fingerLeftClip = null;
      fingerRightClip = null;
      return;
    }

    const loadOne = async (path: string): Promise<THREE.AnimationClip | null> => {
      const cached = bvhClipCache.get(path);
      if (cached) {
        return cached;
      }
      try {
        const loader = new BVHLoader();
        const result = await loader.loadAsync(path);
        bvhClipCache.set(path, result.clip);
        return result.clip;
      } catch (e) {
        console.error('[finger companion] load failed', path, e);
        return null;
      }
    };

    const [left, right] = await Promise.all([loadOne(paths.left), loadOne(paths.right)]);
    fingerLeftClip = left;
    fingerRightClip = right;
  };

  const loadSelectedBvh = async () => {
    const motionPath = currentMotionPath.value;

    // Step 1: load the main motion BVH. Only this step is allowed to flip the
    // status to `status_bvh_failed`; downstream finger overlay or apply
    // failures must never blank out the main motion clip.
    try {
      const cached = bvhClipCache.get(motionPath);
      if (cached) {
        bvhMotionClip = cached;
        animationStatus.value = t('status_bvh_loaded', {
          file: motionPath.split('/').pop(),
          frames: Math.max(...cached.tracks.map(track => track.times.length), 0)
        });
      } else {
        const loader = new BVHLoader();
        const result = await loader.loadAsync(motionPath);
        bvhMotionClip = result.clip;
        bvhClipCache.set(motionPath, result.clip);
        animationStatus.value = t('status_bvh_loaded', {
          file: motionPath.split('/').pop(),
          frames: Math.max(...result.clip.tracks.map(track => track.times.length), 0)
        });
      }
    } catch (error) {
      bvhMotionClip = null;
      fingerLeftClip = null;
      fingerRightClip = null;
      animationStatus.value = t('status_bvh_failed', {
        path: motionPath,
        error: String(error)
      });
      return;
    }

    // Step 2: load finger-pose companions. Failures here are non-fatal and
    // must not affect the main motion clip nor the status message.
    try {
      await loadFingerCompanionClips(motionPath);
    } catch (error) {
      console.error('[finger companion] outer load failed', error);
      fingerLeftClip = null;
      fingerRightClip = null;
    }

    // Step 3: apply the animation. Errors here are logged but must not change
    // the status to `status_bvh_failed`, because the main motion clip is OK.
    if (modelRoot.value && animationEnabled.value) {
      try {
        applyIdleAnimation();
      } catch (error) {
        console.error('[animation apply] failed', error);
      }
    }
  };

  /**
   * Apply animation immediately if the BVH is already cached,
   * otherwise load it first then apply.
   */
  const applyOrLoadAnimation = async () => {
    // Always clear previous pose/action before switching clips. This prevents
    // static hand poses (e.g. Paper) from sticking when the next clip has
    // partial hand/finger tracks or while BVH is loading.
    disposeMixer();
    resetSkinnedMeshesToBindPose();

    // Reset finger-pose companions; loaders below repopulate them when the
    // new motion is an RPS pose.
    fingerLeftClip = null;
    fingerRightClip = null;

    bvhMotionClip = bvhClipCache.get(currentMotionPath.value) ?? null;
    if (bvhMotionClip) {
      await loadFingerCompanionClips(currentMotionPath.value);
      applyIdleAnimation();
    } else {
      await loadSelectedBvh();
    }
  };

  /** Advance the mixer by `delta` seconds. Called each render frame. */
  const tickMixer = (delta: number) => {
    mixer?.update(delta);
  };

  // React to finger test-pose changes so the preview updates live without
  // requiring the user to reselect a motion.
  if (fingerTestSettings) {
    watch(
      () => [fingerTestSettings.value.enabled, fingerTestSettings.value.test_pose] as const,
      () => {
        if (!modelRoot.value || !animationEnabled.value) {
          return;
        }
        void applyOrLoadAnimation();
      }
    );
  }

  return {
    animationStatus,
    applyIdleAnimation,
    stopIdleAnimation,
    loadSelectedBvh,
    applyOrLoadAnimation,
    resetSkinnedMeshesToBindPose,
    tickMixer,
    disposeMixer
  };
}
