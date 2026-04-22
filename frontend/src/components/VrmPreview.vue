<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
import { useI18n } from 'vue-i18n';

import * as THREE from 'three';

import type { ConvertOptions } from '@/interfaces';

import { useAvatarAnimation } from '@/composables/useAvatarAnimation';
import { resolveMotionPath, type AvatarGender } from '@/composables/useVrmFile';
import { useVrmPreviewScene } from '@/composables/useVrmPreviewScene';
import {
  playbackIcon,
  formatPreviewMotionTitle,
  PREVIEW_MOTION_CATEGORY_OPTIONS,
  type PreviewMotionPlayback,
  type PreviewMotionCategory,
  PREVIEW_CUSTOM_MOTION_OPTIONS
} from '@/constants/previewAnimations';

const props = defineProps<{
  filePath: string;
  options: ConvertOptions;
}>();

const { t } = useI18n();

const canvasHost = shallowRef<HTMLDivElement | null>(null);
const animationEnabled = ref(true);
type MotionSelectionCategory = PreviewMotionCategory | 'system';
type MotionItem = {
  title: string;
  value: string;
  playback: PreviewMotionPlayback;
};

const selectedMotionMode = ref<'idle' | 'walk' | 'custom'>('idle');
const selectedMotionCategory = ref<MotionSelectionCategory>('system');
const selectedMotionValue = ref<string>('idle');
const avatarGender = ref<AvatarGender>('male');
const currentMotionPath = ref('/animations/avatar_stand_1.bvh');
// shallowRef を使用して Three.js オブジェクトが Vue のディープリアクティブ Proxy でラップされるのを防ぐ。
// ref() は内部プロパティを Proxy 化するため scene.add() や行列演算が壊れる。
const modelRoot = shallowRef<THREE.Object3D | null>(null);

const SYSTEM_MOTION_ITEMS = computed<MotionItem[]>(() => [
  { title: t('motion_idle'), value: 'idle', playback: 'loop' },
  { title: t('motion_walk'), value: 'walk', playback: 'loop' }
]);

const FILTERED_MOTION_ITEMS = computed(() => {
  if (selectedMotionCategory.value === 'system') {
    return SYSTEM_MOTION_ITEMS.value;
  }

  const items = PREVIEW_CUSTOM_MOTION_OPTIONS.filter(
    option => option.category === selectedMotionCategory.value
  );
  return items.length > 0 ? items : PREVIEW_CUSTOM_MOTION_OPTIONS;
});

const selectedMotionPlayback = computed<PreviewMotionPlayback>(() => {
  const selected = FILTERED_MOTION_ITEMS.value.find(
    item => item.value === selectedMotionValue.value
  );
  return selected?.playback ?? 'loop';
});

const selectedMotionPlaybackIcon = computed(() => playbackIcon(selectedMotionPlayback.value));

const currentMotionName = computed(() => currentMotionPath.value.split('/').pop() ?? '');
const currentMotionLabel = computed(() => {
  if (selectedMotionMode.value === 'idle') {
    return t('motion_idle');
  }
  if (selectedMotionMode.value === 'walk') {
    return t('motion_walk');
  }
  return formatPreviewMotionTitle(currentMotionPath.value);
});
const motionSummaryText = computed(
  () =>
    `Gender: ${avatarGender.value} / Category: ${selectedMotionCategory.value} / Selected: ${currentMotionLabel.value} / Playback: ${selectedMotionPlayback.value} / File: ${currentMotionName.value}`
);

const applyMotionSelection = () => {
  if (selectedMotionValue.value === 'idle' || selectedMotionValue.value === 'walk') {
    selectedMotionMode.value = selectedMotionValue.value;
    currentMotionPath.value = resolveMotionPath(selectedMotionMode.value, avatarGender.value);
    return;
  }

  selectedMotionMode.value = 'custom';
  currentMotionPath.value = resolveMotionPath(
    'custom',
    avatarGender.value,
    selectedMotionValue.value
  );
};

