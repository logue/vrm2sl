<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { AnalysisReport, ConversionReport } from '@/interfaces';

import BoneMappingPanel from '@/components/panels/BoneMappingPanel.vue';
import ConversionPanel from '@/components/panels/ConversionPanel.vue';
import LogPanel from '@/components/panels/LogPanel.vue';
import ValidationPanel from '@/components/panels/ValidationPanel.vue';

const analysis = ref<AnalysisReport | null>(null);
const conversion = ref<ConversionReport | null>(null);
const logs = ref<{ level: string; message: string; timestamp: string }[]>([]);

let unlistenLogMessage: UnlistenFn | null = null;

onMounted(async () => {
  unlistenLogMessage = await listen<{ level: string; message: string; timestamp: string }>(
    'log-message',
    event => {
      const payload = event.payload;
      logs.value.push({
        level: payload.level,
        message: payload.message,
        timestamp: payload.timestamp
      });
      if (logs.value.length > 200) {
        logs.value.splice(0, logs.value.length - 200);
      }
    }
  );
});

onBeforeUnmount(() => {
  if (unlistenLogMessage) {
    unlistenLogMessage();
    unlistenLogMessage = null;
  }
});
</script>

<template>
  <v-container fluid>
    <conversion-panel v-model:analysis="analysis" v-model:conversion="conversion" />

    <v-row class="mt-4">
      <v-col cols="12">
        <log-panel :logs="logs" />
      </v-col>
    </v-row>

    <v-row class="mt-4">
      <v-col cols="12" md="5">
        <bone-mapping-panel :analysis="analysis" />
      </v-col>
      <v-col cols="12" md="7">
        <validation-panel :analysis="analysis" :conversion="conversion" />
      </v-col>
    </v-row>
  </v-container>
</template>
