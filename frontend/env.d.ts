/// <reference types="vite/client" />
/// <reference types="@tauri-apps/api" />

declare module '@/locales/*.yaml' {
  import type { LocaleMessageObject } from 'vue-i18n';

  const data: LocaleMessageObject;
  export default data;
}

/**
 * Vite environment variables available via import.meta.env.
 * Note: APP_NAME, VERSION, PROJECT_URL are injected as __APP_NAME__, __APP_VERSION__, __PROJECT_SITE__
 * via vite.config.ts define (see src/types/env.d.ts).
 */
interface ImportMetaEnv {
  /**
   * Vite runtime flags.
   */
  readonly DEV: boolean;
  readonly MODE: string;
  readonly BASE_URL: string;
  readonly PROD: boolean;
  readonly SSR: boolean;
}

/**
 * ImportMeta typing for Vite.
 */
interface ImportMeta {
  readonly env: ImportMetaEnv;
}
