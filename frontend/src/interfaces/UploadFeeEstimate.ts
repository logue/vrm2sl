/** Estimated upload cost report for textures. */
export interface UploadFeeEstimate {
  /** Estimated upload fee before resize. */
  before_linden_dollar: number;
  /** Estimated upload fee after resize. */
  after_resize_linden_dollar: number;
  /** Reduction ratio in percentage. */
  reduction_percent: number;
}