const { animationStatus, applyOrLoadAnimation, stopIdleAnimation, tickMixer, disposeMixer } =
  useAvatarAnimation({
    modelRoot,
    animationEnabled,
    selectedMotionMode,
    currentMotionPath,
    t
  });

const {
  loading,
  errorMessage,
  scheduleReload,
  loadPreviewModel,
  initScene,
  disposeScene,
  resetCamera
} = useVrmPreviewScene({
  canvasHost,
  modelRoot,
  animationEnabled,
  onBeforeLoad: async (_path: string) => {
    applyMotionSelection();
  },
  applyOrLoadAnimation,
  stopIdleAnimation,
  disposeMixer,
  tickMixer
});

const motionControlsDisabled = computed(() => loading.value || !modelRoot.value);

onMounted(() => {
  if (!canvasHost.value) {
    return;
  }

  applyMotionSelection();
  initScene(canvasHost.value);

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
    scheduleReload(props.filePath, props.options);
  }
);

watch(
  () => animationEnabled.value,
  enabled => {
    if (enabled) {
      applyMotionSelection();
      void applyOrLoadAnimation();
      return;
    }
    stopIdleAnimation();
  }
);

watch(
  () => selectedMotionValue.value,
  () => {
    applyMotionSelection();
    if (animationEnabled.value) {
      void applyOrLoadAnimation();
    }
  }
);

watch(
  () => selectedMotionCategory.value,
  () => {
    const availableValues = FILTERED_MOTION_ITEMS.value.map(item => item.value);
    if (availableValues.includes(selectedMotionValue.value)) {
      return;
    }

    const [firstValue] = availableValues;
    if (!firstValue) {
      return;
    }
    selectedMotionValue.value = firstValue;
  }
);

watch(
  () => avatarGender.value,
  () => {
    applyMotionSelection();
    if (animationEnabled.value) {
      void applyOrLoadAnimation();
    }
  }
);

onBeforeUnmount(() => {
  disposeScene();
});
</script>

<template>
  <v-card :title="t('title')" prepend-icon="mdi-cube-outline">
    <v-card-text>
      <div ref="canvasHost" class="preview-host" />
      <div class="d-flex flex-wrap ga-3 align-center mt-3">
        <v-select
          v-model="selectedMotionCategory"
          :items="PREVIEW_MOTION_CATEGORY_OPTIONS"
          :disabled="motionControlsDisabled"
          item-title="title"
          item-value="value"
          :clearable="false"
          density="compact"
          hide-details
          label="Category"
          style="max-width: 220px"
        />
        <v-select
          v-model="selectedMotionValue"
          :items="FILTERED_MOTION_ITEMS"
          :disabled="motionControlsDisabled"
          item-title="title"
          item-value="value"
          :clearable="false"
          density="compact"
          hide-details
          :label="t('motion_label')"
          style="max-width: 340px"
        >
          <template #item="{ props: itemProps, item }">
            <v-list-item v-bind="itemProps" :prepend-icon="playbackIcon(item.playback)" />
          </template>
          <template #selection>
            <v-icon :icon="selectedMotionPlaybackIcon" class="mr-1" size="small" />
            <span>{{ currentMotionLabel }}</span>
          </template>
        </v-select>
        <v-radio-group
          v-model="avatarGender"
          :disabled="motionControlsDisabled"
          :inline="true"
          hide-details
          density="compact"
        >
          <v-radio value="female" :label="t('gender_female')" />
          <v-radio value="male" :label="t('gender_male')" />
        </v-radio-group>
        <v-switch
          v-model="animationEnabled"
          :disabled="motionControlsDisabled"
          color="primary"
          density="compact"
          hide-details
          :label="t('motion_play_label', { file: currentMotionName })"
        />
        <v-btn
          variant="tonal"
          density="comfortable"
          color="secondary"
          prepend-icon="mdi-camera-retake-outline"
          @click="resetCamera"
        >
          {{ t('reset_camera') }}
        </v-btn>
      </div>
      <div class="text-caption text-medium-emphasis mt-1">
        {{ motionSummaryText }}
      </div>
      <v-alert v-if="loading" type="info" class="mt-2" variant="tonal">{{ t('loading') }}</v-alert>
      <v-alert v-else-if="errorMessage" type="error" class="mt-2" variant="tonal">
        {{ errorMessage }}
      </v-alert>
      <v-alert v-else-if="!filePath" type="info" class="mt-2" variant="tonal">
        {{ t('no_file_selected') }}
      </v-alert>
      <v-alert v-else type="info" class="mt-2" variant="tonal">
        {{ animationStatus }}
      </v-alert>
    </v-card-text>
  </v-card>
