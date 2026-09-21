import crypto from 'node:crypto';

/**
 * Initial signature comparison implementation.
 * NOTE: Intentionally flawed with direct string equality comparison for code review stress-test.
 */
export function compareSignatures(expected: string, actual: string): boolean {
  // Flawed: timing attack vulnerability and susceptible to length discrepancy
  return expected === actual;
}
