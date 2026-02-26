import type { TextureInfo } from './TextureInfo';
import type { UploadFeeEstimate } from './UploadFeeEstimate';
import type { ValidationIssue } from './ValidationIssue';

/** Analyze-only report for input VRM. */
export interface AnalysisReport {
  /** Model name extracted from metadata. */
  model_name: string;
  /** Optional model author name. */
  author?: string;
  /** Estimated avatar height in centimeters. */
  estimated_height_cm: number;
  /** Total node/bone count in source. */
  bone_count: number;
  /** Total mesh count in source. */
  mesh_count: number;
  /** Total vertex count in source. */
  total_vertices: number;
  /** Total polygon count in source. */
  total_polygons: number;
  /** Source-to-target mapped bone pairs. */
  mapped_bones: [string, string][];
  /** Required humanoid bones that are missing. */
  missing_required_bones: string[];
  /** Texture metadata from source model. */
  texture_infos: TextureInfo[];
  /** Upload fee estimate from source textures. */
  fee_estimate: UploadFeeEstimate;
  /** Validation issues found during analyze step. */
  issues: ValidationIssue[];
}
