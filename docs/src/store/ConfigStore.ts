import { defineStore } from 'pinia';
import { type Ref, ref } from 'vue';

/** Config Store */
export default defineStore('config', () => {
  // 1. Initialize theme with a hydration-safe default (light mode).
  // This value will be updated on client-side after hydration.
  const theme: Ref<boolean> = ref(false);

  // 2. Initialize locale as empty (will be set by components using useI18n).
  const currentLocale = ref<string>('');

  /** Toggle Dark/Light mode */
  const toggleTheme = () => (theme.value = !theme.value);

  /**
   * Set Locale.
   *
   * @param locale - Locale code
   */
  const setLocale = (locale: string) => (currentLocale.value = locale);

  /**
   * Initialize theme from system preference (client-only).
   */
  const initializeTheme = () => {
    if (import.meta.client && typeof window !== 'undefined') {
      const darkModeQuery = window.matchMedia('(prefers-color-scheme: dark)');
      // Set initial value from system preference.
      theme.value = darkModeQuery.matches;

      // Listen for system dark mode changes.
      darkModeQuery.addEventListener('change', e => {
        theme.value = e.matches;
      });
    }
  };

  return { theme, currentLocale, toggleTheme, setLocale, initializeTheme };
});
