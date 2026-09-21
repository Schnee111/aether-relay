import { describe, it, expect } from 'vitest';
import crypto from 'node:crypto';
import { verifyWebhookSignature } from '../../src/crypto/adapters.js';

describe('Cryptographic Signature Adapters', () => {
  const testSecret = 'whsec_test_secret_key_12345';

  it('validates GitHub webhook signature correctly', () => {
    const rawPayload = Buffer.from(JSON.stringify({ action: 'push', repository: 'Schnee111/aether-relay' }));
    const validHmac = 'sha256=' + crypto.createHmac('sha256', testSecret).update(rawPayload).digest('hex');

    const result = verifyWebhookSignature({
      provider: 'github',
      secret: testSecret,
      rawBody: rawPayload,
      headers: { 'x-hub-signature-256': validHmac },
    });
    expect(result.isValid).toBe(true);

    // Tampered payload
    const tamperedPayload = Buffer.from(JSON.stringify({ action: 'push', repository: 'Schnee111/tampered' }));
    const badResult = verifyWebhookSignature({
      provider: 'github',
      secret: testSecret,
      rawBody: tamperedPayload,
      headers: { 'x-hub-signature-256': validHmac },
    });
    expect(badResult.isValid).toBe(false);
  });

  it('validates Stripe signature with timestamp replay tolerance window', () => {
    const rawPayload = Buffer.from(JSON.stringify({ id: 'evt_123', type: 'charge.succeeded' }));
    const nowSec = Math.floor(Date.now() / 1000);
    const signedPayload = `${nowSec}.${rawPayload.toString('utf-8')}`;
    const v1Sig = crypto.createHmac('sha256', testSecret).update(signedPayload).digest('hex');

    const validHeader = `t=${nowSec},v1=${v1Sig}`;
    const result = verifyWebhookSignature({
      provider: 'stripe',
      secret: testSecret,
      rawBody: rawPayload,
      headers: { 'stripe-signature': validHeader },
    });
    expect(result.isValid).toBe(true);

    // Replay attack: timestamp expired (> 300s)
    const oldSec = nowSec - 350;
    const oldSignedPayload = `${oldSec}.${rawPayload.toString('utf-8')}`;
    const oldSig = crypto.createHmac('sha256', testSecret).update(oldSignedPayload).digest('hex');
    const expiredHeader = `t=${oldSec},v1=${oldSig}`;

    const replayResult = verifyWebhookSignature({
      provider: 'stripe',
      secret: testSecret,
      rawBody: rawPayload,
      headers: { 'stripe-signature': expiredHeader },
    });
    expect(replayResult.isValid).toBe(false);
    expect(replayResult.reason).toContain('replay detected');
  });

  it('validates Midtrans SHA512 signature correctly', () => {
    const orderId = 'ORDER-991';
    const statusCode = '200';
    const grossAmount = '150000.00';
    const rawSignature = `${orderId}${statusCode}${grossAmount}${testSecret}`;
    const expectedHash = crypto.createHash('sha512').update(rawSignature).digest('hex');

    const payload = {
      order_id: orderId,
      status_code: statusCode,
      gross_amount: grossAmount,
      signature_key: expectedHash,
    };

    const result = verifyWebhookSignature({
      provider: 'midtrans',
      secret: testSecret,
      rawBody: Buffer.from(JSON.stringify(payload)),
      headers: {},
      jsonPayload: payload,
    });
    expect(result.isValid).toBe(true);

    // Corrupt signature
    payload.signature_key = 'deadbeefdeadbeef';
    const badResult = verifyWebhookSignature({
      provider: 'midtrans',
      secret: testSecret,
      rawBody: Buffer.from(JSON.stringify(payload)),
      headers: {},
      jsonPayload: payload,
    });
    expect(badResult.isValid).toBe(false);
  });
});
