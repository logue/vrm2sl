import { invoke } from '@tauri-apps/api/core';

import type { AnalysisReport } from '@/interfaces/AnalysisReport';
import type { ConversionReport } from '@/interfaces/ConversionReport';
import type { ConvertOptions } from '@/interfaces/ConvertOptions';

/** Request payload for `analyze_vrm_command`. */
interface AnalyzeRequest {
  /** Absolute path to the source VRM file. */
  input_path: string;
  /** Conversion options used during analysis. */
  options: ConvertOptions;
  /** Whether to emit a desktop notification on completion. */
  notify_on_complete: boolean;
}

/** Request payload for `convert_vrm_command`. */
interface ConvertRequest {
  /** Absolute path to the source VRM file. */
  input_path: string;
  /** Absolute path where the GLB output should be written. */
  output_path: string;
  /** Conversion options to apply during export. */
  options: ConvertOptions;
  /** Whether to emit a desktop notification on completion. */
  notify_on_complete: boolean;
}

/**
 * Composable that encapsulates Tauri commands for VRM analysis and conversion.
 *
 * @returns Functions to analyze and convert a VRM model, and to retrieve the
 *   backend application version string.
 */
export function useConversion() {
  /**
   * Analyze a VRM file and return validation/analysis metadata.
   *
   * @param request - Analyze request parameters.
   * @returns Resolved {@link AnalysisReport}.
   * @throws {Error} If the Tauri command fails.
   */
  const analyzeVrm = (request: AnalyzeRequest): Promise<AnalysisReport> => {
    return invoke<AnalysisReport>('analyze_vrm_command', { request });
  };

  /**
   * Convert a VRM file to Second Life GLB format.
   *
   * @param request - Conversion request parameters.
   * @returns Resolved {@link ConversionReport}.
   * @throws {Error} If the Tauri command fails.
   */
  const convertVrm = (request: ConvertRequest): Promise<ConversionReport> => {
    return invoke<ConversionReport>('convert_vrm_command', { request });
  };

  /**
   * Retrieve the backend application version string.
   *
   * @returns Semantic version string (e.g. `"0.8.3-alpha.3"`).
   * @throws {Error} If the Tauri command fails.
   */
  const getAppVersion = (): Promise<string> => {
    return invoke<string>('get_app_version');
  };

  return { analyzeVrm, convertVrm, getAppVersion };
}
