import { createHash, createPrivateKey, createPublicKey, sign, verify } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { copyFile, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const [, , source, destination] = process.argv;
const privateSeed = process.env.DYLAN_SIGNING_KEY ?? '';
delete process.env.DYLAN_SIGNING_KEY;

if (!/^[a-f0-9]{64}$/i.test(privateSeed)) {
  throw new Error('DYLAN_SIGNING_KEY must contain a 32-byte Ed25519 seed in hexadecimal.');
}

const files = new Map();
const packageHashes = new Map();
for (const [channel, installer] of [['win-x64', /Setup\.exe$/i], ['linux-x64', /\.AppImage$/]]) {
  const directory = join(source, channel);
  const entries = await readdir(directory, { withFileTypes: true });
  const feed = `releases.${channel}.json`;
  const index = JSON.parse(await readFile(join(directory, feed), 'utf8'));
  const names = entries.map((entry) => entry.name);
  if (names.filter((name) => installer.test(name)).length !== 1 ||
      !Array.isArray(index.Assets) || !index.Assets.some(({ Type }) => Type === 'Full') ||
      !index.Assets.every(({ FileName, SHA256 }) =>
        names.includes(FileName) && /\.nupkg$/.test(FileName) && /^[a-f0-9]{64}$/i.test(SHA256))) {
    throw new Error(`Missing installer or update packages for ${channel}.`);
  }

  for (const { FileName, SHA256 } of index.Assets) {
    packageHashes.set(FileName, SHA256.toLowerCase());
  }

  for (const entry of entries) {
    const name = entry.name;
    if (!entry.isFile() || !/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(name) || files.has(name) ||
        (name !== feed && !installer.test(name) && !/\.nupkg$/.test(name))) {
      throw new Error(`Unexpected or duplicate release file: ${name}`);
    }
    files.set(name, join(directory, name));
  }
}

await mkdir(destination, { recursive: true });
const checksums = [];
for (const name of [...files.keys()].sort()) {
  const artifact = join(destination, name);
  await copyFile(files.get(name), artifact);
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(artifact)) {
    hash.update(chunk);
  }
  const digest = hash.digest('hex');
  if (name.endsWith('.nupkg') && digest !== packageHashes.get(name)) {
    throw new Error(`Package checksum does not match the Velopack feed: ${name}`);
  }
  checksums.push(`${digest}  ${name}`);
}

const manifest = Buffer.from(`${checksums.join('\n')}\n`);
// PKCS#8 encoding for a raw 32-byte Ed25519 seed (RFC 8410).
const keyBytes = Buffer.from(`302e020100300506032b657004220420${privateSeed}`, 'hex');
const privateKey = createPrivateKey({ key: keyBytes, format: 'der', type: 'pkcs8' });
keyBytes.fill(0);
const publicKey = createPublicKey(privateKey);
const signature = sign(null, manifest, privateKey);
if (!verify(null, manifest, publicKey, signature)) {
  throw new Error('The generated Ed25519 signature failed verification.');
}

await writeFile(join(destination, 'SHA256SUMS.txt'), manifest);
await writeFile(join(destination, 'SHA256SUMS.txt.sig'), signature);
await writeFile(join(destination, 'DYLAN_SIGNING_PUBLIC_KEY.pem'), publicKey.export({
  format: 'pem',
  type: 'spki',
}));
