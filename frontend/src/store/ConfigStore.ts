import { defineStore } from 'pinia';
import { type Ref, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';

import type { FaceSettings } from '@/interfaces/FaceSettings';
import type { FingerSettings } from '@/interfaces/FingerSettings';
import type { TextureResizeMethod } from '@/types/TextureResizeMethod';

/** Config Store */
const useConfigStore = defineStore('config', () => {
  // 1. i18nインスタンスからlocaleを取得
  const { locale } = useI18n({ useScope: 'global' });

  // 2. Piniaのstateとして言語を定義（デフォルト値やlocalStorageからの復元など）
  const currentLocale = ref(locale.value); // 初期値をi18nから拝借

  // 3. stateが変更されたら、i18nのlocaleにも反映させる watchを設置
  watch(currentLocale, newLocale => {
    locale.value = newLocale;
    // 必要ならlocalStorageに保存する処理もここに追加
    // localStorage.setItem('locale', newLocale)
  });

  /** Dark Theme mode */
  const theme: Ref<boolean> = ref(window.matchMedia('(prefers-color-scheme: dark)').matches);

  /** Toggle Dark/Light mode */
  const toggleTheme = () => (theme.value = !theme.value);
  /**
   * Set Locale.
   *
   * @param locale - Locale
   */
  const setLocale = (l: string) => (locale.value = l);

  /** Enable height/scale override */
  const heightScaleEnabled = ref(true);
  /** SL target height in cm */
  const targetHeightCm = ref(200);
  /** Manual scale multiplier */
  const manualScale = ref(1);
  /** Prefer 1024px downscale */
  const textureAutoResize = ref(true);
  /** Enable PBR materials */
  const pbrEnabled = ref(true);

  /** Input VRM file path */
  const inputPath = ref('');
  /** Output GLB file path */
  const outputPath = ref('');
  /** Project settings JSON path */
  const settingsPath = ref('project-settings.json');
  /** Texture resize interpolation method */
  const textureResizeMethod = ref<TextureResizeMethod>('Bilinear');
  /** Face-related settings */
  const face = ref<FaceSettings>({
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
  /** Finger-related settings */
  const fingers = ref<FingerSettings>({ enabled: true, test_pose: 'open' });

  return {
    theme,
    locale,
    toggleTheme,
    setLocale,
    heightScaleEnabled,
    targetHeightCm,
    manualScale,
    textureAutoResize,
    pbrEnabled,
    inputPath,
    outputPath,
    settingsPath,
    textureResizeMethod,
    face,
    fingers
  };
});

export { useConfigStore };
