<script setup lang="ts">
import { useConfigStore } from '@/store';
import { computed, onMounted, ref, type ComputedRef, type Ref } from 'vue';

import { useTheme } from 'vuetify';

/** Vuetify Theme */
const theme = useTheme();

/** Config Store */
const configStore = useConfigStore();

/** drawer visibility */
const drawer: Ref<boolean> = ref(false);

// Extract import.meta.env value into a variable for template usage.
const appName = import.meta.env.APP_NAME as string | undefined;

/**
 * Initialize dark mode detection from system preference (client-side only).
 * This must be called after hydration to avoid hydration mismatches.
 */
onMounted(() => {
  configStore.initializeTheme();
});

/** Toggle Dark mode */
const isDark: ComputedRef<string> = computed(() => (configStore.theme ? 'dark' : 'light'));

/** Fallback color for browser theme-color meta tag. */
const DEFAULT_THEME_COLOR = '#1976D2';

/**
 * Convert arbitrary Vuetify color values into a valid CSS color string for theme-color.
 * Prefers rgba() output when channels are available, otherwise keeps HEX when valid.
 */
const normalizeThemeColor = (value: unknown): string => {
  if (typeof value === 'string') {
    const trimmed = value.trim();

    if (/^rgba?\(/i.test(trimmed)) {
      return trimmed;
    }

    if (/^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(trimmed)) {
      return trimmed;
    }

    const channels = trimmed.split(',').map(part => part.trim());
    if (channels.length === 3 || channels.length === 4) {
      const r = Number.parseInt(channels[0] ?? '', 10);
      const g = Number.parseInt(channels[1] ?? '', 10);
      const b = Number.parseInt(channels[2] ?? '', 10);
      const a = channels.length === 4 ? Number.parseFloat(channels[3] ?? '') : 1;

      if (
        [r, g, b].every(channel => Number.isFinite(channel) && channel >= 0 && channel <= 255) &&
        Number.isFinite(a) &&
        a >= 0 &&
        a <= 1
      ) {
        return `rgba(${r}, ${g}, ${b}, ${a})`;
      }
    }
  }

  if (typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 0xffffff) {
    const hex = Math.round(value).toString(16).padStart(6, '0').toUpperCase();
    return `#${hex}`;
  }

  return DEFAULT_THEME_COLOR;
};

/** Resolved meta theme-color value for the current active theme. */
const themeColor: ComputedRef<string> = computed(() => {
  const currentPrimary = theme.computedThemes.value?.[isDark.value]?.colors?.primary;
  return normalizeThemeColor(currentPrimary);
});
</script>

<template>
  <v-app :theme="isDark">
    <v-navigation-drawer v-model="drawer" permanent>
      <drawer-menu />
    </v-navigation-drawer>

    <v-app-bar color="primary">
      <v-app-bar-nav-icon @click="drawer = !drawer" />
      <v-app-bar-title>{{ appName }}</v-app-bar-title>
      <v-spacer />
      <app-bar-menu-component />
    </v-app-bar>

    <v-main>
      <v-container>
        <slot />
      </v-container>
    </v-main>

    <v-footer app elevation="3" color="primary">
      <span class="mr-5">2026 &copy;</span>
    </v-footer>
  </v-app>
  <teleport to="head">
    <meta name="theme-color" :content="themeColor" />
  </teleport>
</template>
