// The i18n keys used in this composable are defined in the calling component's
// scoped <i18n> block (VrmPreview.vue). The @intlify plugin only knows about
// global resources and therefore reports the keys as missing. Suppress those
// warnings for this file since the keys are correct at runtime.
/* eslint-disable @intlify/vue-i18n/no-missing-keys */
import { BVHLoader } from 'three/examples/jsm/loaders/BVHLoader.js';
import { ref, type Ref } from 'vue';
import * as THREE from 'three';
import { buildRetargetedClip, buildProceduralIdleClip } from './useBvhRetargeting';
import type { MotionMode } from './useVrmGender';

// Accept any string key to work with component-scoped <i18n> blocks that are
// not visible to the vue-i18n TypeScript plugin from inside an external composable.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type TFunction = (key: any, params?: any) => string;

export interface UseAvatarAnimationOptions {
  modelRoot: Ref<THREE.Object3D | null>;
  animationEnabled: Ref<boolean>;
  selectedMotionMode: Ref<MotionMode>;
  currentMotionPath: Ref<string>;
  t: TFunction;
}

export function useAvatarAnimation({
  modelRoot,
  animationEnabled,
  selectedMotionMode,
  currentMotionPath,
  t
}: UseAvatarAnimationOptions) {
  const animationStatus = ref('');

  let bvhMotionClip: THREE.AnimationClip | null = null;
  let mixer: THREE.AnimationMixer | null = null;
  const bvhClipCache: Map<string, THREE.AnimationClip> = new Map();

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

    disposeMixer();
    mixer = new THREE.AnimationMixer(modelRoot.value);

    let appliedMeshCount = 0;
    let appliedTrackCount = 0;
    let maxKeyframes = 0;
    let proceduralApplied = false;

    for (const skinnedMesh of skinnedMeshes) {
      const retargeted = bvhMotionClip
        ? buildRetargetedClip(bvhMotionClip, skinnedMesh.skeleton)
        : null;
      const proceduralIdle = allowProceduralIdle
        ? buildProceduralIdleClip(skinnedMesh.skeleton)
        : null;

      if (!retargeted && !proceduralIdle) {
        continue;
      }

      appliedMeshCount += 1;

      const playClip = (clip: THREE.AnimationClip, weight: number) => {
        appliedTrackCount += clip.tracks.length;
        for (const track of clip.tracks) {
          maxKeyframes = Math.max(maxKeyframes, track.times.length);
        }
        // mixer is guaranteed non-null here (set above before the loop).
        const action = mixer.clipAction(clip, skinnedMesh);
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
    }

    if (appliedMeshCount === 0) {
      animationStatus.value = t('status_no_matching_bones');
      return;
    }

    const modeLabel = selectedMotionMode.value === 'walk' ? t('motion_walk') : t('motion_idle');
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

  const loadSelectedBvh = async () => {
    const motionPath = currentMotionPath.value;
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

      if (modelRoot.value && animationEnabled.value) {
        applyIdleAnimation();
      }
    } catch (error) {
      bvhMotionClip = null;
      animationStatus.value = t('status_bvh_failed', {
        path: motionPath,
        error: String(error)
      });
    }
  };

  /**
   * Apply animation immediately if the BVH is already cached,
   * otherwise load it first then apply.
   */
  const applyOrLoadAnimation = async () => {
    bvhMotionClip = bvhClipCache.get(currentMotionPath.value) ?? null;
    if (bvhMotionClip) {
      applyIdleAnimation();
    } else {
      await loadSelectedBvh();
    }
  };

  /** Advance the mixer by `delta` seconds. Called each render frame. */
  const tickMixer = (delta: number) => {
    mixer?.update(delta);
  };

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
