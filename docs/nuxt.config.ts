import { readFileSync } from 'node:fs';
import { fileURLToPath, URL } from 'node:url';

/** Nuxt compatibility date used by this docs app. */
const NUXT_COMPATIBILITY_DATE = '2026-03-25';

/** Shared Google Fonts links for docs pages. */
const APP_HEAD_LINKS = [
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
] as const;

/** i18n locale definitions used by @nuxtjs/i18n. */
const I18N_LOCALES = [
  { code: 'en', language: 'en-US', name: '🇺🇸 English', iso: 'en-US' },
  { code: 'fr', language: 'fr-FR', name: '🇫🇷 Français', iso: 'fr-FR' },
  { code: 'ja', language: 'ja-JP', name: '🇯🇵 日本語', iso: 'ja-JP' },
  { code: 'ko', language: 'ko-KR', name: '🇰🇷 한국어', iso: 'ko-KR' },
  { code: 'zhHans', language: 'zh-CN', name: '🇨🇳 简体中文', iso: 'zh-CN' },
  { code: 'zhHant', language: 'zh-TW', name: '🇹🇼 繁體中文', iso: 'zh-TW' }
] as const;

/** Enabled Nuxt modules for docs app. */
const NUXT_MODULES = [
  '@nuxt/content',
  '@nuxt/eslint',
  '@nuxtjs/i18n',
  '@nuxtjs/sitemap',
  '@pinia/nuxt',
  'nuxt-gtag',
  'vuetify-nuxt-module'
] as const;

/**
 * Load a key from the repository root .env file.
 * Keeps values containing spaces intact by splitting at the first '=' only.
 */
const loadEnvValue = (key: string, defaultValue: string = ''): string => {
  try {
    const envPath = fileURLToPath(new URL('../.env', import.meta.url));
    const envContent = readFileSync(envPath, 'utf-8');
    for (const line of envContent.split(/\r?\n/u)) {
      const trimmedLine = line.trim();
      if (!trimmedLine || trimmedLine.startsWith('#')) {
        continue;
      }

      const separatorIndex = trimmedLine.indexOf('=');
      if (separatorIndex === -1) {
        continue;
      }

      const currentKey = trimmedLine.slice(0, separatorIndex).trim();
      if (currentKey !== key) {
        continue;
      }

      return trimmedLine.slice(separatorIndex + 1).trim();
    }
    return defaultValue;
  } catch {
    console.warn(`Failed to load ${key} from .env, using default value`);
    return defaultValue;
  }
};

const version = loadEnvValue('VERSION', '0.0.0');
const appName = loadEnvValue('APP_NAME', 'Tauri Vuetify Starter');
const siteName = loadEnvValue('NUXT_PUBLIC_SITE_NAME', 'Tauri Vuetify Starter Documentation');
const siteUrl = loadEnvValue('NUXT_PUBLIC_SITE_URL', 'https://logue.dev/tauri-vuetify-starter');
const appBaseUrl = loadEnvValue('NUXT_APP_BASE_URL', '/tauri-vuetify-starter/');
const projectUrl = loadEnvValue('PROJECT_URL', 'https://github.com');
const googleAnalyticsId = loadEnvValue('GOOGLE_ANALYTICS_ID', '');

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
  // Enable SSR to generate localized HTML for static output.
  ssr: true,
  // Set custom source directory.
  srcDir: './src/',
  compatibilityDate: NUXT_COMPATIBILITY_DATE,
  devtools: { enabled: true },
  // Global CSS.
  css: ['~/styles/settings.scss'],
  // Keep CSS externalized in SSR output.
  features: {
    inlineStyles: false
  },

  // Runtime config values exposed to the client.
  runtimeConfig: {
    public: {
      appVersion: version,
      appNameKebab: loadEnvValue('APP_NAME_KEBAB', 'vrm2sl'),
      siteUrl,
      appBaseUrl: normalizedAppBaseUrl
    }
  },

  // Site metadata for nuxt-site-config consumers.
  site: {
    url: siteUrl,
    name: siteName
  },

  // App-level head/base configuration.
  app: {
    baseURL: normalizedAppBaseUrl,
    head: {
      link: [...APP_HEAD_LINKS]
    }
  },

  modules: [...NUXT_MODULES],

  // i18n configuration.
  i18n: {
    locales: [...I18N_LOCALES],
    strategy: 'prefix_except_default',
    baseUrl: i18nBaseUrl,
    defaultLocale: 'en',
    detectBrowserLanguage: {
      useCookie: true,
      cookieKey: 'i18n_redirected',
      redirectOn: 'root'
    }
  },

  // Enable Google Analytics only when ID is provided.
  gtag: googleAnalyticsId
    ? {
        id: googleAnalyticsId
      }
    : undefined,

  // TypeScript path alias.
  alias: {
    '@': fileURLToPath(new URL('./src', import.meta.url))
  },

  build: {
    transpile: ['vue-i18n']
  },

  // Inject selected env values at build time.
  vite: {
    define: {
      'import.meta.env.APP_NAME': JSON.stringify(appName),
      'import.meta.env.NUXT_PUBLIC_SITE_NAME': JSON.stringify(siteName),
      'import.meta.env.PROJECT_URL': JSON.stringify(projectUrl)
    }
  },

  // Prerender settings for static deployment.
  nitro: {
    prerender: {
      crawlLinks: true,
      routes: ['/'],
      failOnError: false
    }
  }
});
