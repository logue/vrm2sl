<script setup lang="ts">
type AlertKind = 'success' | 'info' | 'warning' | 'error';

interface Props {
  type?: string;
  title?: string;
  icon?: string;
}

const props = withDefaults(defineProps<Props>(), {
  type: 'info',
  title: '',
  icon: ''
});

const { t } = useI18n();

const normalizedType = computed<AlertKind>(() => {
  switch (props.type) {
    case 'success':
    case 'warning':
    case 'error':
      return props.type;
    default:
      return 'info';
  }
});

const fallbackTitle = computed(() => {
  switch (normalizedType.value) {
    case 'warning':
      return t('alert.warning');
    case 'error':
      return t('alert.error');
    case 'success':
      return t('alert.success');
    default:
      return t('alert.info');
  }
});
</script>

<template>
  <v-alert
    class="md-alert"
    :type="normalizedType"
    :title="title || fallbackTitle"
    :icon="icon || undefined"
    variant="tonal"
    density="comfortable"
  >
    <slot />
  </v-alert>
</template>

<style scoped lang="scss">
.md-alert {
  margin: 1rem 0;
}
</style>

<i18n lang="yaml">
en:
  alert:
    info: Info
    warning: Warning
    error: Error
    success: Success
fr:
  alert:
    info: Information
    warning: Avertissement
    error: Erreur
    success: Succès
ja:
  alert:
    info: 情報
    warning: 警告
    error: エラー
    success: 成功
ko:
  alert:
    info: 정보
    warning: 경고
    error: 오류
    success: 성공
zhHans:
  alert:
    info: 信息
    warning: 警告
    error: 错误
    success: 成功
zhHant:
  alert:
    info: 資訊
    warning: 警告
    error: 錯誤
    success: 成功
</i18n>
