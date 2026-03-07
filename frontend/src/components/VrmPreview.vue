<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { readFile } from '@tauri-apps/plugin-fs';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { BVHLoader } from 'three/examples/jsm/loaders/BVHLoader.js';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import * as THREE from 'three';
import type { ConvertOptions } from '@/interfaces';

const props = defineProps<{
  filePath: string;
  options: ConvertOptions;
}>();

const canvasHost = ref<HTMLDivElement | null>(null);
const errorMessage = ref('');
const loading = ref(false);
const animationEnabled = ref(false);
const animationStatus = ref('待機モーションは無効です');

let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;
let renderer: THREE.WebGLRenderer | null = null;
let controls: OrbitControls | null = null;
let modelRoot: THREE.Object3D | null = null;
let resizeObserver: ResizeObserver | null = null;
let animationFrameId = 0;
let reloadTimer: ReturnType<typeof setTimeout> | null = null;
let clock: THREE.Clock | null = null;
let bvhIdleClip: THREE.AnimationClip | null = null;
let mixer: THREE.AnimationMixer | null = null;
let activeActions: THREE.AnimationAction[] = [];

const BVH_TO_SL_BONE: Record<string, string> = {
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

const HAND_PROBLEM_BONES = new Set([
  'mCollarLeft',
  'mShoulderLeft',
  'mElbowLeft',
  'mWristLeft',
  'mCollarRight',
  'mShoulderRight',
  'mElbowRight',
  'mWristRight'
]);

const updateRendererSize = () => {
  if (!canvasHost.value || !renderer || !camera) {
    return;
  }

  const width = canvasHost.value.clientWidth;
  const height = Math.min(Math.max(canvasHost.value.clientHeight, 280), 420);

  renderer.setSize(width, height, true);
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
};

const clearModel = () => {
  if (mixer) {
    mixer.stopAllAction();
    mixer.uncacheRoot(mixer.getRoot());
    mixer = null;
  }
  activeActions = [];

  if (!scene || !modelRoot) {
    return;
  }

  scene.remove(modelRoot);
  modelRoot.traverse(object => {
    if (!(object instanceof THREE.Mesh)) {
      return;
    }

    object.geometry.dispose();

    if (Array.isArray(object.material)) {
      object.material.forEach(material => material.dispose());
    } else {
      object.material.dispose();
    }
  });

  modelRoot = null;
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

const ensureEyeMaterialsVisible = (root: THREE.Object3D) => {
  root.traverse(object => {
    if (!(object instanceof THREE.Mesh)) {
      return;
    }

    const materials = Array.isArray(object.material) ? object.material : [object.material];

    for (const material of materials) {
      const materialName = (material.name ?? '').toLowerCase();
      const isEyeSurface =
        materialName.includes('eyeiris') ||
        materialName.includes('eyewhite') ||
        materialName.includes('eyehighlight');
      const isEyelashLike =
        materialName.includes('faceeyeline') ||
        materialName.includes('eyelash') ||
        materialName.includes('faceline');
      const isBrowLike = materialName.includes('facebrow');

      if (!isEyeSurface && !isEyelashLike && !isBrowLike) {
        continue;
      }

      if (isEyeSurface) {
        material.alphaTest = 0.0;
        material.transparent = true;
        material.depthWrite = false;
        // Keep normal depth test so overall mesh ordering stays natural.
        material.depthTest = true;
        material.side = THREE.DoubleSide;
        material.polygonOffset = true;
        material.polygonOffsetFactor = -1;
        material.polygonOffsetUnits = -1;
      } else {
        // Eyelashes/brows rely on smooth alpha blending.
        material.alphaTest = 0.02;
        material.transparent = true;
        material.depthWrite = false;
        material.depthTest = true;
        material.side = THREE.DoubleSide;
        material.polygonOffset = false;
      }

      material.needsUpdate = true;
    }
  });
};

const resetSkinnedMeshesToBindPose = () => {
  if (!modelRoot) {
    return;
  }

  for (const skinnedMesh of collectSkinnedMeshes(modelRoot)) {
    skinnedMesh.pose();
    skinnedMesh.skeleton.update();
  }
  modelRoot.updateMatrixWorld(true);
};

const parseBvhTrack = (trackName: string): { bone: string; property: string } | null => {
  const boneTrack = trackName.match(/^\.bones\[(.+?)\]\.(position|quaternion|scale)$/);
  if (boneTrack) {
    const [, bone, property] = boneTrack;
    if (!bone || !property) {
      return null;
    }
    return { bone, property };
  }

  const simpleTrack = trackName.match(/^([^.[\]]+)\.(position|quaternion|scale)$/);
  if (simpleTrack) {
    const [, bone, property] = simpleTrack;
    if (!bone || !property) {
      return null;
    }
    return { bone, property };
  }

  return null;
};

const buildRetargetedClip = (targetSkeleton: THREE.Skeleton): THREE.AnimationClip | null => {
  if (!bvhIdleClip) {
    return null;
  }

  const tracks: THREE.KeyframeTrack[] = [];

  for (const track of bvhIdleClip.tracks) {
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
    // Applying wrist rotation directly can cause thumb collapse in preview,
    // so wrist tracks are skipped for deformation diagnostics.
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
    tracks.push(nextTrack);
  }

  if (tracks.length === 0) {
    return null;
  }

  return new THREE.AnimationClip('avatar_stand_retargeted', bvhIdleClip.duration, tracks);
};

const applyIdleAnimation = () => {
  if (!modelRoot || !animationEnabled.value) {
    return;
  }

  if (!bvhIdleClip) {
    animationStatus.value = '待機モーション読み込み待ちです。';
    return;
  }

  const skinnedMeshes = collectSkinnedMeshes(modelRoot);
  if (skinnedMeshes.length === 0) {
    animationStatus.value = 'スキン付きメッシュが見つからず、待機モーションを適用できません。';
    return;
  }

  if (mixer) {
    mixer.stopAllAction();
    mixer.uncacheRoot(mixer.getRoot());
  }
  activeActions = [];

  mixer = new THREE.AnimationMixer(modelRoot);

  let appliedMeshCount = 0;
  let appliedTrackCount = 0;
  let maxKeyframes = 0;

  for (const skinnedMesh of skinnedMeshes) {
    const retargeted = buildRetargetedClip(skinnedMesh.skeleton);
    if (!retargeted) {
      continue;
    }

    appliedMeshCount += 1;
    appliedTrackCount += retargeted.tracks.length;
    for (const track of retargeted.tracks) {
      maxKeyframes = Math.max(maxKeyframes, track.times.length);
    }

    const action = mixer.clipAction(retargeted, skinnedMesh);
    action.setLoop(THREE.LoopRepeat, Infinity);
    action.clampWhenFinished = false;
    action.enabled = true;
    action.play();
    activeActions.push(action);
  }

  if (appliedMeshCount === 0) {
    animationStatus.value = '待機モーションのボーントラックが一致しませんでした。';
    return;
  }

  if (maxKeyframes <= 1) {
    animationStatus.value = `待機ポーズ適用中（このBVHは1フレームのため静止） meshes: ${appliedMeshCount}, tracks: ${appliedTrackCount}`;
    return;
  }

  animationStatus.value = `待機モーション再生中（回転のみ） meshes: ${appliedMeshCount}, tracks: ${appliedTrackCount}`;
};

const stopIdleAnimation = () => {
  if (!mixer) {
    resetSkinnedMeshesToBindPose();
    animationStatus.value = '待機モーション停止中';
    return;
  }

  mixer.stopAllAction();
  mixer.setTime(0);
  mixer = null;
  activeActions = [];
  resetSkinnedMeshesToBindPose();
  animationStatus.value = '待機モーション停止中';
};

const loadIdleBvh = async () => {
  try {
    const loader = new BVHLoader();
    const result = await loader.loadAsync('/animations/avatar_stand.bvh');
    bvhIdleClip = result.clip;
    animationStatus.value = `待機モーション読込済み (frames: ${Math.round(result.clip.duration)})`;

    if (modelRoot && animationEnabled.value) {
      applyIdleAnimation();
    }
  } catch (error) {
    bvhIdleClip = null;
    animationStatus.value = `待機モーション読込失敗: ${String(error)}`;
  }
};

const fitCameraToModel = (root: THREE.Object3D) => {
  if (!camera || !controls) {
    return;
  }

  root.updateMatrixWorld(true);
  const box = new THREE.Box3().setFromObject(root);
  if (box.isEmpty()) {
    return;
  }

  const size = box.getSize(new THREE.Vector3());
  const center = box.getCenter(new THREE.Vector3());
  const maxSize = Math.max(size.x, size.y, size.z, 0.1);

  camera.position.set(center.x, center.y + maxSize * 0.4, center.z + maxSize * 1.8);
  camera.near = Math.max(maxSize / 200, 0.01);
  camera.far = Math.max(maxSize * 200, 1000);
  camera.updateProjectionMatrix();
  camera.lookAt(center);

  controls.target.copy(center);
  controls.update();
  controls.saveState();
};

const loadPreviewModel = async (path: string, options: ConvertOptions) => {
  if (!path || !scene) {
    return;
  }

  loading.value = true;
  errorMessage.value = '';

  try {
    const previewPath = await invoke<string>('build_preview_glb_command', {
      request: {
        input_path: path,
        options
      }
    });

    const bytes = await readFile(previewPath);
    const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);

    const loader = new GLTFLoader();

    const gltf = await new Promise<THREE.Group | THREE.Object3D>((resolve, reject) => {
      loader.parse(
        buffer,
        '',
        parsed => {
          resolve(parsed.scene);
        },
        parseError => reject(parseError)
      );
    });

    clearModel();
    modelRoot = gltf;

    // Ensure eye materials are visible in the initial (non-BVH) preview as well.
    ensureEyeMaterialsVisible(modelRoot);

    modelRoot.traverse(object => {
      if (object instanceof THREE.SkinnedMesh) {
        // Animated skinned meshes can be culled incorrectly when bounds are stale.
        // Keep them always renderable in preview to avoid eye/face disappearance.
        object.frustumCulled = false;
      }
    });

    scene.add(modelRoot);
    fitCameraToModel(modelRoot);

    if (animationEnabled.value) {
      applyIdleAnimation();
    }
  } catch (error) {
    errorMessage.value = `Preview failed: ${String(error)}`;
  } finally {
    loading.value = false;
  }
};

const scheduleReload = () => {
  if (reloadTimer) {
    clearTimeout(reloadTimer);
  }

  reloadTimer = setTimeout(() => {
    if (!scene) {
      return;
    }

    if (!props.filePath) {
      clearModel();
      errorMessage.value = '';
      return;
    }

    void loadPreviewModel(props.filePath, props.options);
  }, 250);
};

onMounted(() => {
  if (!canvasHost.value) {
    return;
  }

  scene = new THREE.Scene();
  scene.background = new THREE.Color(0x1f1f1f);

  camera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000);
  camera.position.set(0, 1.2, 2.5);

  renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  canvasHost.value.appendChild(renderer.domElement);

  controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.minDistance = 0.2;
  controls.maxDistance = 50;
  controls.target.set(0, 0, 0);
  camera.lookAt(controls.target);
  controls.update();

  const ambient = new THREE.HemisphereLight(0xffffff, 0x444444, 0.9);
  scene.add(ambient);

  const directional = new THREE.DirectionalLight(0xffffff, 0.9);
  directional.position.set(1.5, 2.5, 2.0);
  scene.add(directional);

  const grid = new THREE.GridHelper(10, 20, 0x555555, 0x333333);
  scene.add(grid);

  updateRendererSize();

  resizeObserver = new ResizeObserver(() => {
    updateRendererSize();
  });
  resizeObserver.observe(canvasHost.value);

  clock = new THREE.Clock();

  const render = () => {
    if (clock && mixer) {
      const delta = clock.getDelta();
      mixer.update(delta);
    }
    if (controls) {
      controls.update();
    }
    if (renderer && scene && camera) {
      renderer.render(scene, camera);
    }
    animationFrameId = requestAnimationFrame(render);
  };

  animationFrameId = requestAnimationFrame(render);

  if (props.filePath) {
    void loadPreviewModel(props.filePath, props.options);
  }
});

