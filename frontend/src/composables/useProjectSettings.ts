import { invoke } from '@tauri-apps/api/core';

import type { ProjectSettings } from '@/interfaces/ProjectSettings';

/** Request payload for `save_project_settings_command`. */
interface SaveSettingsRequest {
  /** Absolute path where the settings JSON should be written. */
  path: string;
  /** Settings object to persist. */
  settings: ProjectSettings;
}

/** Request payload for `load_project_settings_command`. */
interface LoadSettingsRequest {
  /** Absolute path of the settings JSON file to read. */
  path: string;
}

/**
 * Composable that encapsulates Tauri commands for saving and loading project
 * settings to/from a JSON file on disk.
 *
 * @returns Functions to save and load {@link ProjectSettings}.
 */
export function useProjectSettings() {
  /**
   * Persist the current project settings to a file.
   *
   * @param path - Destination file path.
   * @param settings - Settings object to write.
   * @throws {Error} If the Tauri command fails.
   */
  const saveSettings = (path: string, settings: ProjectSettings): Promise<void> => {
    const request: SaveSettingsRequest = { path, settings };
    return invoke<void>('save_project_settings_command', { request });
  };

  /**
   * Load project settings from a file.
   *
   * @param path - Source file path.
   * @returns Deserialized {@link ProjectSettings}.
   * @throws {Error} If the Tauri command fails or the file is malformed.
   */
  const loadSettings = (path: string): Promise<ProjectSettings> => {
    const request: LoadSettingsRequest = { path };
    return invoke<ProjectSettings>('load_project_settings_command', { request });
  };

  return { saveSettings, loadSettings };
}
