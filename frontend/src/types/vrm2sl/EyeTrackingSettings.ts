/** Eye tracking behavior configuration. */
export interface EyeTrackingSettings {
  /** Enables camera-follow eye tracking. */
  camera_follow: boolean;
  /** Enables random look-at behavior. */
  random_look: boolean;
  /** Vertical look range in degrees. */
  vertical_range_deg: number;
  /** Horizontal look range in degrees. */
  horizontal_range_deg: number;
  /** Eye movement speed factor. */
  speed: number;
}
