import type { TextureResizeMethod } from '@/types/TextureResizeMethod';

/** Conversion options sent to backend analyze/convert commands. */
export interface ConvertOptions {
  /** Target avatar height in centimeters for Second Life. */
  target_height_cm: number;
  /** Additional manual scale multiplier. */
  manual_scale: number;
  /** Enables automatic texture downscaling with 1024px limit. */
  texture_auto_resize: boolean;
  /** Interpolation method used for texture downscaling. */
  texture_resize_method: TextureResizeMethod;
  /** Enable PBR (Physically Based Rendering) material support. */
  pbr_enabled: boolean;
  /**
   * Enable finger bone renaming/reconstruction/normalization during export.
   * When `false`, finger bones are left untouched (original VRM names,
   * hierarchy, and bind pose) since finger bone conversion is not yet
   * reliable for all sources. Sourced from `fingers.enabled`.
   */
  convert_fingers: boolean;
}
