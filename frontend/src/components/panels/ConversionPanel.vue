<script setup lang="ts">
import { useConfigStore, useGlobalStore } from '@/store';
import { computed, onMounted, ref, toRef, watch } from 'vue';
import { useI18n } from 'vue-i18n';

import { invoke } from '@tauri-apps/api/core';

import type { AnalysisReport, ConversionReport, ConvertOptions } from '@/interfaces';

import VrmPreview from '@/components/VrmPreview.vue';
import MessageDialog from '@/components/modals/MessageDialog.vue';
import { useFileSystem } from '@/composables/useFileSystem';
import { useNotification } from '@/composables/useNotification';
import { useVrmFileDrop } from '@/composables/useVrmFileDrop';
import { ValidationSeverity } from '@/types';

const props = defineProps<{
  analysis: AnalysisReport | null;
  conversion: ConversionReport | null;
}>();

const emit = defineEmits<{
  'update:analysis': [value: AnalysisReport | null];
  'update:conversion': [value: ConversionReport | null];
}>();

const { t } = useI18n();
const globalStore = useGlobalStore();
const configStore = useConfigStore();
const targetHeightCm = toRef(configStore, 'targetHeightCm');
const manualScale = toRef(configStore, 'manualScale');
const textureAutoResize = toRef(configStore, 'textureAutoResize');
const pbrEnabled = toRef(configStore, 'pbrEnabled');
const inputPath = toRef(configStore, 'inputPath');
const outputPath = toRef(configStore, 'outputPath');
const textureResizeMethod = toRef(configStore, 'textureResizeMethod');
const face = toRef(configStore, 'face');
const fingers = toRef(configStore, 'fingers');

const notification = useNotification(t);
const fs = useFileSystem();

const appVersion = ref('');
const convertResultPath = ref('');
const showVersionErrorDialog = ref(false);
const previewLoading = ref(false);
const operationLoading = ref(false);

const { isDropActive, showDropWarningDialog, dropWarningTitleKey, dropWarningMessageKey } =
  useVrmFileDrop(inputPath);

const options = ref<ConvertOptions>({
  target_height_cm: targetHeightCm.value,
  manual_scale: manualScale.value,
  texture_auto_resize: textureAutoResize.value,
  texture_resize_method: textureResizeMethod.value,
  pbr_enabled: pbrEnabled.value
});

watch([targetHeightCm, manualScale, textureAutoResize, pbrEnabled, textureResizeMethod], () => {
  options.value.target_height_cm = targetHeightCm.value;
  options.value.manual_scale = manualScale.value;
  options.value.texture_auto_resize = textureAutoResize.value;
  options.value.texture_resize_method = textureResizeMethod.value;
  options.value.pbr_enabled = pbrEnabled.value;
});

watch(inputPath, (newPath, oldPath) => {
  // Clear output when switching to a different input VRM to avoid accidental overwrite.
  if (newPath && oldPath && newPath !== oldPath) {
    outputPath.value = '';
  }
});

const hasBlockingIssue = computed(
  () => props.analysis?.issues.some(issue => issue.severity === ValidationSeverity.Error) ?? false
);

/** エクスポートボタンの無効条件: 入出力パス不足 or 解析エラーあり */
const exportDisabled = computed(
  () => !inputPath.value || !outputPath.value || previewLoading.value || hasBlockingIssue.value
);

const uiLocked = computed(() => previewLoading.value || operationLoading.value);

watch(
  uiLocked,
  locked => {
    globalStore.setLoading(locked);
  },
  { immediate: true }
);

const outputMaxTextureDimension = computed(() => {
  if (!props.conversion || props.conversion.output_texture_infos.length === 0) {
    return 0;
  }
  return props.conversion.output_texture_infos.reduce((max, texture) => {
    return Math.max(max, texture.width, texture.height);
  }, 0);
});

