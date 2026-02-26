import type { TextureInfo } from './TextureInfo';

/** Conversion report returned after export. */
export interface ConversionReport {
  /** Estimated original height in centimeters. */
  estimated_height_cm: number;
  /** Requested output target height in centimeters. */
  target_height_cm: number;
  /** Computed scale multiplier applied to roots. */
  computed_scale_factor: number;
  /** Texture count from source model. */
  texture_count: number;
  /** Source texture count exceeding 1024px. */
  texture_over_1024_count: number;
  /** Texture metadata from exported output file. */
  output_texture_infos: TextureInfo[];
  /** Output texture count exceeding 1024px. */
  output_texture_over_1024_count: number;
}
