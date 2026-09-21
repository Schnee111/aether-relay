import crypto from 'node:crypto';
import { compareSignatures } from './index.js';
import type { ProviderType } from '../db/schema.js';

export interface VerificationContext {
  provider: ProviderType;
  secret: string;
  rawBody: Buffer;
  headers: Record<string, string | string[] | undefined>;
  jsonPayload?: any;
}

export interface VerificationResult {
  isValid: boolean;
  reason?: string;
}

export function verifyWebhookSignature(ctx: VerificationContext): VerificationResult {
  const { provider, secret, rawBody, headers, jsonPayload } = ctx;

  switch (provider) {
    case 'github': {
      const headerVal = headers['x-hub-signature-256'];
      if (!headerVal || typeof headerVal !== 'string') {
        return { isValid: false, reason: 'Missing X-Hub-Signature-256 header' };
      }
      const expected = 'sha256=' + crypto.createHmac('sha256', secret).update(rawBody).digest('hex');
      const isValid = compareSignatures(expected, headerVal);
      return { isValid, reason: isValid ? undefined : 'GitHub HMAC signature mismatch' };
    }

    case 'stripe': {
      const headerVal = headers['stripe-signature'];
      if (!headerVal || typeof headerVal !== 'string') {
        return { isValid: false, reason: 'Missing Stripe-Signature header' };
      }

      // Parse t=...,v1=...
      const parts = headerVal.split(',');
      let timestamp = '';
      let v1Sig = '';

      for (const part of parts) {
        const [k, v] = part.split('=');
        if (k === 't') timestamp = v;
        if (k === 'v1') v1Sig = v;
      }

      if (!timestamp || !v1Sig) {
        return { isValid: false, reason: 'Malformed Stripe-Signature header' };
      }

      // Replay attack tolerance check (300 seconds)
      const nowSec = Math.floor(Date.now() / 1000);
      const sentSec = parseInt(timestamp, 10);
      if (isNaN(sentSec) || Math.abs(nowSec - sentSec) > 300) {
        return { isValid: false, reason: 'Stripe timestamp drift exceeds 300s window (replay detected)' };
      }

      const signedPayload = `${timestamp}.${rawBody.toString('utf-8')}`;
      const expected = crypto.createHmac('sha256', secret).update(signedPayload).digest('hex');
      const isValid = compareSignatures(expected, v1Sig);
      return { isValid, reason: isValid ? undefined : 'Stripe signature mismatch' };
    }

    case 'midtrans': {
      if (!jsonPayload || typeof jsonPayload !== 'object') {
        return { isValid: false, reason: 'Missing JSON payload for Midtrans verification' };
      }
      const orderId = jsonPayload.order_id;
      const statusCode = jsonPayload.status_code;
      const grossAmount = jsonPayload.gross_amount;
      const incomingSignature = jsonPayload.signature_key;

      if (!incomingSignature || !orderId || !statusCode || !grossAmount) {
        return { isValid: false, reason: 'Missing required Midtrans signature fields' };
      }

      const signatureRaw = `${orderId}${statusCode}${grossAmount}${secret}`;
      const expected = crypto.createHash('sha512').update(signatureRaw).digest('hex');
      const isValid = compareSignatures(expected, incomingSignature);
      return { isValid, reason: isValid ? undefined : 'Midtrans SHA512 signature mismatch' };
    }

    case 'generic': {
      const headerVal = headers['x-signature-sha256'];
      if (!headerVal || typeof headerVal !== 'string') {
        return { isValid: false, reason: 'Missing X-Signature-SHA256 header' };
      }
      const expected = crypto.createHmac('sha256', secret).update(rawBody).digest('hex');
      const cleaned = headerVal.startsWith('sha256=') ? headerVal.slice(7) : headerVal;
      const isValid = compareSignatures(expected, cleaned);
      return { isValid, reason: isValid ? undefined : 'Generic HMAC signature mismatch' };
    }

    default:
      return { isValid: false, reason: `Unsupported provider: ${provider}` };
  }
}
