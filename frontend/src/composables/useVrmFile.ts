export type MotionMode = 'idle' | 'walk' | 'custom';
export type AvatarGender = 'female' | 'male';

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
 * Resolve the BVH motion file path for a given mode and optional gender.
 */
export function resolveMotionPath(
  mode: MotionMode,
  gender?: AvatarGender,
  customMotionPath?: string
): string {
  if (mode === 'custom') {
    return customMotionPath ?? '/animations/avatar_stand_1.bvh';
  }

  if (mode === 'walk') {
    if (gender === 'female') {
      return '/animations/avatar_female_walk.bvh';
    }
    return '/animations/avatar_walk.bvh';
  }

  // Use multi-frame stand so preview clearly animates.
  return '/animations/avatar_stand_1.bvh';
}
