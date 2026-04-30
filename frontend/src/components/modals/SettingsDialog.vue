<script setup lang="ts">
import { useConfigStore, useGlobalStore } from '@/store';
import { ref, toRef } from 'vue';
import { useI18n } from 'vue-i18n';

import type { ProjectSettings } from '@/interfaces';

import { useNotification } from '@/composables/useNotification';
import { useProjectSettings } from '@/composables/useProjectSettings';

const { t } = useI18n();
const configStore = useConfigStore();
const globalStore = useGlobalStore();
const notification = useNotification(t);
const projectSettings = useProjectSettings();

const heightScaleEnabled = toRef(configStore, 'heightScaleEnabled');
const targetHeightCm = toRef(configStore, 'targetHeightCm');
const manualScale = toRef(configStore, 'manualScale');
const textureAutoResize = toRef(configStore, 'textureAutoResize');
const pbrEnabled = toRef(configStore, 'pbrEnabled');
const inputPath = toRef(configStore, 'inputPath');
const outputPath = toRef(configStore, 'outputPath');
const settingsPath = toRef(configStore, 'settingsPath');
const textureResizeMethod = toRef(configStore, 'textureResizeMethod');
const face = toRef(configStore, 'face');
const fingers = toRef(configStore, 'fingers');

const tab = ref('common');

const toProjectSettings = (): ProjectSettings => ({
  input_path: inputPath.value || undefined,
  output_path: outputPath.value || undefined,
  target_height_cm: targetHeightCm.value,
  manual_scale: manualScale.value,
  texture_auto_resize: textureAutoResize.value,
  texture_resize_method: textureResizeMethod.value,
  pbr_enabled: pbrEnabled.value,
  face: face.value,
  fingers: fingers.value
});

const applyProjectSettings = (settings: ProjectSettings) => {
  inputPath.value = settings.input_path ?? '';
  outputPath.value = settings.output_path ?? '';
  targetHeightCm.value = settings.target_height_cm;
  manualScale.value = settings.manual_scale;
  textureAutoResize.value = settings.texture_auto_resize;
  textureResizeMethod.value = settings.texture_resize_method;
  pbrEnabled.value = settings.pbr_enabled ?? true;
  face.value = settings.face;
  fingers.value = settings.fingers;
};

const saveSettings = async () => {
  globalStore.setLoading(true);
  try {
    await projectSettings.saveSettings(settingsPath.value, toProjectSettings());
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
    const settings = await projectSettings.loadSettings(settingsPath.value);
    applyProjectSettings(settings);
    notification.success(t('success_load_settings'));
  } catch (error) {
    notification.error(String(error));
  } finally {
    globalStore.setLoading(false);
  }
};
</script>

<template>
  <v-dialog fullscreen persistent>
    <template #activator="{ props: dialogProps }">
      <v-tooltip :text="t('settings')" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="{ ...dialogProps, ...tooltipProps }"
            icon="mdi-cog-outline"
            variant="plain"
          />
        </template>
      </v-tooltip>
    </template>
    <template #default="{ isActive }">
      <v-card flat>
        <v-toolbar>
          <v-toolbar-title>{{ t('settings') }}</v-toolbar-title>
          <v-spacer />
          <v-btn icon="mdi-close" @click="isActive.value = false" />
        </v-toolbar>
        <v-card-text class="d-flex flex-row pa-0" style="height: calc(100vh - 64px)">
          <v-layout>
            <v-navigation-drawer permanent>
              <v-list nav>
                <v-list-item
                  :title="t('common_options')"
                  :active="tab === 'common'"
                  value="common"
                  @click="tab = 'common'"
                />
              </v-list>
            </v-navigation-drawer>
            <v-main class="overflow-y-auto">
              <v-card class="pa-2" flat>
                <v-window v-model="tab">
                  <v-window-item value="common">
                    <v-card flat>
                      <v-card-title>{{ t('common_options') }}</v-card-title>
                      <v-card-text>
                        <v-row>
                          <v-col cols="12" md="4" class="d-flex flex-column align-start">
                            <v-switch
                              v-model="heightScaleEnabled"
                              :label="t('height_scale_enabled')"
                            />
                          </v-col>
                          <v-col cols="12" md="4">
                            <v-text-field
                              v-model.number="targetHeightCm"
                              :label="t('target_height')"
                              :disabled="!heightScaleEnabled"
                              type="number"
                              variant="outlined"
                            />
                          </v-col>
                          <v-col cols="12" md="4">
                            <v-slider
                              v-model="manualScale"
                              :label="t('manual_scale')"
                              :disabled="!heightScaleEnabled"
                              min="0.5"
                              max="1.5"
                              step="0.01"
                              thumb-label
                            />
                          </v-col>
                          <v-col cols="12" md="4" class="d-flex flex-column align-start">
                            <v-switch
                              v-model="textureAutoResize"
                              :label="t('texture_auto_resize')"
                            />
                            <div class="text-caption text-medium-emphasis mt-1">
                              {{ t('texture_resize_hint') }}
                            </div>
                          </v-col>
                          <v-col cols="12" md="4" class="d-flex flex-column align-start">
                            <v-switch v-model="pbrEnabled" :label="t('pbr_enabled')" />
                            <div class="text-caption text-medium-emphasis mt-1">
                              {{ t('pbr_hint') }}
                            </div>
                          </v-col>
                        </v-row>
                      </v-card-text>
                    </v-card>
                  </v-window-item>
                </v-window>
              </v-card>
            </v-main>
          </v-layout>
        </v-card-text>
        <v-card-actions>
          <v-text-field
            v-model="settingsPath"
            :label="t('settings_path')"
            prepend-icon="mdi-file"
          />
          <v-btn
            :text="t('btn_save_settings')"
            prepend-icon="mdi-content-save"
            color="primary"
            @click="saveSettings"
          />
          <v-btn
            :text="t('btn_load_settings')"
            prepend-icon="mdi-folder-open"
            @click="loadSettings"
          />
        </v-card-actions>
      </v-card>
    </template>
  </v-dialog>
