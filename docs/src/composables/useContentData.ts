/**
 * Provides localized content helpers used by the landing page.
 *
 * @returns Feature metadata and normalized lead descriptions.
 */
export const useContentData = () => {
  const { tm } = useI18n();

  // Feature descriptor list used by cards and sections.
  const features = [
    {
      icon: 'mdi-package-variant',
      key: 'multiple_formats'
    },
    {
      icon: 'mdi-lightning-bolt',
      key: 'high_speed'
    },
    {
      icon: 'mdi-application-outline',
      key: 'drag_drop'
    },
    {
      icon: 'mdi-earth',
      key: 'i18n'
    },
    {
      icon: 'mdi-theme-light-dark',
      key: 'dark_mode'
    },
    {
      icon: 'mdi-shield-check',
      key: 'paste'
    }
  ];

  /**
   * Safely resolves lead description entries from i18n messages.
   *
   * @returns Description lines as an array of strings.
   */
  const leadDescriptions = computed(() => {
    try {
      const descriptions = tm('lead.description') as unknown;
      return Array.isArray(descriptions) ? (descriptions as string[]) : [];
    } catch {
      return [];
    }
  });

  return {
    features,
    leadDescriptions
  };
};
