<script setup lang="ts">
import { useI18n } from 'vue-i18n';

import type { AnalysisReport } from '@/interfaces';

defineProps<{
  analysis: AnalysisReport | null;
}>();

const { t } = useI18n();
</script>

<template>
  <v-card :title="t('bone_mapping')" prepend-icon="mdi-bone">
    <v-card-text>
      <v-list v-if="analysis" density="compact">
        <v-list-item
          v-for="pair in analysis.mapped_bones"
          :key="pair[0]"
          :title="`${pair[0]} → ${pair[1]}`"
        />
      </v-list>
      <v-alert v-else type="info" variant="tonal">{{ t('analyze_first') }}</v-alert>
    </v-card-text>
  </v-card>
</template>

<i18n lang="yaml">
en:
  bone_mapping: Bone Mapping
  analyze_first: Run analysis to view.
fr:
  bone_mapping: Mapping des os
  analyze_first: "Lancez l'analyse pour afficher."
ja:
  bone_mapping: ボーンマッピング
  analyze_first: 解析すると表示されます。
ko:
  bone_mapping: 본 매핑
  analyze_first: 분석을 실행하면 표시됩니다.
zhHant:
  bone_mapping: 骨架映射
  analyze_first: 執行分析後顯示。
zhHans:
  bone_mapping: 骨骼映射
  analyze_first: 执行分析后显示。
</i18n>
