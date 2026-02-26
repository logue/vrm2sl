import type { BlinkSettings } from './BlinkSettings';
import type { EyeTrackingSettings } from './EyeTrackingSettings';
import type { LipSyncSettings } from './LipSyncSettings';

/** Face-related runtime settings. */
export interface FaceSettings {
  /** Blink configuration. */
  blink: BlinkSettings;
  /** Lip sync configuration. */
  lip_sync: LipSyncSettings;
  /** Eye tracking configuration. */
  eye_tracking: EyeTrackingSettings;
}
