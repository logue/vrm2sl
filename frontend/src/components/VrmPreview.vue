<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { readFile } from '@tauri-apps/plugin-fs';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
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

let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;
let renderer: THREE.WebGLRenderer | null = null;
let controls: OrbitControls | null = null;
let modelRoot: THREE.Object3D | null = null;
let resizeObserver: ResizeObserver | null = null;
let animationFrameId = 0;
let reloadTimer: ReturnType<typeof setTimeout> | null = null;

const updateRendererSize = () => {
  if (!canvasHost.value || !renderer || !camera) {
    return;
  }

  const width = canvasHost.value.clientWidth;
  const height = Math.min(Math.max(canvasHost.value.clientHeight, 280), 420);

  renderer.setSize(width, height, false);
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
};

const clearModel = () => {
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

const fitCameraToModel = (root: THREE.Object3D) => {
  if (!camera || !controls) {
    return;
  }

  const box = new THREE.Box3().setFromObject(root);
  const size = box.getSize(new THREE.Vector3());
  const center = box.getCenter(new THREE.Vector3());
  const maxSize = Math.max(size.x, size.y, size.z, 0.1);

  camera.position.x = center.x;
  camera.near = Math.max(maxSize / 200, 0.01);
  camera.far = Math.max(maxSize * 200, 1000);
  camera.updateProjectionMatrix();

  controls.target.x = center.x;
  controls.update();
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
    scene.add(modelRoot);
    fitCameraToModel(modelRoot);
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

  const render = () => {
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

onBeforeUnmount(() => {
  if (reloadTimer) {
    clearTimeout(reloadTimer);
    reloadTimer = null;
  }
  cancelAnimationFrame(animationFrameId);
  resizeObserver?.disconnect();
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
      <v-alert v-if="loading" type="info" class="mt-2" variant="tonal">読み込み中...</v-alert>
      <v-alert v-else-if="errorMessage" type="error" class="mt-2" variant="tonal">
        {{ errorMessage }}
      </v-alert>
      <v-alert v-else-if="!filePath" type="info" class="mt-2" variant="tonal">
        VRMファイルを選択するとここに表示されます。
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
