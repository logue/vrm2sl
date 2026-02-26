export type ValidationSeverity = (typeof ValidationSeverity)[keyof typeof ValidationSeverity];

export const ValidationSeverity = {
  Error: 'Error',
  Warning: 'Warning',
  Info: 'Info'
} as const;
