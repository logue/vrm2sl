<script setup lang="ts">
import { useConfigStore } from '@/store';
import { computed, toRef } from 'vue';
import { useI18n } from 'vue-i18n';

import type { AnalysisReport, ConversionReport } from '@/interfaces';

const props = defineProps<{
  analysis: AnalysisReport | null;
  conversion: ConversionReport | null;
}>();

const { t } = useI18n();
const configStore = useConfigStore();
const textureAutoResize = toRef(configStore, 'textureAutoResize');

const outputMaxTextureDimension = computed(() => {
  if (!props.conversion || props.conversion.output_texture_infos.length === 0) {
    return 0;
  }
  return props.conversion.output_texture_infos.reduce((max, texture) => {
    return Math.max(max, texture.width, texture.height);
  }, 0);
});

const outputTextureSizePreview = computed(() => {
  if (!props.conversion) {
    return '';
  }
  return props.conversion.output_texture_infos
    .slice(0, 5)
    .map(texture => `#${texture.index}: ${texture.width}x${texture.height}`)
    .join(', ');
});

const resizedTextureCount = computed(() => {
  if (!props.conversion) {
    return 0;
  }
  return Math.max(
    0,
    props.conversion.texture_over_1024_count - props.conversion.output_texture_over_1024_count
  );
});
</script>

<template>
  <v-card :title="t('validation')" prepend-icon="mdi-alert-circle">
    <v-card-text>
      <v-list v-if="analysis" density="compact">
        <v-list-item>
          <v-list-item-title>
            {{
              t('label_model_author', {
                model: analysis.model_name,
                author: analysis.author || 'Unknown'
              })
            }}
          </v-list-item-title>
        </v-list-item>
        <v-list-item>
          <v-list-item-title>
            {{
              t('label_height_mesh_bone', {
                height: analysis.estimated_height_cm.toFixed(2),
                mesh: analysis.mesh_count,
                bone: analysis.bone_count
              })
            }}
          </v-list-item-title>
        </v-list-item>
        <v-list-item>
          <v-list-item-title>
            {{
              t('label_vertices_polygons', {
                vertices: analysis.total_vertices,
                polygons: analysis.total_polygons
              })
            }}
          </v-list-item-title>
        </v-list-item>
        <v-list-item>
          <v-list-item-title>
            {{
              t('label_tex_cost', {
                before: analysis.fee_estimate.before_linden_dollar,
                after: analysis.fee_estimate.after_resize_linden_dollar,
                reduction: analysis.fee_estimate.reduction_percent
              })
            }}
          </v-list-item-title>
        </v-list-item>
        <v-list-item>
          <v-list-item-title>
            {{ t('label_tex_policy') }}
            {{ textureAutoResize ? t('policy_aggressive_short') : t('policy_conservative_short') }}
          </v-list-item-title>
        </v-list-item>
        <v-list-item v-if="conversion">
          <v-list-item-title>
            {{
              t('label_converted_tex', {
                count: conversion.output_texture_infos.length,
                max: outputMaxTextureDimension,
                over: conversion.output_texture_over_1024_count
              })
            }}
          </v-list-item-title>
        </v-list-item>
        <v-list-item v-if="conversion">
          <v-list-item-title>
            {{ t('label_resized_count', { count: resizedTextureCount }) }}
            <span v-if="outputTextureSizePreview">
              {{ t('label_size_preview', { preview: outputTextureSizePreview }) }}
            </span>
          </v-list-item-title>
        </v-list-item>
        <v-list-item v-for="issue in analysis.issues" :key="`${issue.code}-${issue.message}`">
          <v-list-item-title>{{ `[${issue.severity}] ${issue.message}` }}</v-list-item-title>
        </v-list-item>
      </v-list>
      <v-alert v-else type="info" variant="tonal">{{ t('not_analyzed') }}</v-alert>
    </v-card-text>
  </v-card>
</template>

<i18n lang="yaml">
en:
  validation: Validation
  label_model_author: 'Model: {model} / Author: {author}'
  label_height_mesh_bone: 'Height (est.): {height}cm / Mesh: {mesh} / Bone: {bone}'
  label_vertices_polygons: 'Vertices: {vertices} / Polygons: {polygons}'
  label_tex_cost: 'Texture cost: {before}L$ -> {after}L$ ({reduction}%)'
  label_tex_policy: 'Texture Resize Policy:'
  policy_aggressive_short: '>=1025px -> 1024px (incl. >=2049px)'
  policy_conservative_short: 'only >=2049px -> 2048px (keep 1025-2048px)'
  label_converted_tex: 'Post-conv. textures: {count} / Max side: {max}px / Over 1024px: {over}'
  label_resized_count: 'Resized texture count (est.): {count}'
  label_size_preview: '/ Size preview: {preview}'
  not_analyzed: Not yet analyzed.
