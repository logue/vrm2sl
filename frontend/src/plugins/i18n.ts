import { createI18n } from 'vue-i18n';

import { en, fr, ja, ko, zhHans, zhHant } from 'vuetify/locale';

// Import locale messages
// @intlify/unplugin-vue-i18n がYAMLファイルを自動的に処理します
import enMessages from '@/locales/en.yaml';
import frMessages from '@/locales/fr.yaml';
import jaMessages from '@/locales/ja.yaml';
import koMessages from '@/locales/ko.yaml';
import zhHansMessages from '@/locales/zhHans.yaml';
import zhHantMessages from '@/locales/zhHant.yaml';

// ユーザーのブラウザ/OS言語を取得
let locale = navigator.language.slice(0, 2) || 'en'; // フォールバックとして'en'

if (locale === 'zh') {
  // 中国語の詳細なロケールを確認
  const fullLocale = navigator.language.toLowerCase();
  if (fullLocale === 'zh-cn' || fullLocale === 'zh-sg') {
    locale = 'zhHans'; // 簡体字中国語
  } else {
    locale = 'zhHant'; // 繁体字中国語
  }
}

const i18n = createI18n({
  locale, // 'en-US' -> 'en' など
  fallbackLocale: 'en',
  messages: {
    // @ts-ignore 英語
    en: { ...enMessages, $vuetify: { ...en } },
    // @ts-ignore フランス語
    fr: { ...frMessages, $vuetify: { ...fr } },
    // @ts-ignore 日本語
    ja: { ...jaMessages, $vuetify: { ...ja } },
    // @ts-ignore 韓国語
    ko: { ...koMessages, $vuetify: { ...ko } },
    // @ts-ignore 繁体字中国語
    zhHant: { ...zhHantMessages, $vuetify: { ...zhHant } },
    // @ts-ignore 簡体字中国語
    zhHans: { ...zhHansMessages, $vuetify: { ...zhHans } }
  },
  legacy: false,
  globalInjection: true
});

document.documentElement.lang = locale;

export { i18n };
