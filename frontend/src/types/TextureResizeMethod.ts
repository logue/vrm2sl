export type TextureResizeMethod = (typeof TextureResizeMethod)[keyof typeof TextureResizeMethod];

export const TextureResizeMethod = {
  Nearest: 'Nearest',
  Bilinear: 'Bilinear',
  Bicubic: 'Bicubic',
  Gaussian: 'Gaussian',
  Lanczos3: 'Lanczos3'
} as const;