fr:
  validation: Validation
  label_model_author: 'Modèle: {model} / Auteur: {author}'
  label_height_mesh_bone: 'Hauteur (est.): {height}cm / Maillage: {mesh} / Os: {bone}'
  label_vertices_polygons: 'Sommets: {vertices} / Polygones: {polygons}'
  label_tex_cost: 'Coût texture: {before}L$ -> {after}L$ ({reduction}%)'
  label_tex_policy: 'Politique de redimensionnement:'
  policy_aggressive_short: '>=1025px -> 1024px (incl. >=2049px)'
  policy_conservative_short: 'uniquement >=2049px -> 2048px (garder 1025-2048px)'
  label_converted_tex: 'Textures (post-conv.): {count} / Côté max: {max}px / >1024px: {over}'
  label_resized_count: 'Textures redimensionnées (est.): {count}'
  label_size_preview: '/ Aperçu taille: {preview}'
  not_analyzed: Non analysé.
ja:
  validation: バリデーション
  label_model_author: 'モデル: {model} / 作者: {author}'
  label_height_mesh_bone: '身長推定: {height}cm / メッシュ: {mesh} / ボーン: {bone}'
  label_vertices_polygons: '頂点: {vertices} / ポリゴン: {polygons}'
  label_tex_cost: 'テクスチャ費用: {before}L$ → {after}L$ ({reduction}%)'
  label_tex_policy: 'テクスチャ縮小ポリシー:'
  policy_aggressive_short: '1025px以上→1024px（2049px以上を含む）'
  policy_conservative_short: '2049px以上のみ→2048px（1025〜2048pxは維持）'
  label_converted_tex: '変換後テクスチャ: {count}枚 / 最大辺: {max}px / 1024px超過: {over}'
  label_resized_count: '縮小適用テクスチャ数(推定): {count}'
  label_size_preview: '/ 縮小後サイズ例: {preview}'
  not_analyzed: 未解析です。
ko:
  validation: 유효성 검사
  label_model_author: '모델: {model} / 작성자: {author}'
  label_height_mesh_bone: '신장 추정: {height}cm / 메시: {mesh} / 본: {bone}'
  label_vertices_polygons: '정점: {vertices} / 폴리곤: {polygons}'
  label_tex_cost: '텍스처 비용: {before}L$ → {after}L$ ({reduction}%)'
  label_tex_policy: '텍스처 축소 정책:'
  policy_aggressive_short: '1025px 이상→1024px（2049px 이상 포함）'
  policy_conservative_short: '2049px 이상만→2048px（1025〜2048px 유지）'
  label_converted_tex: '변환 후 텍스처: {count}장 / 최대 변: {max}px / 1024px 초과: {over}'
  label_resized_count: '축소 적용 텍스처 수(추정): {count}'
  label_size_preview: '/ 축소 후 크기 예시: {preview}'
  not_analyzed: 아직 분석되지 않았습니다.
zhHant:
  validation: 驗證
  label_model_author: '模型: {model} / 作者: {author}'
  label_height_mesh_bone: '身高推測: {height}cm / 網格: {mesh} / 骨骼: {bone}'
  label_vertices_polygons: '頂點: {vertices} / 多邊形: {polygons}'
  label_tex_cost: '貼圖費用: {before}L$ → {after}L$ ({reduction}%)'
  label_tex_policy: '貼圖縮小政策:'
  policy_aggressive_short: '>=1025px→1024px（含 >=2049px）'
  policy_conservative_short: '僅 >=2049px→2048px（保留 1025〜2048px）'
  label_converted_tex: '轉換後貼圖: {count}張 / 最大邊: {max}px / 超過1024px: {over}'
  label_resized_count: '縮小貼圖數量(推測): {count}'
  label_size_preview: '/ 縮小後尺寸示例: {preview}'
  not_analyzed: 尚未分析。
zhHans:
  validation: 验证
  label_model_author: '模型: {model} / 作者: {author}'
  label_height_mesh_bone: '身高估算: {height}cm / 网格: {mesh} / 骨骼: {bone}'
  label_vertices_polygons: '顶点: {vertices} / 多边形: {polygons}'
  label_tex_cost: '贴图费用: {before}L$ → {after}L$ ({reduction}%)'
  label_tex_policy: '贴图缩小策略:'
  policy_aggressive_short: '>=1025px→1024px（含 >=2049px）'
  policy_conservative_short: '仅 >=2049px→2048px（保留 1025〜2048px）'
  label_converted_tex: '转换后贴图: {count}张 / 最大边: {max}px / 超过1024px: {over}'
  label_resized_count: '缩小贴图数量(估算): {count}'
  label_size_preview: '/ 缩小后尺寸示例: {preview}'
  not_analyzed: 尚未分析。
</i18n>