const pickInputFile = async () => {
  const selected = await fs.selectFiles({
    multiple: false,
    filters: [{ name: 'VRM', extensions: ['vrm'] }]
  });
  if (typeof selected === 'string') {
    inputPath.value = selected;
  }
};

const pickOutputFile = async () => {
  // 入力ファイル名から拡張子を除去し.glbを付与
  let defaultName = 'output.glb';
  if (inputPath.value) {
    const inputFile = inputPath.value.split(/[\\/]/).pop() || '';
    const base = inputFile.replace(/\.[^/.]+$/, '');
    defaultName = base + '.glb';
  }
  const selected = await fs.saveFile({
    defaultPath: outputPath.value || defaultName,
    filters: [{ name: 'glTF Binary', extensions: ['glb'] }]
  });
  if (selected) {
    outputPath.value = selected;
  }
};

const runAnalyze = async () => {
  if (!inputPath.value) {
    notification.error(t('error_no_input'));
    return;
  }
  operationLoading.value = true;
  try {
    emit('update:conversion', null);
    const result = await invoke<AnalysisReport>('analyze_vrm_command', {
      request: {
        input_path: inputPath.value,
        options: options.value,
        notify_on_complete: true
      }
    });
    emit('update:analysis', result);
    if (result.issues.some(issue => issue.code === 'UNSUPPORTED_VRM_VERSION')) {
      showVersionErrorDialog.value = true;
    } else {
      notification.success(t('success_analyze'));
    }
  } catch (error) {
    notification.error(String(error));
  } finally {
    operationLoading.value = false;
  }
};

const runExport = async () => {
  if (!inputPath.value || !outputPath.value) {
    notification.error(t('error_no_paths'));
    return;
  }
  operationLoading.value = true;
  try {
    const result = await invoke<ConversionReport>('convert_vrm_command', {
      request: {
        input_path: inputPath.value,
        output_path: outputPath.value,
        options: options.value,
        notify_on_complete: true
      }
    });
    emit('update:conversion', result);
    convertResultPath.value = outputPath.value;
    notification.success(t('success_convert', { scale: result.computed_scale_factor.toFixed(4) }));
  } catch (error) {
    notification.error(String(error));
  } finally {
    operationLoading.value = false;
  }
};

onMounted(async () => {
  try {
    appVersion.value = await invoke<string>('get_app_version');
  } catch (error) {
    console.error('Failed to get version:', error);
  }
});
</script>

