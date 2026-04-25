import { readFileSync } from 'node:fs';
import { fileURLToPath, URL } from 'node:url';

// Load value from .env file
const loadEnvValue = (key: string, defaultValue: string = ''): string => {
  try {
    const envPath = fileURLToPath(new URL('../.env', import.meta.url));
    const envContent = readFileSync(envPath, 'utf-8');
    const regex = new RegExp(`^${key}=(.+)$`, 'm');
    const match = regex.exec(envContent);
    return match ? match[1]!.trim() : defaultValue;
  } catch {
    console.warn(`Failed to load ${key} from .env, using default value`);
    return defaultValue;
  }
};

const version = loadEnvValue('VERSION', '0.0.0');
const appName = loadEnvValue('APP_NAME', 'Vrm2SL');
const siteName = loadEnvValue('NUXT_PUBLIC_SITE_NAME', 'Vrm2SL Documentation');
const siteUrl = loadEnvValue('NUXT_PUBLIC_SITE_URL', 'https://logue.dev/vrm2sl');
const appBaseUrl = loadEnvValue('NUXT_APP_BASE_URL', '/vrm2sl/');
const projectUrl = loadEnvValue('PROJECT_URL', 'https://github.com');
const googleAnalyticsId = loadEnvValue('GOOGLE_ANALYTICS_ID', 'UA-33600926-1');

const normalizedAppBaseUrl = (() => {
  const trimmed = appBaseUrl.trim();
  if (!trimmed || trimmed === '/') {
    return '/';
  }
  return `/${trimmed.replace(/^\/+|\/+$/g, '')}`;
})();

const i18nBaseUrl = (() => {
  try {
    return new URL(siteUrl).origin;
  } catch {
    return 'https://logue.dev';
  }
})();

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  // SSG設定（CSS外部化対応）
  ssr: true, // SSRで翻訳済みHTMLを生成
  // ソースコードディレクトリの変更
  srcDir: './src/',
  compatibilityDate: '2026-03-25',
  devtools: { enabled: true },
  // CSSファイル（Vuetifyスタイル確保 + GitHub Markdown CSS）
  css: ['~/styles/settings.scss'],
  // SSRスタイル設定（CSS最適化）
  features: {
    inlineStyles: false // CSS外部化
  },

  // Runtime config to expose version
  runtimeConfig: {
    public: {
      appVersion: version,
      siteUrl,
      appBaseUrl: normalizedAppBaseUrl
    }
  },

  // サイト設定
  site: {
    url: siteUrl,
    name: siteName
  },

  // アプリ設定
  app: {
    baseURL: normalizedAppBaseUrl,
    head: {
      link: [
        // app側と同じGoogle Fontsを読み込み
        {
          rel: 'preconnect',
          href: 'https://fonts.googleapis.com'
        },
        {
          rel: 'preconnect',
          href: 'https://fonts.gstatic.com',
          crossorigin: 'anonymous'
        },
        {
          rel: 'stylesheet',
          href: 'https://fonts.googleapis.com/css2?family=Noto+Color+Emoji&family=Noto+Sans+JP:wght@100..900&family=Noto+Sans+KR:wght@100..900&family=Noto+Sans+Mono:wght@100..900&family=Noto+Sans+SC:wght@100..900&family=Noto+Sans+TC:wght@100..900&family=Noto+Sans:ital,wght@0,100..900;1,100..900&display=swap'
        }
      ]
    }
  },
  // モジュール
  modules: [
    '@nuxt/content',
    '@nuxt/eslint',
    '@nuxtjs/i18n',
    '@nuxtjs/sitemap',
    '@pinia/nuxt',
    'nuxt-gtag',
    'vuetify-nuxt-module'
  ],

  // i18n設定（<i18n>ブロック使用）
  i18n: {
    locales: [
      { code: 'en', language: 'en-US', name: '🇺🇸 English', iso: 'en-US' },
      { code: 'fr', language: 'fr-FR', name: '🇫🇷 Français', iso: 'fr-FR' },
      { code: 'ja', language: 'ja-JP', name: '🇯🇵 日本語', iso: 'ja-JP' },
      { code: 'ko', language: 'ko-KR', name: '🇰🇷 한국어', iso: 'ko-KR' },
      { code: 'zhHans', language: 'zh-CN', name: '🇨🇳 简体中文', iso: 'zh-CN' },
      { code: 'zhHant', language: 'zh-TW', name: '🇹🇼 繁體中文', iso: 'zh-TW' }
    ],
    strategy: 'prefix_except_default',
    baseUrl: i18nBaseUrl,
    defaultLocale: 'en',
    detectBrowserLanguage: {
      useCookie: true,
      cookieKey: 'i18n_redirected',
      redirectOn: 'root' // recommended
    }
  },
  // Google Analytics設定（.envからIDを読み込み、空の場合は無効）
  gtag: googleAnalyticsId
    ? {
        id: googleAnalyticsId
      }
    : undefined,

  // TypeScript パスエイリアス設定
  alias: {
    '@': fileURLToPath(new URL('./src', import.meta.url))
  },

  build: {
    transpile: ['vue-i18n']
  },

  // Vite設定 - APP_NAME、NUXT_PUBLIC_SITE_NAME、PROJECT_URL をビルド時に注入
  vite: {
    define: {
      'import.meta.env.APP_NAME': JSON.stringify(appName),
      'import.meta.env.NUXT_PUBLIC_SITE_NAME': JSON.stringify(siteName),
      'import.meta.env.PROJECT_URL': JSON.stringify(projectUrl)
    }
  },

  // Nitro設定（SSG用プリレンダリング - 自動生成を利用）
  nitro: {
    prerender: {
      crawlLinks: true,
      routes: ['/'],
      failOnError: false
    }
  }
});