watch(
  () => [
    props.filePath,
    props.options.target_height_cm,
    props.options.manual_scale,
    props.options.texture_auto_resize,
    props.options.texture_resize_method
  ],
  () => {
    scheduleReload();
  }
);

watch(
  () => animationEnabled.value,
  enabled => {
    if (enabled) {
      if (!bvhIdleClip) {
        void loadIdleBvh();
      } else {
        applyIdleAnimation();
      }
      return;
    }
    stopIdleAnimation();
  }
);

onBeforeUnmount(() => {
  if (reloadTimer) {
    clearTimeout(reloadTimer);
    reloadTimer = null;
  }
  cancelAnimationFrame(animationFrameId);
  resizeObserver?.disconnect();
  stopIdleAnimation();
  clearModel();
  controls?.dispose();
  renderer?.dispose();

  if (renderer?.domElement.parentElement) {
    renderer.domElement.parentElement.removeChild(renderer.domElement);
  }

  scene = null;
  camera = null;
  renderer = null;
  controls = null;
  resizeObserver = null;
  clock = null;
});
</script>

<template>
  <v-card>
    <v-card-title>
      <v-icon icon="mdi-cube-outline" class="mr-2" />
      VRMプレビュー(three.js)
    </v-card-title>
    <v-card-text>
      <div ref="canvasHost" class="preview-host" />
      <div class="d-flex flex-wrap ga-3 align-center mt-3">
        <v-switch
          v-model="animationEnabled"
          color="primary"
          density="compact"
          hide-details
          label="待機モーション (avatar_stand.bvh)"
        />
      </div>
      <v-alert v-if="loading" type="info" class="mt-2" variant="tonal">読み込み中...</v-alert>
      <v-alert v-else-if="errorMessage" type="error" class="mt-2" variant="tonal">
        {{ errorMessage }}
      </v-alert>
      <v-alert v-else-if="!filePath" type="info" class="mt-2" variant="tonal">
        VRMファイルを選択するとここに表示されます。
      </v-alert>
      <v-alert v-else type="info" class="mt-2" variant="tonal">
        {{ animationStatus }}
      </v-alert>
    </v-card-text>
  </v-card>
</template>

<style scoped>
.preview-host {
  width: 100%;
  height: 360px;
  max-height: 420px;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.12);
}
</style>
