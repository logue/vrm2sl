<script setup lang="ts">
import { ref } from 'vue';

import type { AnalysisReport, ConversionReport } from '@/interfaces';

import BoneMappingPanel from '@/components/panels/BoneMappingPanel.vue';
import ConversionPanel from '@/components/panels/ConversionPanel.vue';
import LogPanel from '@/components/panels/LogPanel.vue';
import ValidationPanel from '@/components/panels/ValidationPanel.vue';
import { useLogger } from '@/composables/useLogger';

const analysis = ref<AnalysisReport | null>(null);
const conversion = ref<ConversionReport | null>(null);

const { logs } = useLogger();
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
