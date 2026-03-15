<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { useConfigStore } from '@/store';

import LocaleSelector from './LocaleSelector.vue';
import AboutDialog from './modals/AboutDialog.vue';
import SettingsDialog from './modals/SettingsDialog.vue';

const { t } = useI18n();
/** Config Store */
const configStore = useConfigStore();
</script>

<template>
  <!-- About -->
  <about-dialog />
  <v-tooltip :text="t('donate')" location="bottom" open-delay="300">
    <template #activator="{ props }">
      <v-btn
        v-bind="props"
        icon="mdi-coffee"
        href="https://ko-fi.com/logue256"
        target="_blank"
        variant="plain"
      />
    </template>
  </v-tooltip>
  <!-- Locale Menu -->
  <locale-selector />
  <!-- Toggle Dark mode -->
  <v-tooltip :text="t('toggle_dark_mode_btn')" location="bottom">
    <template #activator="{ props }">
      <v-btn
        v-bind="props"
        icon="mdi-theme-light-dark"
        variant="plain"
        @click="configStore.toggleTheme"
      />
    </template>
  </v-tooltip>
  <!-- Settings -->
  <settings-dialog />
</template>

<i18n lang="yaml">
en:
  toggle_dark_mode_btn: Toggle Dark Mode
  donate: Donate
fr:
  toggle_dark_mode_btn: Basculer en mode sombre
  donate: Faire un don
ja:
  toggle_dark_mode_btn: ダークモード切り替え
  donate: 寄付
ko:
  toggle_dark_mode_btn: 다크 모드 전환
  donate: 기부
zhHant:
  toggle_dark_mode_btn: 切換深色模式
  donate: 捐助
zhHans:
  toggle_dark_mode_btn: 切换深色模式
  donate: 捐助
</i18n>