<template>
  <message-dialog
    v-model="showVersionErrorDialog"
    :title="t('vrm0_error_title')"
    :message="t('vrm0_error_message')"
  />
  <message-dialog
    v-model="showDropWarningDialog"
    :title="t(dropWarningTitleKey)"
    :message="t(dropWarningMessageKey)"
  />
  <v-row>
    <v-col cols="12" lg="6">
      <v-card
        :class="{ 'drop-active': isDropActive }"
        :subtitle="t('backend_version', { version: appVersion })"
        :title="t('title')"
        prepend-icon="mdi-account-convert"
      >
        <v-card-text>
          <v-text-field
            v-model="inputPath"
            :label="t('input_vrm')"
            append-inner-icon="mdi-folder-open"
            prepend-icon="mdi-import"
            clearable
            @click:append-inner="pickInputFile"
          >
            <template #append>
              <v-btn
                :text="t('btn_analyze')"
                :disabled="!inputPath"
                color="primary"
                prepend-icon="mdi-magnify"
                @click="runAnalyze"
              />
            </template>
          </v-text-field>
          <v-text-field
            v-model="outputPath"
            :label="t('output_glb')"
            append-inner-icon="mdi-content-save"
            prepend-icon="mdi-export"
            @click:append-inner="pickOutputFile"
          >
            <template #append>
              <v-btn
                :disabled="exportDisabled"
                :text="t('btn_export')"
                color="success"
                prepend-icon="mdi-file-export"
                @click="runExport"
              />
            </template>
          </v-text-field>

          <v-divider class="my-4" />

          <v-row>
            <v-col cols="12" md="4">
              <v-switch v-model="face.blink.enabled" :label="t('blink_enabled')" />
              <v-slider
                v-model="face.blink.interval_sec"
                :label="t('blink_interval')"
                max="10"
                min="1"
                step="0.1"
              />
            </v-col>
            <v-col cols="12" md="4">
              <v-switch v-model="face.lip_sync.enabled" :label="t('lip_sync_enabled')" />
              <v-slider
                v-model="face.lip_sync.open_angle"
                :label="t('lip_open_angle')"
                max="1"
                min="0"
                step="0.01"
              />
            </v-col>
            <v-col cols="12" md="4">
              <v-switch v-model="fingers.enabled" :label="t('fingers_enabled')" />
              <v-select
                v-model="fingers.test_pose"
                :items="['open', 'fist']"
                :label="t('fingers_test_pose')"
              />
            </v-col>
          </v-row>

          <v-alert v-if="convertResultPath" type="success" class="mt-2" variant="tonal">
            {{ t('converted', { path: convertResultPath }) }}
          </v-alert>
          <v-alert v-if="conversion" type="info" class="mt-2" variant="tonal">
            {{
              t('output_tex_max_over', {
                max: outputMaxTextureDimension,
                over: conversion.output_texture_over_1024_count
              })
            }}
            <div class="text-caption mt-1">
              {{ t('current_setting') }}
              {{
                options.texture_auto_resize
                  ? t('resize_policy_aggressive')
                  : t('resize_policy_conservative')
              }}
            </div>
          </v-alert>
        </v-card-text>
      </v-card>
    </v-col>

    <v-col cols="12" lg="6">
      <vrm-preview
        :file-path="inputPath"
        :options="options"
        @update:loading="previewLoading = $event"
      />
    </v-col>
  </v-row>
</template>

<i18n lang="yaml">
en:
  title: VRM → Second Life glTF Conversion
  backend_version: 'Backend Version: {version}'
  input_vrm: Input VRM
  btn_analyze: Analyze
  output_glb: Output .glb
  btn_export: Export
  blink_enabled: Blink ON
  blink_interval: Blink Interval (sec)
  lip_sync_enabled: Lip Sync ON
  lip_open_angle: Mouth Open Angle
  fingers_enabled: Finger Check ON
  fingers_test_pose: Finger Test Pose
  converted: 'Converted: {path}'
  output_tex_max_over: 'Post-conv. texture max side: {max}px / Over 1024px (post): {over}'
  current_setting: 'Current setting:'
  resize_policy_aggressive: Resize >=1025px to 1024px (incl. >=2049px)
  resize_policy_conservative: Resize only >=2049px to 2048px (keep 1025-2048px)
  error_no_input: Please select a VRM file.
  error_no_paths: Please set the input VRM and output destination.
  success_analyze: Analysis complete.
  success_convert: 'Conversion complete (scale={scale})'
  vrm0_error_title: Unsupported VRM Version
  vrm0_error_message: Only VRM 1.0 is supported. Please convert your file to VRM 1.0 before importing.
  drop_unsupported_ext_title: Unsupported File Type
  drop_unsupported_ext_message: Only .vrm files can be dropped.
  drop_invalid_title: Unsupported VRM File
  drop_invalid_message: The dropped file is not a supported VRM 1.0 file.