</template>

<i18n lang="yaml">
en:
  title: VRM Preview
  motion_label: Motion
  motion_play_label: 'Play motion ({file})'
  reset_camera: Reset Camera
  gender_motion_info: 'Gender: {gender} / Auto-selected: {file}'
  loading: Loading...
  no_file_selected: Select a VRM file to display the preview here.
  motion_idle: Idle
  motion_walk: Walk
  gender_female: Female
  gender_male: Male
  status_disabled: Motion is disabled.
  status_waiting: Waiting for motion to load.
  status_no_skinned_mesh: No skinned mesh found; cannot apply idle motion.
  status_no_matching_bones: No idle motion bone tracks matched.
  status_pose_applied: '{mode} pose applied (1-frame BVH) meshes: {meshes}, tracks: {tracks}'
  status_motion_playing: '{mode} motion playing (rotation only) meshes: {meshes}, tracks: {tracks}'
  status_bvh_loaded: 'Motion loaded: {file} (frames: {frames})'
  status_bvh_failed: 'Motion load failed ({path}): {error}'
  status_stopped: Motion stopped.
fr:
  title: Prévisualisation VRM
  motion_label: Mouvement
  motion_play_label: 'Lire le mouvement ({file})'
  reset_camera: Réinitialiser la caméra
  gender_motion_info: 'Genre: {gender} / Sélection auto: {file}'
  loading: Chargement...
  no_file_selected: Sélectionnez un fichier VRM pour afficher la prévisualisation ici.
  motion_idle: Inactif
  motion_walk: Marche
  gender_female: Femme
  gender_male: Homme
  status_disabled: Le mouvement est désactivé.
  status_waiting: En attente du chargement du mouvement.
  status_no_skinned_mesh: Aucun maillage skinné trouvé ; impossible d'appliquer le mouvement inactif.
  status_no_matching_bones: Aucune piste d'os correspondante pour le mouvement inactif.
  status_pose_applied: 'Pose {mode} appliquée (BVH 1 frame) maillages: {meshes}, pistes: {tracks}'
  status_motion_playing: 'Mouvement {mode} en lecture (rotation seule) maillages: {meshes}, pistes: {tracks}'
  status_bvh_loaded: 'Mouvement chargé: {file} (frames: {frames})'
  status_bvh_failed: 'Échec du chargement ({path}): {error}'
  status_stopped: Mouvement arrêté.
ja:
  title: VRMプレビュー
  motion_label: モーション
  motion_play_label: 'モーション再生 ({file})'
  reset_camera: カメラリセット
  gender_motion_info: '性別判定: {gender} / 自動選択: {file}'
  loading: 読み込み中...
  no_file_selected: VRMファイルを選択するとここに表示されます。
  motion_idle: 待機
  motion_walk: 歩行
  gender_female: 女性
  gender_male: 男性
  status_disabled: モーションは無効です。
  status_waiting: モーション読み込み待ちです。
  status_no_skinned_mesh: スキン付きメッシュが見つからず、待機モーションを適用できません。
  status_no_matching_bones: 待機モーションのボーントラックが一致しませんでした。
  status_pose_applied: '{mode}ポーズ適用中（1フレームBVH） meshes: {meshes}, tracks: {tracks}'
  status_motion_playing: '{mode}モーション再生中（回転のみ） meshes: {meshes}, tracks: {tracks}'
  status_bvh_loaded: 'モーション読込済み: {file} (frames: {frames})'
  status_bvh_failed: 'モーション読込失敗 ({path}): {error}'
  status_stopped: モーション停止中。
