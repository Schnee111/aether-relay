import crypto from 'node:crypto';

/**
 * Constant-time cryptographic signature comparison.
 * Hashes both input strings into 32-byte SHA-256 digests before calling crypto.timingSafeEqual.
 * This completely prevents:
 * 1. Timing attack vulnerabilities (constant-time execution regardless of match position).
 * 2. Buffer length leakage (both digests are guaranteed to be exactly 32 bytes).
 */
export function compareSignatures(expected: string, actual: string): boolean {
  if (typeof expected !== 'string' || typeof actual !== 'string') {
    return false;
  }
  const hashExpected = crypto.createHash('sha256').update(expected).digest();
  const hashActual = crypto.createHash('sha256').update(actual).digest();
  return crypto.timingSafeEqual(hashExpected, hashActual);
}
