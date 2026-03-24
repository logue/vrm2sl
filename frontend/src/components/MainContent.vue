<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import VrmPreview from '@/components/VrmPreview.vue';
import { useFileSystem } from '@/composables/useFileSystem';
import { useNotification } from '@/composables/useNotification';
import { ValidationSeverity } from '@/types';
import type {
  AnalysisReport,
  ConversionReport,
  ConvertOptions,
  ProjectSettings
} from '@/interfaces';
import { useGlobalStore } from '@/store';

const { t } = useI18n();

const globalStore = useGlobalStore();
const notification = useNotification(t);
const fs = useFileSystem();

const inputPath = ref('');
const outputPath = ref('');
const settingsPath = ref('project-settings.json');

const options = ref<ConvertOptions>({
  target_height_cm: 200,
  manual_scale: 1,
  texture_auto_resize: true,
  texture_resize_method: 'Bilinear',
  pbr_enabled: true
});

const face = ref<ProjectSettings['face']>({
  blink: { enabled: true, interval_sec: 4, close_duration_sec: 0.15, wink_enabled: true },
  lip_sync: { enabled: false, mode: 'chat', open_angle: 0.5, speed: 0.5 },
  eye_tracking: {
    camera_follow: true,
    random_look: true,
    vertical_range_deg: 25,
    horizontal_range_deg: 40,
    speed: 0.5
  }
});

const fingers = ref<ProjectSettings['fingers']>({ enabled: true, test_pose: 'open' });

const analysis = ref<AnalysisReport | null>(null);
const conversion = ref<ConversionReport | null>(null);
const appVersion = ref('');
const convertResultPath = ref('');
const logs = ref<{ level: string; message: string; timestamp: string }[]>([]);

let unlistenLogMessage: UnlistenFn | null = null;

const outputMaxTextureDimension = computed(() => {
  if (!conversion.value || conversion.value.output_texture_infos.length === 0) {
    return 0;
  }
  return conversion.value.output_texture_infos.reduce((max, texture) => {
    return Math.max(max, texture.width, texture.height);
  }, 0);
});

const outputTextureSizePreview = computed(() => {
  if (!conversion.value) {
    return '';
  }
  return conversion.value.output_texture_infos
    .slice(0, 5)
    .map(texture => `#${texture.index}: ${texture.width}x${texture.height}`)
    .join(', ');
});

const resizedTextureCount = computed(() => {
  if (!conversion.value) {
    return 0;
  }
  return Math.max(
    0,
    conversion.value.texture_over_1024_count - conversion.value.output_texture_over_1024_count
  );
});

const hasBlockingIssue = computed(
  () => analysis.value?.issues.some(issue => issue.severity === ValidationSeverity.Error) ?? false
);

const toProjectSettings = (): ProjectSettings => ({
  input_path: inputPath.value || undefined,
  output_path: outputPath.value || undefined,
  target_height_cm: options.value.target_height_cm,
  manual_scale: options.value.manual_scale,
  texture_auto_resize: options.value.texture_auto_resize,
  texture_resize_method: options.value.texture_resize_method,
  pbr_enabled: options.value.pbr_enabled,
  face: face.value,
  fingers: fingers.value
});

