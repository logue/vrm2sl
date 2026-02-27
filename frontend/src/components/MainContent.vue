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
  manual_scale: 1.0,
  texture_auto_resize: true,
  texture_resize_method: 'Bilinear'
});

const face = ref<ProjectSettings['face']>({
  blink: { enabled: true, interval_sec: 4.0, close_duration_sec: 0.15, wink_enabled: true },
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
  face.value = settings.face;
  fingers.value = settings.fingers;
};

const pickInputFile = async () => {
  const selected = await fs.selectFiles({
    multiple: false,
    filters: [{ name: 'VRM', extensions: ['vrm', 'glb'] }]
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
    notification.error('VRMファイルを選択してください');
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
    notification.success('解析が完了しました');
  } catch (error) {
    notification.error(String(error));
  } finally {
    globalStore.setLoading(false);
  }
};

const runExport = async () => {
  if (!inputPath.value || !outputPath.value) {
    notification.error('入力VRMと出力先を設定してください');
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
    notification.success(`変換完了 (scale=${result.computed_scale_factor.toFixed(4)})`);
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
    notification.success('設定を保存しました');
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
    notification.success('設定を読み込みました');
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
            VRM → Second Life glTF 変換
          </v-card-title>

          <v-card-subtitle v-if="appVersion">Backend Version: {{ appVersion }}</v-card-subtitle>

          <v-card-text>
            <v-row>
              <v-col cols="12" md="6">
                <v-text-field
                  v-model="inputPath"
                  label="入力VRM"
                  variant="outlined"
                  density="comfortable"
                />
              </v-col>
              <v-col cols="12" md="6" class="d-flex ga-2 align-center">
                <v-btn prepend-icon="mdi-folder-open" @click="pickInputFile">VRM選択</v-btn>
                <v-btn color="primary" prepend-icon="mdi-magnify" @click="runAnalyze">解析</v-btn>
              </v-col>
            </v-row>

            <v-row>
              <v-col cols="12" md="6">
                <v-text-field
                  v-model="outputPath"
                  label="出力.glb"
                  variant="outlined"
                  density="comfortable"
                />
              </v-col>
              <v-col cols="12" md="6" class="d-flex ga-2 align-center">
                <v-btn prepend-icon="mdi-content-save" @click="pickOutputFile">保存先選択</v-btn>
                <v-btn
                  color="success"
                  prepend-icon="mdi-file-export"
                  :disabled="hasBlockingIssue"
                  @click="runExport"
                >
                  エクスポート
                </v-btn>
              </v-col>
            </v-row>

            <v-row>
              <v-col cols="12" md="4">
                <v-text-field
                  v-model.number="options.target_height_cm"
                  type="number"
                  label="SL目標身長(cm)"
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
                  label="手動スケール"
                />
              </v-col>
              <v-col cols="12" md="4" class="d-flex flex-column align-start">
                <v-switch v-model="options.texture_auto_resize" label="1024px優先縮小" />
                <div class="text-caption text-medium-emphasis mt-1">
                  ON: 1025px以上→1024px / OFF: 2049px以上のみ→2048px
                </div>
              </v-col>
            </v-row>

            <v-divider class="my-4" />

            <v-row>
              <v-col cols="12" md="4">
                <v-switch v-model="face.blink.enabled" label="瞬きON" />
                <v-slider
                  v-model="face.blink.interval_sec"
                  min="1"
                  max="10"
                  step="0.1"
                  label="瞬き間隔(秒)"
                />
              </v-col>
              <v-col cols="12" md="4">
                <v-switch v-model="face.lip_sync.enabled" label="クチパクON" />
                <v-slider
                  v-model="face.lip_sync.open_angle"
                  min="0"
                  max="1"
                  step="0.01"
                  label="開口角度"
                />
              </v-col>
              <v-col cols="12" md="4">
                <v-switch v-model="fingers.enabled" label="指確認ON" />
                <v-select
                  v-model="fingers.test_pose"
                  :items="['open', 'fist']"
                  label="指テストポーズ"
                />
              </v-col>
            </v-row>

            <v-row>
              <v-col cols="12" md="8">
                <v-text-field
                  v-model="settingsPath"
                  label="設定JSONパス"
                  variant="outlined"
                  density="comfortable"
                />
              </v-col>
              <v-col cols="12" md="4" class="d-flex ga-2 align-center">
                <v-btn prepend-icon="mdi-content-save" @click="saveSettings">設定保存</v-btn>
                <v-btn prepend-icon="mdi-folder-open" @click="loadSettings">設定読込</v-btn>
              </v-col>
            </v-row>

            <v-alert v-if="convertResultPath" type="success" class="mt-2" variant="tonal">
              変換済み: {{ convertResultPath }}
            </v-alert>
            <v-alert v-if="conversion" type="info" class="mt-2" variant="tonal">
              変換後テクスチャ最大辺: {{ outputMaxTextureDimension }}px / 1024px超過(変換後):
              {{ conversion.output_texture_over_1024_count }}
              <div class="text-caption mt-1">
                現在の設定:
                {{
                  options.texture_auto_resize
                    ? '1025px以上を1024pxへ縮小（2049px以上も1024px）'
                    : '2049px以上のみ2048pxへ縮小（1025〜2048pxは維持）'
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
            ログ
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
              <div v-else class="text-medium-emphasis">ログはまだありません。</div>
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
            ボーンマッピング
          </v-card-title>
          <v-card-text>
            <v-list v-if="analysis" density="compact">
              <v-list-item v-for="pair in analysis.mapped_bones" :key="pair[0]">
                <v-list-item-title>{{ pair[0] }} → {{ pair[1] }}</v-list-item-title>
              </v-list-item>
            </v-list>
            <v-alert v-else type="info" variant="tonal">解析すると表示されます。</v-alert>
          </v-card-text>
        </v-card>
      </v-col>

      <v-col cols="12" md="7">
        <v-card>
          <v-card-title>
            <v-icon icon="mdi-alert-circle" class="mr-2" />
            バリデーション
          </v-card-title>
          <v-card-text>
            <v-list v-if="analysis" density="compact">
              <v-list-item>
                <v-list-item-title>
                  モデル: {{ analysis.model_name }} / 作者: {{ analysis.author || 'Unknown' }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item>
                <v-list-item-title>
                  身長推定: {{ analysis.estimated_height_cm.toFixed(2) }}cm / メッシュ:
                  {{ analysis.mesh_count }} / ボーン: {{ analysis.bone_count }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item>
                <v-list-item-title>
                  頂点: {{ analysis.total_vertices }} / ポリゴン: {{ analysis.total_polygons }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item>
                <v-list-item-title>
                  テクスチャ費用: {{ analysis.fee_estimate.before_linden_dollar }}L$ →
                  {{ analysis.fee_estimate.after_resize_linden_dollar }}L$ ({{
                    analysis.fee_estimate.reduction_percent
                  }}%)
                </v-list-item-title>
              </v-list-item>
              <v-list-item>
                <v-list-item-title>
                  テクスチャ縮小ポリシー:
                  {{
                    options.texture_auto_resize
                      ? '1025px以上→1024px（2049px以上を含む）'
                      : '2049px以上のみ→2048px（1025〜2048pxは維持）'
                  }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item v-if="conversion">
                <v-list-item-title>
                  変換後テクスチャ: {{ conversion.output_texture_infos.length }}枚 / 最大辺:
                  {{ outputMaxTextureDimension }}px / 1024px超過:
                  {{ conversion.output_texture_over_1024_count }}
                </v-list-item-title>
              </v-list-item>
              <v-list-item v-if="conversion">
                <v-list-item-title>
                  縮小適用テクスチャ数(推定): {{ resizedTextureCount }}
                  <span v-if="outputTextureSizePreview">
                    / 縮小後サイズ例: {{ outputTextureSizePreview }}
                  </span>
                </v-list-item-title>
              </v-list-item>
              <v-list-item v-for="issue in analysis.issues" :key="`${issue.code}-${issue.message}`">
                <v-list-item-title>[{{ issue.severity }}] {{ issue.message }}</v-list-item-title>
              </v-list-item>
            </v-list>
            <v-alert v-else type="info" variant="tonal">未解析です。</v-alert>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<style scoped>
.v-card-title {
  line-height: 1.3;
}

.log-output {
  max-height: 220px;
  overflow-y: auto;
  font-family: monospace;
  white-space: pre-wrap;
  word-break: break-word;
}

.log-line {
  margin-bottom: 4px;
}
</style>
