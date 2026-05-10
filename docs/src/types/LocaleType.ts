/**
 * Union type of supported locale codes.
 */
export type Locale = (typeof Locale)[keyof typeof Locale];

/**
 * Canonical locale code map used across docs i18n logic.
 */
export const Locale = {
  en: 'en',
  ja: 'ja',
  fr: 'fr',
  ko: 'ko',
  zhHans: 'zhHans',
  zhHant: 'zhHant'
} as const;
