import { open, save } from '@tauri-apps/plugin-dialog';
import { exists, readFile, writeFile } from '@tauri-apps/plugin-fs';

export function useFileSystem() {
  /**
   * Open file selection dialog
   */
  const selectFiles = async (options?: {
    multiple?: boolean;
    filters?: { name: string; extensions: string[] }[];
  }) => {
    return await open({
      multiple: options?.multiple ?? false,
      filters: options?.filters
    });
  };

  /**
   * Open folder selection dialog
   */
  const selectFolder = async () => {
    return await open({
      directory: true
    });
  };

  /**
   * Open save file dialog
   */
  const saveFile = async (options?: {
    defaultPath?: string;
    filters?: { name: string; extensions: string[] }[];
  }) => {
    return await save({
      defaultPath: options?.defaultPath,
      filters: options?.filters
    });
  };

  /**
   * Read file contents
   */
  const readFileContents = async (path: string) => {
    return await readFile(path);
  };

  /**
   * Write file contents
   */
  const writeFileContents = async (path: string, data: Uint8Array) => {
    return await writeFile(path, data);
  };

  /**
   * Check if file exists
   */
  const fileExists = async (path: string) => {
    return await exists(path);
  };

  return {
    selectFiles,
    selectFolder,
    saveFile,
    readFileContents,
    writeFileContents,
    fileExists
  };
}
