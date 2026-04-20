<script setup lang="ts">
defineProps<{
  title: string;
  message: string;
}>();

const model = defineModel<boolean>();
</script>

<template>
  <v-dialog v-model="model" max-width="480" persistent>
    <v-card :title="title" prepend-icon="mdi-alert-circle" color="error">
      <v-card-text>{{ message }}</v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn color="primary" variant="flat" text="OK" @click="model = false" />
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>
