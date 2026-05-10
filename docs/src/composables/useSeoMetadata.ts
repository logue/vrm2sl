/**
 * Provides SEO metadata, hreflang links, and JSON-LD helpers.
 *
 * @returns Reactive SEO metadata builders and setup function.
 */
import ogp from '@/assets/ogp.png';

export const useSeoMetadata = () => {
  const { locale, t } = useI18n();
  const { version } = useDownloads();
  const config = useRuntimeConfig();

  // Absolute site base URL.
  const sitePath = computed(() => {
    const value = String(config.public.siteUrl || 'https://logue.dev');
    return value.replace(/\/+$/, '');
  });

  // Application base path.
  const appBasePath = computed(() => {
    const value = String(config.public.appBaseUrl || '/');
    if (!value || value === '/') {
      return '/';
    }
    return `/${value.replace(/^\/+|\/+$/g, '')}`;
  });

  const rootUrl = computed(() => {
    const normalizedPath = appBasePath.value === '/' ? '/' : `${appBasePath.value}/`;
    return new URL(normalizedPath, `${sitePath.value}/`).toString();
  });

  // Project repository base URL.
  const projectUrl = import.meta.env.PROJECT_URL || '/';

  const currentUrl = computed(() => {
    const path = locale.value === 'en' ? '' : `${locale.value}/`;
    return new URL(path, rootUrl.value).toString();
  });

  // OGP image URL.
  const ogImage = computed(() => {
    return new URL(ogp, rootUrl.value).toString();
  });

  // Supported language descriptors.
  const languages = [
    { code: 'en', name: '🇺🇸 English' },
    { code: 'ja', name: '🇯🇵 日本語' },
    { code: 'fr', name: '🇫🇷 Français' },
    { code: 'ko', name: '🇰🇷 한국어' },
    { code: 'zhHans', name: '🇨🇳 简体中文' },
    { code: 'zhHant', name: '🇹🇼 繁體中文' }
  ];

  // hreflang link entries.
  const hreflangLinks = computed(() => {
    const links = [];

    // x-default (English root).
    links.push({
      rel: 'alternate',
      hreflang: 'x-default',
      href: rootUrl.value
    });

    // Per-language entries.
    languages.forEach(lang => {
      if (lang.code === 'en') {
        links.push({
          rel: 'alternate',
          hreflang: 'en',
          href: rootUrl.value
        });
      } else {
        links.push({
          rel: 'alternate',
          hreflang: lang.code,
          href: new URL(`${lang.code}/`, rootUrl.value).toString()
        });
      }
    });

    return links;
  });

  // JSON-LD structured data payload.
  const jsonLdData = computed(() => {
    const languageMap: Record<string, string> = {
      ja: 'ja',
      en: 'en',
      fr: 'fr',
      ko: 'ko',
      zhHans: 'zh-CN',
      zhHant: 'zh-TW'
    };

    return {
      '@context': 'https://schema.org',
      '@type': 'SoftwareApplication',
      name: 'Tauri Vue3 App',
      applicationCategory: 'DeveloperApplication',
      operatingSystem: 'Windows, macOS, Linux',
      offers: {
        '@type': 'Offer',
        price: '0',
        priceCurrency: 'USD'
      },
      description: unref(t('lead.description[0]')),
      url: currentUrl.value,
      image: unref(ogImage),
      softwareVersion: unref(version),
      releaseNotes: `${projectUrl}/releases/tag/${unref(version)}`,
      downloadUrl: `${projectUrl}/releases/download/${unref(version)}/`,
      author: {
        '@type': 'Person',
        name: 'Logue',
        url: 'https://github.com/logue'
      },
      featureList: [
        unref(t('features.multiple_formats.title')),
        unref(t('features.high_speed.title')),
        unref(t('features.drag_drop.title')),
        unref(t('features.i18n.title')),
        unref(t('features.dark_mode.title')),
        unref(t('features.paste.title'))
      ],
      screenshot: unref(ogImage),
      inLanguage: languageMap[locale.value] || 'en'
    };
  });

  /**
   * Applies SEO meta tags and structured data to the current page.
   *
   * @returns Void.
   */
  const setupSeoMeta = () => {
    useSeoMeta({
      title: computed(() => `Tauri Vue3 App - ${t('lead.subtitle')}`),
      ogSiteName: 'Tauri Vue3 App',
      description: computed(() => t('lead.description[0]')),
      ogTitle: 'Tauri Vue3 App',
      ogDescription: computed(() => t('lead.subtitle')),
      ogImage: ogImage,
      ogUrl: currentUrl,
      ogType: 'website',
      ogLocale: computed(() => {
        const localeMap: Record<string, string> = {
          ja: 'ja_JP',
          en: 'en_US',
          fr: 'fr_FR',
          ko: 'ko_KR',
          zhHans: 'zh_CN',
          zhHant: 'zh_TW'
        };
        return localeMap[locale.value] || 'en_US';
      }),
      twitterCard: 'summary_large_image',
      twitterTitle: 'Tauri Vue3 App',
      twitterDescription: computed(() => t('lead.subtitle')),
      twitterImage: ogImage
    });

    useHead({
      link: hreflangLinks,
      script: [
        {
          type: 'application/ld+json',
          innerHTML: () => JSON.stringify(jsonLdData.value)
        }
      ]
    });
  };

  return {
    sitePath,
    currentUrl,
    ogImage,
    languages,
    hreflangLinks,
    jsonLdData,
    setupSeoMeta
  };
};
