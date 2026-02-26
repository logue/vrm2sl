import type { ConvertOptions } from './ConvertOptions';
import type { FaceSettings } from './FaceSettings';
import type { FingerSettings } from './FingerSettings';

/** Serializable project settings persisted as JSON. */
export interface ProjectSettings {
  /** Optional source file path. */
  input_path?: string;
  /** Optional output file path. */
  output_path?: string;
  /** Target avatar height in centimeters. */
  target_height_cm: number;
  /** Additional manual scale multiplier. */
  manual_scale: number;
  /** Enables automatic texture downscaling. */
  texture_auto_resize: boolean;
  /** Interpolation method used for texture downscaling. */
  texture_resize_method: ConvertOptions['texture_resize_method'];
  /** Face-related settings. */
  face: FaceSettings;
  /** Finger-related settings. */
  fingers: FingerSettings;
}
