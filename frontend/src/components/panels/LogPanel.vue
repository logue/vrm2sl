<script setup lang="ts">
import { useI18n } from 'vue-i18n';

defineProps<{
  logs: { level: string; message: string; timestamp: string }[];
}>();

const { t } = useI18n();
</script>

<template>
  <v-card :title="t('logs')" prepend-icon="mdi-text-box-search-outline">
    <v-card-text>
      <ul class="log-output" role="log" aria-live="polite">
        <template v-if="logs.length > 0">
          <li v-for="(entry, index) in logs" :key="`${entry.timestamp}-${index}`" class="log-line">
            {{ `[${entry.timestamp}] [${entry.level.toUpperCase()}] ${entry.message}` }}
          </li>
        </template>
        <v-alert v-else type="info" variant="tonal">{{ t('no_logs') }}</v-alert>
      </ul>
    </v-card-text>
  </v-card>
</template>

<i18n lang="yaml">
en:
  logs: Logs
  no_logs: No logs yet.
fr:
  logs: Journaux
  no_logs: "Aucun journal pour l'instant."
ja:
  logs: ログ
  no_logs: ログはまだありません。
ko:
  logs: 로그
  no_logs: 아직 로그가 없습니다.
zhHant:
  logs: 日誌
  no_logs: 尚無日誌。
zhHans:
  logs: 日志
  no_logs: 暂无日志。
</i18n>

<style scoped>
.log-output {
  max-height: 220px;
  overflow-y: auto;
  font-family: monospace;
  white-space: pre-wrap;
  list-style-type: none;
  padding: 0;
}

.log-line {
  margin-bottom: 4px;
}
</style>