const applyProjectSettings = (settings: ProjectSettings) => {
  inputPath.value = settings.input_path ?? '';
  outputPath.value = settings.output_path ?? '';
  options.value.target_height_cm = settings.target_height_cm;
  options.value.manual_scale = settings.manual_scale;
  options.value.texture_auto_resize = settings.texture_auto_resize;
  options.value.texture_resize_method = settings.texture_resize_method;
  options.value.pbr_enabled = settings.pbr_enabled ?? true;
  face.value = settings.face;
  fingers.value = settings.fingers;
};

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
  const selected = await fs.saveFile({
    defaultPath: outputPath.value || 'output.glb',
    filters: [{ name: 'GLB', extensions: ['glb'] }]
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

  globalStore.setLoading(true);
  try {
    conversion.value = null;
    analysis.value = await invoke<AnalysisReport>('analyze_vrm_command', {
      request: {
        input_path: inputPath.value,
        options: options.value,
        notify_on_complete: true
      }
    });
    notification.success(t('success_analyze'));
  } catch (error) {
    notification.error(String(error));
  } finally {
    globalStore.setLoading(false);
  }
};

const runExport = async () => {
  if (!inputPath.value || !outputPath.value) {
    notification.error(t('error_no_paths'));
    return;
  }

  globalStore.setLoading(true);
  try {
    const result = await invoke<ConversionReport>('convert_vrm_command', {
      request: {
        input_path: inputPath.value,
        output_path: outputPath.value,
        options: options.value,
        notify_on_complete: true
      }
    });
    conversion.value = result;
    convertResultPath.value = outputPath.value;
    notification.success(t('success_convert', { scale: result.computed_scale_factor.toFixed(4) }));
  } catch (error) {
    notification.error(String(error));
  } finally {
    globalStore.setLoading(false);
  }
};

const saveSettings = async () => {
  globalStore.setLoading(true);
  try {
    await invoke('save_project_settings_command', {
      request: {
        path: settingsPath.value,
        settings: toProjectSettings()
      }
    });
    notification.success(t('success_save_settings'));
  } catch (error) {
    notification.error(String(error));
  } finally {
    globalStore.setLoading(false);
  }
};

const loadSettings = async () => {
  globalStore.setLoading(true);
  try {
    const settings = await invoke<ProjectSettings>('load_project_settings_command', {
      request: { path: settingsPath.value }
    });
    applyProjectSettings(settings);
    notification.success(t('success_load_settings'));
  } catch (error) {
    notification.error(String(error));
  } finally {
    globalStore.setLoading(false);
  }
};

const getVersion = async () => {
  try {
    appVersion.value = await invoke<string>('get_app_version');
  } catch (error) {
    console.error('Failed to get version:', error);
  }
};

onMounted(async () => {
  await getVersion();
  unlistenLogMessage = await listen<{ level: string; message: string; timestamp: string }>(
    'log-message',
    event => {
      const payload = event.payload;
      logs.value.push({
        level: payload.level,
        message: payload.message,
        timestamp: payload.timestamp
      });

      if (logs.value.length > 200) {
        logs.value.splice(0, logs.value.length - 200);
      }
    }
  );
});

onBeforeUnmount(() => {
  if (unlistenLogMessage) {
    unlistenLogMessage();
    unlistenLogMessage = null;
  }
});
</script>

<template>
  <v-container fluid>
    <v-row align="start">
      <v-col cols="12" lg="6">
        <v-card>
          <v-card-title class="text-h5">
            <v-icon icon="mdi-account-convert" class="mr-2" />
            {{ t('title') }}
          </v-card-title>

          <v-card-subtitle v-if="appVersion">
            {{ t('backend_version', { version: appVersion }) }}
          </v-card-subtitle>

          <v-card-text>
            <v-row>
              <v-col cols="12" md="6">
                <v-text-field
                  v-model="inputPath"
                  :label="t('input_vrm')"
                  variant="outlined"
                  density="comfortable"
                />
              </v-col>
              <v-col cols="12" md="6" class="d-flex ga-2 align-center">
                <v-btn prepend-icon="mdi-folder-open" @click="pickInputFile">
                  {{ t('btn_select_vrm') }}
                </v-btn>
                <v-btn color="primary" prepend-icon="mdi-magnify" @click="runAnalyze">
                  {{ t('btn_analyze') }}
                </v-btn>
              </v-col>
            </v-row>

            <v-row>
              <v-col cols="12" md="6">
                <v-text-field
                  v-model="outputPath"
                  :label="t('output_glb')"
                  variant="outlined"
                  density="comfortable"
                />
              </v-col>
              <v-col cols="12" md="6" class="d-flex ga-2 align-center">
                <v-btn prepend-icon="mdi-content-save" @click="pickOutputFile">
                  {{ t('btn_select_output') }}
                </v-btn>
                <v-btn
                  color="success"
                  prepend-icon="mdi-file-export"
                  :disabled="hasBlockingIssue"
                  @click="runExport"
                >
                  {{ t('btn_export') }}
                </v-btn>
              </v-col>
            </v-row>

            <v-row>
              <v-col cols="12" md="4">
                <v-text-field
                  v-model.number="options.target_height_cm"
                  type="number"
                  :label="t('target_height')"
                  variant="outlined"
                />
              </v-col>
              <v-col cols="12" md="4">
                <v-slider
                  v-model="options.manual_scale"
                  min="0.5"
                  max="1.5"
                  step="0.01"
                  thumb-label
                  :label="t('manual_scale')"
                />
              </v-col>
              <v-col cols="12" md="4" class="d-flex flex-column align-start">
                <v-switch v-model="options.texture_auto_resize" :label="t('texture_auto_resize')" />
                <div class="text-caption text-medium-emphasis mt-1">
                  {{ t('texture_resize_hint') }}
                </div>
              </v-col>
              <v-col cols="12" md="4" class="d-flex flex-column align-start">
                <v-switch v-model="options.pbr_enabled" :label="t('pbr_enabled')" />
                <div class="text-caption text-medium-emphasis mt-1">
                  {{ t('pbr_hint') }}
                </div>
              </v-col>
            </v-row>

            <v-divider class="my-4" />

            <v-row>
              <v-col cols="12" md="4">
                <v-switch v-model="face.blink.enabled" :label="t('blink_enabled')" />
                <v-slider
                  v-model="face.blink.interval_sec"
                  min="1"
                  max="10"
                  step="0.1"
                  :label="t('blink_interval')"
                />
              </v-col>
              <v-col cols="12" md="4">
                <v-switch v-model="face.lip_sync.enabled" :label="t('lip_sync_enabled')" />
                <v-slider
                  v-model="face.lip_sync.open_angle"
                  min="0"
                  max="1"
                  step="0.01"
                  :label="t('lip_open_angle')"
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

            <v-row>
              <v-col cols="12" md="8">
                <v-text-field
                  v-model="settingsPath"
                  :label="t('settings_path')"
                  variant="outlined"
                  density="comfortable"
                />
              </v-col>
              <v-col cols="12" md="4" class="d-flex ga-2 align-center">
                <v-btn prepend-icon="mdi-content-save" @click="saveSettings">
                  {{ t('btn_save_settings') }}
                </v-btn>
                <v-btn prepend-icon="mdi-folder-open" @click="loadSettings">
                  {{ t('btn_load_settings') }}
                </v-btn>
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
        <vrm-preview :file-path="inputPath" :options="options" />
      </v-col>
    </v-row>

    <v-row class="mt-4">
      <v-col cols="12">
        <v-card>
          <v-card-title>
            <v-icon icon="mdi-text-box-search-outline" class="mr-2" />
            {{ t('logs') }}
          </v-card-title>
          <v-card-text>
            <div class="log-output" role="log" aria-live="polite">
              <template v-if="logs.length > 0">
                <div
                  v-for="(entry, index) in logs"
                  :key="`${entry.timestamp}-${index}`"
                  class="log-line"
                >
                  [{{ entry.timestamp }}] [{{ entry.level.toUpperCase() }}] {{ entry.message }}
                </div>
              </template>
              <div v-else class="text-medium-emphasis">{{ t('no_logs') }}</div>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>

    <v-row class="mt-4">
      <v-col cols="12" md="5">
        <v-card>
          <v-card-title>
            <v-icon icon="mdi-bone" class="mr-2" />
            {{ t('bone_mapping') }}
          </v-card-title>
          <v-card-text>
            <v-list v-if="analysis" density="compact">
              <v-list-item v-for="pair in analysis.mapped_bones" :key="pair[0]">
                <v-list-item-title>{{ pair[0] }} → {{ pair[1] }}</v-list-item-title>
              </v-list-item>
            </v-list>
            <v-alert v-else type="info" variant="tonal">{{ t('analyze_first') }}</v-alert>
          </v-card-text>
        </v-card>
      </v-col>

      <v-col cols="12" md="7">
        <v-card>
          <v-card-title>
            <v-icon icon="mdi-alert-circle" class="mr-2" />
            {{ t('validation') }}
          </v-card-title>
          <v-card-text>
            <v-list v-if="analysis" density="compact">
              <v-list-item>
                <v-list-item-title>
                  {{
                    t('label_model_author', {
                      model: analysis.model_name,
                      author: analysis.author || 'Unknown'
                    })
                  }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item>
                <v-list-item-title>
                  {{
                    t('label_height_mesh_bone', {
                      height: analysis.estimated_height_cm.toFixed(2),
                      mesh: analysis.mesh_count,
                      bone: analysis.bone_count
                    })
                  }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item>
                <v-list-item-title>
                  {{
                    t('label_vertices_polygons', {
                      vertices: analysis.total_vertices,
                      polygons: analysis.total_polygons
                    })
                  }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item>
                <v-list-item-title>
                  {{
                    t('label_tex_cost', {
                      before: analysis.fee_estimate.before_linden_dollar,
                      after: analysis.fee_estimate.after_resize_linden_dollar,
                      reduction: analysis.fee_estimate.reduction_percent
                    })
                  }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item>
                <v-list-item-title>
                  {{ t('label_tex_policy') }}
                  {{
                    options.texture_auto_resize
                      ? t('policy_aggressive_short')
                      : t('policy_conservative_short')
                  }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item v-if="conversion">
                <v-list-item-title>
                  {{
                    t('label_converted_tex', {
                      count: conversion.output_texture_infos.length,
                      max: outputMaxTextureDimension,
                      over: conversion.output_texture_over_1024_count
                    })
                  }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item v-if="conversion">
                <v-list-item-title>
                  {{ t('label_resized_count', { count: resizedTextureCount }) }}
                  <span v-if="outputTextureSizePreview">
                    {{ t('label_size_preview', { preview: outputTextureSizePreview }) }}
                  </span>
                </v-list-item-title>
              </v-list-item>
              <v-list-item v-for="issue in analysis.issues" :key="`${issue.code}-${issue.message}`">
                <v-list-item-title>[{{ issue.severity }}] {{ issue.message }}</v-list-item-title>
              </v-list-item>
            </v-list>
            <v-alert v-else type="info" variant="tonal">{{ t('not_analyzed') }}</v-alert>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<i18n lang="yaml">
en:
  title: VRM → Second Life glTF Conversion
  backend_version: 'Backend Version: {version}'
  input_vrm: Input VRM
  btn_select_vrm: Select VRM
  btn_analyze: Analyze
  output_glb: Output .glb
  btn_select_output: Select Output
  btn_export: Export
  target_height: SL Target Height (cm)
  manual_scale: Manual Scale
  texture_auto_resize: Prefer 1024px Downscale
  texture_resize_hint: 'ON: >=1025px -> 1024px / OFF: only >=2049px -> 2048px'
  pbr_enabled: Enable PBR Materials
  pbr_hint: 'ON: Process metallic/roughness / OFF: Use only simple materials'
  blink_enabled: Blink ON
  blink_interval: Blink Interval (sec)
  lip_sync_enabled: Lip Sync ON
  lip_open_angle: Mouth Open Angle
  fingers_enabled: Finger Check ON
  fingers_test_pose: Finger Test Pose
  settings_path: Settings JSON Path
  btn_save_settings: Save Settings
  btn_load_settings: Load Settings
  converted: 'Converted: {path}'
  output_tex_max_over: 'Post-conv. texture max side: {max}px / Over 1024px (post): {over}'
  current_setting: 'Current setting:'
  resize_policy_aggressive: Resize >=1025px to 1024px (incl. >=2049px)
  resize_policy_conservative: Resize only >=2049px to 2048px (keep 1025-2048px)
  logs: Logs
  no_logs: No logs yet.
  bone_mapping: Bone Mapping
  analyze_first: Run analysis to view.
  validation: Validation
  label_model_author: 'Model: {model} / Author: {author}'
  label_height_mesh_bone: 'Height (est.): {height}cm / Mesh: {mesh} / Bone: {bone}'
  label_vertices_polygons: 'Vertices: {vertices} / Polygons: {polygons}'
  label_tex_cost: 'Texture cost: {before}L$ -> {after}L$ ({reduction}%)'
  label_tex_policy: 'Texture Resize Policy:'
  policy_aggressive_short: '>=1025px -> 1024px (incl. >=2049px)'
  policy_conservative_short: 'only >=2049px -> 2048px (keep 1025-2048px)'
  label_converted_tex: 'Post-conv. textures: {count} / Max side: {max}px / Over 1024px: {over}'
  label_resized_count: 'Resized texture count (est.): {count}'
  label_size_preview: '/ Size preview: {preview}'
  not_analyzed: Not yet analyzed.
  error_no_input: Please select a VRM file.
  error_no_paths: Please set the input VRM and output destination.
  success_analyze: Analysis complete.
  success_convert: 'Conversion complete (scale={scale})'
  success_save_settings: Settings saved.
  success_load_settings: Settings loaded.
fr:
  title: Conversion VRM -> Second Life glTF
  backend_version: 'Version du backend: {version}'
  input_vrm: "VRM d'entrée"
  btn_select_vrm: Sélectionner VRM
  btn_analyze: Analyser
  output_glb: Sortie .glb
  btn_select_output: Sélectionner la sortie
  btn_export: Exporter
  target_height: Taille cible SL (cm)
  manual_scale: Échelle manuelle
  texture_auto_resize: Priorité réduction 1024px
  texture_resize_hint: 'ON: >=1025px -> 1024px / OFF: uniquement >=2049px -> 2048px'
  pbr_enabled: Activer les matériaux PBR
  pbr_hint: 'ON: Traiter metallic/roughness / OFF: Utiliser uniquement des matériaux simples'
  blink_enabled: Clignement activé
  blink_interval: Intervalle de clignement (s)
  lip_sync_enabled: Sync labiale activée
  lip_open_angle: "Angle d'ouverture buccale"
  fingers_enabled: Vérification des doigts activée
  fingers_test_pose: Pose de test des doigts
  settings_path: Chemin JSON des paramètres
  btn_save_settings: Enregistrer
  btn_load_settings: Charger
  converted: 'Converti: {path}'
  output_tex_max_over: 'Côté max texture (post-conv.): {max}px / >1024px (post): {over}'
  current_setting: 'Paramètre actuel:'
  resize_policy_aggressive: Réduire >=1025px à 1024px (incl. >=2049px)
  resize_policy_conservative: Réduire uniquement >=2049px à 2048px (garder 1025-2048px)
  logs: Journaux
  no_logs: "Aucun journal pour l'instant."
  bone_mapping: Mapping des os
  analyze_first: "Lancez l'analyse pour afficher."
  validation: Validation
  label_model_author: 'Modèle: {model} / Auteur: {author}'
  label_height_mesh_bone: 'Hauteur (est.): {height}cm / Maillage: {mesh} / Os: {bone}'
  label_vertices_polygons: 'Sommets: {vertices} / Polygones: {polygons}'
  label_tex_cost: 'Coût texture: {before}L$ -> {after}L$ ({reduction}%)'
  label_tex_policy: 'Politique de redimensionnement:'
  policy_aggressive_short: '>=1025px -> 1024px (incl. >=2049px)'
  policy_conservative_short: 'uniquement >=2049px -> 2048px (garder 1025-2048px)'
  label_converted_tex: 'Textures (post-conv.): {count} / Côté max: {max}px / >1024px: {over}'
  label_resized_count: 'Textures redimensionnées (est.): {count}'
  label_size_preview: '/ Aperçu taille: {preview}'
  not_analyzed: Non analysé.
  error_no_input: Veuillez sélectionner un fichier VRM.
  error_no_paths: "Veuillez définir le VRM d'entrée et la destination de sortie."
  success_analyze: Analyse terminée.
  success_convert: 'Conversion terminée (scale={scale})'
  success_save_settings: Paramètres enregistrés.
  success_load_settings: Paramètres chargés.
ja:
  title: VRM → Second Life glTF 変換
  backend_version: 'バックエンドバージョン: {version}'
  input_vrm: 入力VRM
  btn_select_vrm: VRM選択
  btn_analyze: 解析
  output_glb: 出力 .glb
  btn_select_output: 保存先選択
  btn_export: エクスポート
  target_height: SL目標身長(cm)
  manual_scale: 手動スケール
  texture_auto_resize: 1024px優先縮小
  texture_resize_hint: 'ON: 1025px以上→1024px / OFF: 2049px以上のみ→2048px'
  pbr_enabled: PBRマテリアル有効
  pbr_hint: 'ON: metallic/roughness処理 / OFF: シンプルマテリアルのみ'
  blink_enabled: 瞬きON
  blink_interval: 瞬き間隔(秒)
  lip_sync_enabled: クチパクON
  lip_open_angle: 開口角度
  fingers_enabled: 指確認ON
  fingers_test_pose: 指テストポーズ
  settings_path: 設定JSONパス
  btn_save_settings: 設定保存
  btn_load_settings: 設定読込
  converted: '変換済み: {path}'
  output_tex_max_over: '変換後テクスチャ最大辺: {max}px / 1024px超過(変換後): {over}'
  current_setting: '現在の設定:'
  resize_policy_aggressive: 1025px以上を1024pxへ縮小（2049px以上も1024px）
  resize_policy_conservative: 2049px以上のみ2048pxへ縮小（1025〜2048pxは維持）
  logs: ログ
  no_logs: ログはまだありません。
  bone_mapping: ボーンマッピング
  analyze_first: 解析すると表示されます。
  validation: バリデーション
  label_model_author: 'モデル: {model} / 作者: {author}'
  label_height_mesh_bone: '身長推定: {height}cm / メッシュ: {mesh} / ボーン: {bone}'
  label_vertices_polygons: '頂点: {vertices} / ポリゴン: {polygons}'
  label_tex_cost: 'テクスチャ費用: {before}L$ → {after}L$ ({reduction}%)'
  label_tex_policy: 'テクスチャ縮小ポリシー:'
  policy_aggressive_short: '1025px以上→1024px（2049px以上を含む）'
  policy_conservative_short: '2049px以上のみ→2048px（1025〜2048pxは維持）'
  label_converted_tex: '変換後テクスチャ: {count}枚 / 最大辺: {max}px / 1024px超過: {over}'
  label_resized_count: '縮小適用テクスチャ数(推定): {count}'
  label_size_preview: '/ 縮小後サイズ例: {preview}'
  not_analyzed: 未解析です。
  error_no_input: VRMファイルを選択してください。
  error_no_paths: 入力VRMと出力先を設定してください。
  success_analyze: 解析が完了しました。
  success_convert: '変換完了 (scale={scale})'
  success_save_settings: 設定を保存しました。
  success_load_settings: 設定を読み込みました。
ko:
  title: VRM → Second Life glTF 변환
  backend_version: '백엔드 버전: {version}'
  input_vrm: 입력 VRM
  btn_select_vrm: VRM 선택
  btn_analyze: 분석
  output_glb: 출력 .glb
  btn_select_output: 저장 위치 선택
  btn_export: 내보내기
  target_height: SL 목표 신장(cm)
  manual_scale: 수동 스케일
  texture_auto_resize: 1024px 우선 축소
  texture_resize_hint: 'ON: 1025px 이상→1024px / OFF: 2049px 이상만→2048px'
  pbr_enabled: PBR 재료 활성화
  pbr_hint: 'ON: metallic/roughness 처리 / OFF: 단순 재료만'
  blink_enabled: 눈 깜빡임 ON
  blink_interval: 깜빡임 간격(초)
  lip_sync_enabled: 립싱크 ON
  lip_open_angle: 입 개방 각도
  fingers_enabled: 손가락 확인 ON
  fingers_test_pose: 손가락 테스트 포즈
  settings_path: 설정 JSON 경로
  btn_save_settings: 설정 저장
  btn_load_settings: 설정 불러오기
  converted: '변환 완료: {path}'
  output_tex_max_over: '변환 후 텍스처 최대 변: {max}px / 1024px 초과(변환 후): {over}'
  current_setting: '현재 설정:'
  resize_policy_aggressive: 1025px 이상을 1024px로 축소（2049px 이상 포함）
  resize_policy_conservative: 2049px 이상만 2048px로 축소（1025〜2048px 유지）
  logs: 로그
  no_logs: 아직 로그가 없습니다.
  bone_mapping: 본 매핑
  analyze_first: 분석을 실행하면 표시됩니다.
  validation: 유효성 검사
  label_model_author: '모델: {model} / 작성자: {author}'
  label_height_mesh_bone: '신장 추정: {height}cm / 메시: {mesh} / 본: {bone}'
  label_vertices_polygons: '정점: {vertices} / 폴리곤: {polygons}'
  label_tex_cost: '텍스처 비용: {before}L$ → {after}L$ ({reduction}%)'
  label_tex_policy: '텍스처 축소 정책:'
  policy_aggressive_short: '1025px 이상→1024px（2049px 이상 포함）'
  policy_conservative_short: '2049px 이상만→2048px（1025〜2048px 유지）'
  label_converted_tex: '변환 후 텍스처: {count}장 / 최대 변: {max}px / 1024px 초과: {over}'
  label_resized_count: '축소 적용 텍스처 수(추정): {count}'
  label_size_preview: '/ 축소 후 크기 예시: {preview}'
  not_analyzed: 아직 분석되지 않았습니다.
  error_no_input: VRM 파일을 선택해 주세요.
  error_no_paths: 입력 VRM과 출력 대상을 설정해 주세요.
  success_analyze: 분석이 완료되었습니다.
  success_convert: '변환 완료 (scale={scale})'
  success_save_settings: 설정을 저장했습니다.
  success_load_settings: 설정을 불러왔습니다.
zhHant:
  title: VRM → Second Life glTF 轉換
  backend_version: '後端版本: {version}'
  input_vrm: 輸入 VRM
  btn_select_vrm: 選擇 VRM
  btn_analyze: 分析
  output_glb: 輸出 .glb
  btn_select_output: 選擇輸出位置
  btn_export: 匯出
  target_height: SL 目標身高(cm)
  manual_scale: 手動縮放
  texture_auto_resize: 優先縮小至 1024px
  texture_resize_hint: 'ON: >=1025px→1024px / OFF: 僅 >=2049px→2048px'
  pbr_enabled: 啟用 PBR 材質
  pbr_hint: 'ON: 處理 metallic/roughness / OFF: 僅使用簡單材質'
  blink_enabled: 眨眼 ON
  blink_interval: 眨眼間隔(秒)
  lip_sync_enabled: 口型同步 ON
  lip_open_angle: 張口角度
  fingers_enabled: 手指確認 ON
  fingers_test_pose: 手指測試姿勢
  settings_path: 設定 JSON 路徑
  btn_save_settings: 儲存設定
  btn_load_settings: 載入設定
  converted: '已轉換: {path}'
  output_tex_max_over: '轉換後貼圖最大邊: {max}px / 超過1024px(轉換後): {over}'
  current_setting: '目前設定:'
  resize_policy_aggressive: 將 >=1025px 縮小至 1024px（含 >=2049px）
  resize_policy_conservative: 僅將 >=2049px 縮小至 2048px（保留 1025〜2048px）
  logs: 日誌
  no_logs: 尚無日誌。
  bone_mapping: 骨架映射
  analyze_first: 執行分析後顯示。
  validation: 驗證
  label_model_author: '模型: {model} / 作者: {author}'
  label_height_mesh_bone: '身高推測: {height}cm / 網格: {mesh} / 骨骼: {bone}'
  label_vertices_polygons: '頂點: {vertices} / 多邊形: {polygons}'
  label_tex_cost: '貼圖費用: {before}L$ → {after}L$ ({reduction}%)'
  label_tex_policy: '貼圖縮小政策:'
  policy_aggressive_short: '>=1025px→1024px（含 >=2049px）'
  policy_conservative_short: '僅 >=2049px→2048px（保留 1025〜2048px）'
  label_converted_tex: '轉換後貼圖: {count}張 / 最大邊: {max}px / 超過1024px: {over}'
  label_resized_count: '縮小貼圖數量(推測): {count}'
  label_size_preview: '/ 縮小後尺寸示例: {preview}'
  not_analyzed: 尚未分析。
  error_no_input: 請選擇 VRM 檔案。
  error_no_paths: 請設定輸入 VRM 和輸出目的地。
  success_analyze: 分析完成。
  success_convert: '轉換完成 (scale={scale})'
  success_save_settings: 設定已儲存。
  success_load_settings: 設定已載入。
zhHans:
  title: VRM → Second Life glTF 转换
  backend_version: '后端版本: {version}'
  input_vrm: 输入 VRM
  btn_select_vrm: 选择 VRM
  btn_analyze: 分析
  output_glb: 输出 .glb
  btn_select_output: 选择输出位置
  btn_export: 导出
  target_height: SL 目标身高(cm)
  manual_scale: 手动缩放
  texture_auto_resize: 优先缩小至 1024px
  texture_resize_hint: 'ON: >=1025px→1024px / OFF: 仅 >=2049px→2048px'
  pbr_enabled: 启用 PBR 材质
  pbr_hint: 'ON: 处理 metallic/roughness / OFF: 仅使用简单材质'
  blink_enabled: 眨眼 ON
  blink_interval: 眨眼间隔(秒)
  lip_sync_enabled: 口型同步 ON
  lip_open_angle: 张口角度
  fingers_enabled: 手指确认 ON
  fingers_test_pose: 手指测试姿势
  settings_path: 设置 JSON 路径
  btn_save_settings: 保存设置
  btn_load_settings: 加载设置
  converted: '已转换: {path}'
  output_tex_max_over: '转换后贴图最大边: {max}px / 超过1024px(转换后): {over}'
  current_setting: '当前设置:'
  resize_policy_aggressive: 将 >=1025px 缩小至 1024px（含 >=2049px）
  resize_policy_conservative: 仅将 >=2049px 缩小至 2048px（保留 1025〜2048px）
  logs: 日志
  no_logs: 暂无日志。
  bone_mapping: 骨骼映射
  analyze_first: 执行分析后显示。
  validation: 验证
  label_model_author: '模型: {model} / 作者: {author}'
  label_height_mesh_bone: '身高估算: {height}cm / 网格: {mesh} / 骨骼: {bone}'
  label_vertices_polygons: '顶点: {vertices} / 多边形: {polygons}'
  label_tex_cost: '贴图费用: {before}L$ → {after}L$ ({reduction}%)'
  label_tex_policy: '贴图缩小策略:'
  policy_aggressive_short: '>=1025px→1024px（含 >=2049px）'
  policy_conservative_short: '仅 >=2049px→2048px（保留 1025〜2048px）'
  label_converted_tex: '转换后贴图: {count}张 / 最大边: {max}px / 超过1024px: {over}'
  label_resized_count: '缩小贴图数量(估算): {count}'
  label_size_preview: '/ 缩小后尺寸示例: {preview}'
  not_analyzed: 尚未分析。
  error_no_input: 请选择 VRM 文件。
  error_no_paths: 请设置输入 VRM 和输出目标。
  success_analyze: 分析完成。
  success_convert: '转换完成 (scale={scale})'
  success_save_settings: 设置已保存。
  success_load_settings: 设置已加载。
</i18n>

<style scoped>
.v-card-title {
  line-height: 1.3;
}

.log-output {
  max-height: 220px;
  overflow-y: auto;
  font-family: monospace;
  white-space: pre-wrap;
}

.log-line {
  margin-bottom: 4px;
}
</style>
