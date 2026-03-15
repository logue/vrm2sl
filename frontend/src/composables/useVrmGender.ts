import { readFile } from '@tauri-apps/plugin-fs';

export type MotionMode = 'idle' | 'walk';
export type AvatarGender = 'female' | 'male' | 'unknown';

/**
 * Parse the JSON chunk from a GLB file and return it as a plain object.
 * Returns null when the bytes are not a valid GLB or the JSON cannot be decoded.
 */
export function parseGlbJsonChunk(bytes: Uint8Array): Record<string, unknown> | null {
  if (bytes.length < 20) {
    return null;
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const magic = view.getUint32(0, true);
  // ASCII "glTF" in little-endian.
  if (magic !== 0x46546c67) {
    return null;
  }

  const jsonChunkLength = view.getUint32(12, true);
  const jsonChunkType = view.getUint32(16, true);
  // JSON chunk type ASCII "JSON" in little-endian.
  if (jsonChunkType !== 0x4e4f534a || 20 + jsonChunkLength > bytes.length) {
    return null;
  }

  const jsonBytes = bytes.slice(20, 20 + jsonChunkLength);
  const decoder = new TextDecoder();
  try {
    return JSON.parse(decoder.decode(jsonBytes)) as Record<string, unknown>;
  } catch {
    return null;
  }
}

/**
 * Read VRM metadata from `path` and infer the avatar's gender.
 * Falls back to `'unknown'` when the metadata is absent or unreadable.
 */
export async function detectGenderFromVrm(path: string): Promise<AvatarGender> {
  if (!path) {
    return 'unknown';
  }

  try {
    const bytes = await readFile(path);
    const json = parseGlbJsonChunk(bytes);
    if (!json) {
      return 'unknown';
    }

    const extensions = (json.extensions ?? {}) as Record<string, unknown>;
    const vrm0Meta =
      ((extensions.VRM as Record<string, unknown> | undefined)?.meta as
        | Record<string, unknown>
        | undefined) ?? {};
    const vrm1Meta =
      ((extensions.VRMC_vrm as Record<string, unknown> | undefined)?.meta as
        | Record<string, unknown>
        | undefined) ?? {};

    const raw =
      (vrm0Meta.sex as string | undefined) ??
      (vrm0Meta.gender as string | undefined) ??
      (vrm1Meta.sex as string | undefined) ??
      (vrm1Meta.gender as string | undefined) ??
      '';
    const value = raw.toLowerCase();

    if (value.includes('female') || value.includes('woman') || value.includes('girl')) {
      return 'female';
    }
    if (value.includes('male') || value.includes('man') || value.includes('boy')) {
      return 'male';
    }
  } catch {
    // Fall through to unknown when metadata cannot be parsed.
  }

  return 'unknown';
}

/**
 * Resolve the BVH motion file path for a given mode and gender combination.
 */
export function resolveMotionPath(mode: MotionMode, gender: AvatarGender): string {
  if (mode === 'walk') {
    if (gender === 'female') {
      return '/animations/avatar_female_walk.bvh';
    }
    return '/animations/avatar_walk.bvh';
  }

  // Use multi-frame stand so preview clearly animates.
  return '/animations/avatar_stand_1.bvh';
}
