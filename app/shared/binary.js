// These are explicitly declared native host adapters, not a Node dependency.
import { createHash } from 'crypto';
import { readFile } from 'fs/promises';
export function installReceipt(text) {
  if (text === null || text.length > 1048576) throw Error('install.json');
  const receipt = JSON.parse(text);
  const fields = ['v', 'install_id', 'files', 'dirs', 'keys'];
  if (!receipt || typeof receipt !== 'object' || Object.keys(receipt).length !== fields.length || fields.some(key => !Object.hasOwn(receipt, key))
      || receipt.v !== 2 || !/^[a-f0-9]{32}$/.test(receipt.install_id)) throw Error('install.json');
  return receipt;
}
export async function verifiedBinary(home, text, generation = null) {
  const receipt = installReceipt(text);
  if (generation !== null && receipt.install_id !== generation) throw Error('Installation changed');
  const entry = receipt.files?.['data/panorama/bin/panorama'];
  if (!entry || !/^[a-f0-9]{64}$/.test(entry.sha256) || entry.mode !== 493) throw Error('data/panorama/bin/panorama');
  const path = `${home}/data/panorama/bin/panorama`;
  // No bounded-read/stat API: the cap runs AFTER allocation, not before reading.
  const bytes = await readFile(path);
  if (!(bytes instanceof Uint8Array) || bytes.byteLength > 256 * 1024 * 1024
      || createHash('sha256').update(bytes).digest('hex') !== entry.sha256) throw Error(path);
  // The executable independently validates receipt/path/mode under the lifecycle
  // lock. Prehash supplies neither no-follow nor concurrent replacement proof.
  return path;
}
