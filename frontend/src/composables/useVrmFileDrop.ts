import { onBeforeUnmount, onMounted, ref } from 'vue';
import type { Ref } from 'vue';

import { getCurrentWindow } from '@tauri-apps/api/window';

import { useFileSystem } from '@/composables/useFileSystem';
import { parseGlbJsonChunk } from '@/composables/useVrmFile';

type DroppedFileError = 'unsupported_extension' | 'invalid_vrm' | 'unsupported_vrm_version';

/**
 * Provides window-level VRM drag-and-drop handling.
 *
 * - Validates the dropped file (extension, GLB magic, VRM version).
 * - Updates `inputPath` only when the file is a supported VRM 1.0 file.
 * - Exposes reactive state for the drop overlay and warning dialog.
 *
 * i18n key names used for the warning dialog are returned as `dropWarningTitleKey`
 * and `dropWarningMessageKey` so the caller can pass them directly to `t()`.
 */
export function useVrmFileDrop(inputPath: Ref<string>) {
  const fs = useFileSystem();

  const isDropActive = ref(false);
  const showDropWarningDialog = ref(false);
  const dropWarningTitleKey = ref('drop_invalid_title');
  const dropWarningMessageKey = ref('drop_invalid_message');

  let unlistenDragDrop: (() => void) | null = null;

  const openDropWarningDialog = (error: DroppedFileError) => {
    if (error === 'unsupported_vrm_version') {
      dropWarningTitleKey.value = 'vrm0_error_title';
      dropWarningMessageKey.value = 'vrm0_error_message';
    } else if (error === 'unsupported_extension') {
      dropWarningTitleKey.value = 'drop_unsupported_ext_title';
      dropWarningMessageKey.value = 'drop_unsupported_ext_message';
    } else {
      dropWarningTitleKey.value = 'drop_invalid_title';
      dropWarningMessageKey.value = 'drop_invalid_message';
    }
    showDropWarningDialog.value = true;
  };

  const validateDroppedVrmFile = async (path: string): Promise<DroppedFileError | null> => {
    if (!path.toLowerCase().endsWith('.vrm')) {
      return 'unsupported_extension';
    }

    try {
      const bytes = await fs.readFileContents(path);
      const json = parseGlbJsonChunk(bytes);
      if (!json) {
        return 'invalid_vrm';
      }

      const extensions = (json.extensions ?? {}) as Record<string, unknown>;
      const hasVrm1 = Boolean(extensions.VRMC_vrm);
      const hasVrm0 = Boolean(extensions.VRM);

      if (!hasVrm1 && hasVrm0) {
        return 'unsupported_vrm_version';
      }
      if (!hasVrm1) {
        return 'invalid_vrm';
      }
      return null;
    } catch {
      return 'invalid_vrm';
    }
  };

  const applyDroppedInputPath = async (paths: string[]) => {
    const firstPath = paths[0];
    if (!firstPath) {
      return;
    }

    const validationError = await validateDroppedVrmFile(firstPath);
    if (validationError) {
      openDropWarningDialog(validationError);
      return;
    }

    inputPath.value = firstPath;
  };

  onMounted(async () => {
    try {
      unlistenDragDrop = await getCurrentWindow().onDragDropEvent(({ payload }) => {
        if (payload.type === 'enter' || payload.type === 'over') {
          isDropActive.value = true;
          return;
        }

        isDropActive.value = false;
        if (payload.type === 'drop') {
          void applyDroppedInputPath(payload.paths);
        }
      });
    } catch (error) {
      console.error('Failed to register drag and drop events:', error);
    }
  });

  onBeforeUnmount(() => {
    if (unlistenDragDrop) {
      unlistenDragDrop();
      unlistenDragDrop = null;
    }
  });

  return {
    isDropActive,
    showDropWarningDialog,
    dropWarningTitleKey,
    dropWarningMessageKey
  };
}
