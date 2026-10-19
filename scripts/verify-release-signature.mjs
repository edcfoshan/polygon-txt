import { createHash, createPublicKey, verify } from 'node:crypto';
import { readFileSync } from 'node:fs';

export function verifyReleaseSignature(file, signature, publicKey) {
  const publicLines = Buffer.from(publicKey, 'base64').toString('utf8').trim().split(/\r?\n/);
  const pub = Buffer.from(publicLines[1], 'base64');
  const text = signature.startsWith('untrusted comment:') ? signature : Buffer.from(signature, 'base64').toString('utf8');
  const lines = text.trim().split(/\r?\n/);
  const sig = Buffer.from(lines[1], 'base64');
  if (pub.length !== 42 || sig.length !== 74 || !pub.subarray(2, 10).equals(sig.subarray(2, 10))) {
    throw new Error(`Invalid signing key/signature: ${file}`);
  }
  const key = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), pub.subarray(10)]), format: 'der', type: 'spki' });
  const raw = readFileSync(file);
  const algorithm = sig.subarray(0, 2).toString('ascii');
  if (!['ED', 'Ed'].includes(algorithm)) throw new Error(`Unknown signature algorithm: ${algorithm}`);
  const data = algorithm === 'ED' ? createHash('blake2b512').update(raw).digest() : raw;
  if (!verify(null, data, key, sig.subarray(10))) throw new Error(`Signature verification failed: ${file}`);
  if (!lines[2]?.startsWith('trusted comment: ')
      || !verify(null, Buffer.concat([sig.subarray(10), Buffer.from(lines[2].slice(17))]), key, Buffer.from(lines[3], 'base64'))) {
    throw new Error(`Trusted comment verification failed: ${file}`);
  }
}
