<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';

const { t } = useI18n();

const tab = ref('common');
</script>

<template>
  <v-dialog fullscreen persistent>
    <template #activator="{ props: dialogProps }">
      <v-tooltip :text="t('settings')" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="{ ...dialogProps, ...tooltipProps }"
            icon="mdi-cog-outline"
            variant="plain"
          />
        </template>
      </v-tooltip>
    </template>
    <template #default="{ isActive }">
      <v-card flat>
        <v-toolbar>
          <v-toolbar-title>{{ t('settings') }}</v-toolbar-title>
          <v-spacer />
          <v-btn icon="mdi-close" @click="isActive.value = false" />
        </v-toolbar>
        <v-card-text class="d-flex flex-row pa-0" style="height: calc(100vh - 64px)">
          <v-layout>
            <v-navigation-drawer permanent>
              <v-list nav>
                <v-list-item
                  :title="t('common_options')"
                  value="common"
                  :active="tab === 'common'"
                  @click="tab = 'common'"
                />
              </v-list>
            </v-navigation-drawer>
            <v-main class="overflow-y-auto">
              <v-card flat class="pa-2">
                <v-window v-model="tab">
                  <v-window-item value="common">
                    <v-card flat>
                      <v-card-title>{{ t('common_options') }}</v-card-title>
                      <v-card-text>
                        <!-- Common options content goes here -->
                        <p>Here you can add common settings for your application.</p>
                      </v-card-text>
                    </v-card>
                  </v-window-item>
                </v-window>
              </v-card>
            </v-main>
          </v-layout>
        </v-card-text>
      </v-card>
    </template>
  </v-dialog>
</template>

<i18n lang="yaml">
en:
  settings: Settings
  common_options: Common Options
fr:
  settings: Paramètres
  common_options: Options communes
ja:
  settings: 設定
  common_options: 共通設定
ko:
  settings: 설정
  common_options: 공통 설정
zhHant:
  settings: 設定
  common_options: 共通設定
zhHans:
  settings: 设置
  common_options: 通用设置
</i18n>