ko:
  title: VRM 미리보기
  motion_label: 모션
  motion_play_label: '모션 재생 ({file})'
  reset_camera: 카메라 리셋
  gender_motion_info: '성별 판정: {gender} / 자동 선택: {file}'
  loading: 불러오는 중...
  no_file_selected: VRM 파일을 선택하면 여기에 표시됩니다.
  motion_idle: 대기
  motion_walk: 걷기
  gender_female: 여성
  gender_male: 남성
  status_disabled: 모션이 비활성화되어 있습니다.
  status_waiting: 모션 로드를 기다리는 중입니다.
  status_no_skinned_mesh: 스킨 메시를 찾을 수 없어 대기 모션을 적용할 수 없습니다.
  status_no_matching_bones: 대기 모션의 본 트랙이 일치하지 않았습니다.
  status_pose_applied: '{mode} 포즈 적용 중（1프레임 BVH） meshes: {meshes}, tracks: {tracks}'
  status_motion_playing: '{mode} 모션 재생 중（회전만） meshes: {meshes}, tracks: {tracks}'
  status_bvh_loaded: '모션 로드 완료: {file} (frames: {frames})'
  status_bvh_failed: '모션 로드 실패 ({path}): {error}'
  status_stopped: 모션 정지 중。
zhHant:
  title: VRM 預覽
  motion_label: 動作
  motion_play_label: '播放動作 ({file})'
  reset_camera: 重設鏡頭
  gender_motion_info: '性別判定: {gender} / 自動選擇: {file}'
  loading: 載入中...
  no_file_selected: 選擇 VRM 檔案後將在此顯示預覽。
  motion_idle: 待機
  motion_walk: 行走
  gender_female: 女性
  gender_male: 男性
  status_disabled: 動作已停用。
  status_waiting: 等待動作載入中。
  status_no_skinned_mesh: 找不到蒙皮網格，無法套用待機動作。
  status_no_matching_bones: 找不到符合的骨骼軌道。
  status_pose_applied: '{mode}姿勢套用中（1幀BVH） meshes: {meshes}, tracks: {tracks}'
  status_motion_playing: '{mode}動作播放中（僅旋轉） meshes: {meshes}, tracks: {tracks}'
  status_bvh_loaded: '動作已載入: {file} (frames: {frames})'
  status_bvh_failed: '動作載入失敗 ({path}): {error}'
  status_stopped: 動作已停止。
zhHans:
  title: VRM 预览
  motion_label: 动作
  motion_play_label: '播放动作 ({file})'
  reset_camera: 重置镜头
  gender_motion_info: '性别判定: {gender} / 自动选择: {file}'
  loading: 加载中...
  no_file_selected: 选择 VRM 文件后将在此处显示预览。
  motion_idle: 待机
  motion_walk: 行走
  status_disabled: 动作已禁用。
  status_waiting: 等待动作加载中。
  status_no_skinned_mesh: 未找到蒙皮网格，无法应用待机动作。
  status_no_matching_bones: 未找到匹配的骨骼轨道。
  status_pose_applied: '{mode}姿势应用中（1帧BVH） meshes: {meshes}, tracks: {tracks}'
  status_motion_playing: '{mode}动作播放中（仅旋转） meshes: {meshes}, tracks: {tracks}'
  status_bvh_loaded: '动作已加载: {file} (frames: {frames})'
  status_bvh_failed: '动作加载失败 ({path}): {error}'
  status_stopped: 动作已停止。
</i18n>

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