</template>

<i18n lang="yaml">
en:
  settings: Settings
  common_options: Common Options
  height_scale_enabled: Enable Height/Scale Override
  target_height: SL Target Height (cm)
  manual_scale: Manual Scale
  texture_auto_resize: Prefer 1024px Downscale
  texture_resize_hint: 'ON: >=1025px -> 1024px / OFF: only >=2049px -> 2048px'
  pbr_enabled: Enable PBR Materials
  pbr_hint: 'ON: Process metallic/roughness / OFF: Use only simple materials'
  settings_path: Settings JSON Path
  btn_save_settings: Save Settings
  btn_load_settings: Load Settings
  success_save_settings: Settings saved.
  success_load_settings: Settings loaded.
fr:
  settings: Paramètres
  common_options: Options communes
  height_scale_enabled: 'Activer la modification hauteur/échelle'
  target_height: Taille cible SL (cm)
  manual_scale: Échelle manuelle
  texture_auto_resize: Priorité réduction 1024px
  texture_resize_hint: 'ON: >=1025px -> 1024px / OFF: uniquement >=2049px -> 2048px'
  pbr_enabled: Activer les matériaux PBR
  pbr_hint: 'ON: Traiter metallic/roughness / OFF: Utiliser uniquement des matériaux simples'
  settings_path: Chemin JSON des paramètres
  btn_save_settings: Enregistrer
  btn_load_settings: Charger
  success_save_settings: Paramètres enregistrés.
  success_load_settings: Paramètres chargés.
ja:
  settings: 設定
  common_options: 共通設定
  height_scale_enabled: 身長・スケール変更を有効にする
  target_height: SL目標身長(cm)
  manual_scale: 手動スケール
  texture_auto_resize: 1024px優先縮小
  texture_resize_hint: 'ON: 1025px以上→1024px / OFF: 2049px以上のみ→2048px'
  pbr_enabled: PBRマテリアル有効
  pbr_hint: 'ON: metallic/roughness処理 / OFF: シンプルマテリアルのみ'
  settings_path: 設定JSONパス
  btn_save_settings: 設定保存
  btn_load_settings: 設定読込
  success_save_settings: 設定を保存しました。
  success_load_settings: 設定を読み込みました。
ko:
  settings: 설정
  common_options: 공통 설정
  height_scale_enabled: 키/스케일 변경 활성화
  target_height: SL 목표 신장(cm)
  manual_scale: 수동 스케일
  texture_auto_resize: 1024px 우선 축소
  texture_resize_hint: 'ON: 1025px 이상→1024px / OFF: 2049px 이상만→2048px'
  pbr_enabled: PBR 재료 활성화
  pbr_hint: 'ON: metallic/roughness 처리 / OFF: 단순 재료만'
  settings_path: 설정 JSON 경로
  btn_save_settings: 설정 저장
  btn_load_settings: 설정 불러오기
  success_save_settings: 설정을 저장했습니다.
  success_load_settings: 설정을 불러왔습니다.
zhHant:
  settings: 設定
  common_options: 共通設定
  height_scale_enabled: 啟用身高/縮放調整
  target_height: SL 目標身高(cm)
  manual_scale: 手動縮放
  texture_auto_resize: 優先縮小至 1024px
  texture_resize_hint: 'ON: >=1025px→1024px / OFF: 僅 >=2049px→2048px'
  pbr_enabled: 啟用 PBR 材質
  pbr_hint: 'ON: 處理 metallic/roughness / OFF: 僅使用簡單材質'
  settings_path: 設定 JSON 路徑
  btn_save_settings: 儲存設定
  btn_load_settings: 載入設定
  success_save_settings: 設定已儲存。
  success_load_settings: 設定已載入。
zhHans:
  settings: 设置
  common_options: 通用设置
  height_scale_enabled: 启用身高/缩放调整
  target_height: SL 目标身高(cm)
  manual_scale: 手动缩放
  texture_auto_resize: 优先缩小至 1024px
  texture_resize_hint: 'ON: >=1025px→1024px / OFF: 仅 >=2049px→2048px'
  pbr_enabled: 启用 PBR 材质
  pbr_hint: 'ON: 处理 metallic/roughness / OFF: 仅使用简单材质'
  settings_path: 设置 JSON 路径
  btn_save_settings: 保存设置
  btn_load_settings: 加载设置
  success_save_settings: 设置已保存。
  success_load_settings: 设置已加载。
</i18n>