fr:
  title: Conversion VRM -> Second Life glTF
  backend_version: 'Version du backend: {version}'
  input_vrm: "VRM d'entrée"
  btn_analyze: Analyser
  output_glb: Sortie .glb
  btn_export: Exporter
  blink_enabled: Clignement activé
  blink_interval: Intervalle de clignement (s)
  lip_sync_enabled: Sync labiale activée
  lip_open_angle: "Angle d'ouverture buccale"
  fingers_enabled: Vérification des doigts activée
  fingers_test_pose: Pose de test des doigts
  converted: 'Converti: {path}'
  output_tex_max_over: 'Côté max texture (post-conv.): {max}px / >1024px (post): {over}'
  current_setting: 'Paramètre actuel:'
  resize_policy_aggressive: Réduire >=1025px à 1024px (incl. >=2049px)
  resize_policy_conservative: Réduire uniquement >=2049px à 2048px (garder 1025-2048px)
  error_no_input: Veuillez sélectionner un fichier VRM.
  error_no_paths: "Veuillez définir le VRM d'entrée et la destination de sortie."
  success_analyze: Analyse terminée.
  success_convert: 'Conversion terminée (scale={scale})'
  vrm0_error_title: Version VRM non prise en charge
  vrm0_error_message: "Seule la version VRM 1.0 est prise en charge. Veuillez convertir votre fichier en VRM 1.0 avant de l'importer."
  drop_unsupported_ext_title: Type de fichier non pris en charge
  drop_unsupported_ext_message: Seuls les fichiers .vrm peuvent être déposés.
  drop_invalid_title: Fichier VRM non pris en charge
  drop_invalid_message: Le fichier déposé n'est pas un fichier VRM 1.0 pris en charge.
ja:
  title: VRM → Second Life glTF 変換
  backend_version: 'バックエンドバージョン: {version}'
  input_vrm: 入力VRM
  btn_analyze: 解析
  output_glb: 出力 .glb
  btn_export: エクスポート
  blink_enabled: 瞬きON
  blink_interval: 瞬き間隔(秒)
  lip_sync_enabled: クチパクON
  lip_open_angle: 開口角度
  fingers_enabled: 指確認ON
  fingers_test_pose: 指テストポーズ
  converted: '変換済み: {path}'
  output_tex_max_over: '変換後テクスチャ最大辺: {max}px / 1024px超過(変換後): {over}'
  current_setting: '現在の設定:'
  resize_policy_aggressive: 1025px以上を1024pxへ縮小（2049px以上も1024px）
  resize_policy_conservative: 2049px以上のみ2048pxへ縮小（1025〜2048pxは維持）
  error_no_input: VRMファイルを選択してください。
  error_no_paths: 入力VRMと出力先を設定してください。
  success_analyze: 解析が完了しました。
  success_convert: '変換完了 (scale={scale})'
  vrm0_error_title: サポートされていないVRMバージョン
  vrm0_error_message: VRM 1.0のみ使用可能です。VRM 1.0に変換してからインポートしてください。
  drop_unsupported_ext_title: 非対応のファイル形式
  drop_unsupported_ext_message: ドロップできるのは .vrm ファイルのみです。
  drop_invalid_title: 非対応のVRMファイル
  drop_invalid_message: ドロップされたファイルは対応している VRM 1.0 ではありません。
ko:
  title: VRM → Second Life glTF 변환
  backend_version: '백엔드 버전: {version}'
  input_vrm: 입력 VRM
  btn_analyze: 분석
  output_glb: 출력 .glb
  btn_export: 내보내기
  blink_enabled: 눈 깜빡임 ON
  blink_interval: 깜빡임 간격(초)
  lip_sync_enabled: 립싱크 ON
  lip_open_angle: 입 개방 각도
  fingers_enabled: 손가락 확인 ON
  fingers_test_pose: 손가락 테스트 포즈
  converted: '변환 완료: {path}'
  output_tex_max_over: '변환 후 텍스처 최대 변: {max}px / 1024px 초과(변환 후): {over}'
  current_setting: '현재 설정:'
  resize_policy_aggressive: 1025px 이상을 1024px로 축소（2049px 이상 포함）
  resize_policy_conservative: 2049px 이상만 2048px로 축소（1025〜2048px 유지）
  error_no_input: VRM 파일을 선택해 주세요.
  error_no_paths: 입력 VRM과 출력 대상을 설정해 주세요.
  success_analyze: 분석이 완료되었습니다.
  success_convert: '변환 완료 (scale={scale})'
  vrm0_error_title: 지원되지 않는 VRM 버전
  vrm0_error_message: VRM 1.0만 사용 가능합니다. 파일을 VRM 1.0으로 변환한 후 가져오세요.
  drop_unsupported_ext_title: 지원되지 않는 파일 형식
  drop_unsupported_ext_message: .vrm 파일만 드롭할 수 있습니다.
  drop_invalid_title: 지원되지 않는 VRM 파일
  drop_invalid_message: 드롭한 파일은 지원되는 VRM 1.0 파일이 아닙니다.
