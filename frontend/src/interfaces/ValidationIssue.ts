import type { ValidationSeverity } from '@/types/ValidationSeverity';

/** Validation issue returned from backend checks. */
export interface ValidationIssue {
  /** Severity level of the issue. */
  severity: ValidationSeverity;
  /** Stable issue code for programmatic handling. */
  code: string;
  /** Human-readable issue description. */
  message: string;
}
