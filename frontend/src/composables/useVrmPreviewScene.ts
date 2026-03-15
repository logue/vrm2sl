import { invoke } from '@tauri-apps/api/core';
import { readFile } from '@tauri-apps/plugin-fs';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { ref, type Ref } from 'vue';
import * as THREE from 'three';
import type { ConvertOptions } from '@/interfaces';

export interface UseVrmPreviewSceneOptions {
  canvasHost: Ref<HTMLDivElement | null>;
  modelRoot: Ref<THREE.Object3D | null>;
  animationEnabled: Ref<boolean>;
  /** Called at the start of each model load (e.g. detect gender, update motion path). */
  onBeforeLoad: (path: string) => Promise<void>;
  applyOrLoadAnimation: () => Promise<void>;
  stopIdleAnimation: () => void;
  disposeMixer: () => void;
  tickMixer: (delta: number) => void;
}

export function useVrmPreviewScene({
  canvasHost,
  modelRoot,
  animationEnabled,
  onBeforeLoad,
  applyOrLoadAnimation,
  stopIdleAnimation,
  disposeMixer,
  tickMixer
}: UseVrmPreviewSceneOptions) {
  const loading = ref(false);
  const errorMessage = ref('');

  let scene: THREE.Scene | null = null;
  let camera: THREE.PerspectiveCamera | null = null;
  let renderer: THREE.WebGLRenderer | null = null;
  let controls: OrbitControls | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let animationFrameId = 0;
  // Track time manually to avoid using the deprecated THREE.Clock API.
  let lastFrameTime = -1;
  let reloadTimer: ReturnType<typeof setTimeout> | null = null;

  // ---------------------------------------------------------------------------
  // Material helpers
  // ---------------------------------------------------------------------------

  /**
   * Adjust depth/blending settings on eye-related materials so that iris,
   * highlight, lash and brow surfaces render correctly without z-fighting.
   */
  const ensureEyeMaterialsVisible = (root: THREE.Object3D) => {
    root.traverse(object => {
      if (!(object instanceof THREE.Mesh)) {
        return;
      }

      const materials = Array.isArray(object.material) ? object.material : [object.material];

      for (const material of materials) {
        const materialName = (material.name ?? '').toLowerCase();
        const isEyeIris = materialName.includes('eyeiris');
        const isEyeWhite = materialName.includes('eyewhite');
        const isEyeHighlight = materialName.includes('eyehighlight');
        const isEyeSurface = isEyeIris || isEyeWhite || isEyeHighlight;
        const isEyelashLike =
          materialName.includes('faceeyeline') ||
          materialName.includes('eyelash') ||
          materialName.includes('faceline');
        const isBrowLike = materialName.includes('facebrow');

        if (!isEyeSurface && !isEyelashLike && !isBrowLike) {
          continue;
        }

        if (isEyeIris || isEyeHighlight) {
          // Iris/highlight are often near-coplanar with eye-white.
          // Slightly prioritize them to avoid being buried by depth fighting.
          material.alphaTest = 0;
          material.transparent = true;
          material.depthTest = true;
          material.depthWrite = false;
          material.side = THREE.DoubleSide;
          material.polygonOffset = true;
          material.polygonOffsetFactor = -2;
          material.polygonOffsetUnits = -2;
        } else if (isEyeWhite) {
          material.depthTest = true;
          material.depthWrite = true;
          material.polygonOffset = false;
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

  // ---------------------------------------------------------------------------
  // Scene / model lifecycle
  // ---------------------------------------------------------------------------

  const clearModel = () => {
    disposeMixer();
    if (!scene || !modelRoot.value) {
      modelRoot.value = null;
      return;
    }

    scene.remove(modelRoot.value);
    modelRoot.value.traverse(object => {
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
    modelRoot.value = null;
  };

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

  /**
   * Reposition the camera so the full avatar is visible.
   * Ry(+90°) rotates the VRM's default -Z forward direction to -X,
   * so the camera must be placed on the +X side to see the avatar's face.
   */
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

    camera.position.set(center.x + maxSize * 1.8, center.y + maxSize * 0.4, center.z);
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
      await onBeforeLoad(path);

      const previewPath = await invoke<string>('build_preview_glb_command', {
        request: { input_path: path, options }
      });

      const bytes = await readFile(previewPath);
      const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);

      const loader = new GLTFLoader();
      const gltf = await new Promise<THREE.Group | THREE.Object3D>((resolve, reject) => {
        loader.parse(
          buffer,
          '',
          parsed => resolve(parsed.scene),
          parseError =>
            reject(parseError instanceof Error ? parseError : new Error(String(parseError)))
        );
      });

      clearModel();
      modelRoot.value = gltf;

      // Ensure eye materials are visible in the initial (non-BVH) preview as well.
      ensureEyeMaterialsVisible(modelRoot.value);

      modelRoot.value.traverse(object => {
        if (object instanceof THREE.SkinnedMesh) {
          // Animated skinned meshes can be culled incorrectly when bounds are stale.
          // Keep them always renderable in preview to avoid eye/face disappearance.
          object.frustumCulled = false;
        }
      });

      scene.add(modelRoot.value);
      fitCameraToModel(modelRoot.value);

      if (animationEnabled.value) {
        await applyOrLoadAnimation();
      }
    } catch (error) {
      errorMessage.value = `Preview failed: ${String(error)}`;
    } finally {
      loading.value = false;
    }
  };

  const scheduleReload = (path: string, options: ConvertOptions) => {
    if (reloadTimer) {
      clearTimeout(reloadTimer);
    }
    reloadTimer = setTimeout(() => {
      if (!scene) {
        return;
      }
      if (!path) {
        clearModel();
        errorMessage.value = '';
        return;
      }
      void loadPreviewModel(path, options);
    }, 250);
  };

  // ---------------------------------------------------------------------------
  // Scene initialisation / teardown
  // ---------------------------------------------------------------------------

  const initScene = (host: HTMLDivElement) => {
    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x1f1f1f);

    camera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000);
    // Ry(+90°) rotates the VRM's -Z forward to -X.
    // Place camera on the +X side so the avatar's front face is visible.
    camera.position.set(2.5, 1.2, 0);

    renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    host.appendChild(renderer.domElement);

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
    // Light from the +X side to illuminate the avatar's front face.
    directional.position.set(1.5, 2.5, 0.5);
    scene.add(directional);

    const grid = new THREE.GridHelper(10, 20, 0x555555, 0x333333);
    scene.add(grid);

    updateRendererSize();

    resizeObserver = new ResizeObserver(() => {
      updateRendererSize();
    });
    resizeObserver.observe(host);

    lastFrameTime = -1;

    const render = () => {
      const now = performance.now() / 1000;
      const delta = lastFrameTime >= 0 ? now - lastFrameTime : 0;
      lastFrameTime = now;
      tickMixer(delta);
      if (controls) {
        controls.update();
      }
      if (renderer && scene && camera) {
        renderer.render(scene, camera);
      }
      animationFrameId = requestAnimationFrame(render);
    };

    animationFrameId = requestAnimationFrame(render);
  };

  const disposeScene = () => {
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
      renderer.domElement.remove();
    }

    scene = null;
    camera = null;
    renderer = null;
    controls = null;
    resizeObserver = null;
    lastFrameTime = -1;
  };

  return {
    loading,
    errorMessage,
    clearModel,
    loadPreviewModel,
    scheduleReload,
    initScene,
    disposeScene
  };
}