zhHant:
  title: VRM → Second Life glTF 轉換
  backend_version: '後端版本: {version}'
  input_vrm: 輸入 VRM
  btn_analyze: 分析
  output_glb: 輸出 .glb
  btn_export: 匯出
  blink_enabled: 眨眼 ON
  blink_interval: 眨眼間隔(秒)
  lip_sync_enabled: 口型同步 ON
  lip_open_angle: 張口角度
  fingers_enabled: 手指確認 ON
  fingers_test_pose: 手指測試姿勢
  converted: '已轉換: {path}'
  output_tex_max_over: '轉換後貼圖最大邊: {max}px / 超過1024px(轉換後): {over}'
  current_setting: '目前設定:'
  resize_policy_aggressive: 將 >=1025px 縮小至 1024px（含 >=2049px）
  resize_policy_conservative: 僅將 >=2049px 縮小至 2048px（保留 1025〜2048px）
  error_no_input: 請選擇 VRM 檔案。
  error_no_paths: 請設定輸入 VRM 和輸出目的地。
  success_analyze: 分析完成。
  success_convert: '轉換完成 (scale={scale})'
  vrm0_error_title: 不支援的 VRM 版本
  vrm0_error_message: 僅支援 VRM 1.0。請先將您的檔案轉換為 VRM 1.0 再匯入。
  drop_unsupported_ext_title: 不支援的檔案類型
  drop_unsupported_ext_message: 只能拖放 .vrm 檔案。
  drop_invalid_title: 不支援的 VRM 檔案
  drop_invalid_message: 拖放的檔案不是受支援的 VRM 1.0 檔案。
zhHans:
  title: VRM → Second Life glTF 转换
  backend_version: '后端版本: {version}'
  input_vrm: 输入 VRM
  btn_analyze: 分析
  output_glb: 输出 .glb
  btn_export: 导出
  blink_enabled: 眨眼 ON
  blink_interval: 眨眼间隔(秒)
  lip_sync_enabled: 口型同步 ON
  lip_open_angle: 张口角度
  fingers_enabled: 手指确认 ON
  fingers_test_pose: 手指测试姿势
  converted: '已转换: {path}'
  output_tex_max_over: '转换后贴图最大边: {max}px / 超过1024px(转换后): {over}'
  current_setting: '当前设置:'
  resize_policy_aggressive: 将 >=1025px 缩小至 1024px（含 >=2049px）
  resize_policy_conservative: 仅将 >=2049px 缩小至 2048px（保留 1025〜2048px）
  error_no_input: 请选择 VRM 文件。
  error_no_paths: 请设置输入 VRM 和输出目标。
  success_analyze: 分析完成。
  success_convert: '转换完成 (scale={scale})'
  vrm0_error_title: 不支持的 VRM 版本
  vrm0_error_message: 仅支持 VRM 1.0。请先将您的文件转换为 VRM 1.0 再导入。
  drop_unsupported_ext_title: 不支持的文件类型
  drop_unsupported_ext_message: 仅支持拖放 .vrm 文件。
  drop_invalid_title: 不支持的 VRM 文件
  drop_invalid_message: 拖放的文件不是受支持的 VRM 1.0 文件。
</i18n>

<style scoped>
.v-card-title {
  line-height: 1.3;
}

.drop-active {
  background-color: rgba(var(--v-theme-success), 0.12);
  transition: background-color 0.15s ease;
}
</style>
