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
}
