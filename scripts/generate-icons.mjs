// Generates the FinEnv application icons without external dependencies.
// Run: node scripts/generate-icons.mjs
import { deflateSync } from 'node:zlib';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const outDir = join(root, 'src-tauri', 'icons');
mkdirSync(outDir, { recursive: true });
mkdirSync(join(root, 'frontend', 'public'), { recursive: true });

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function png(size) {
  const raw = Buffer.alloc(size * (size * 4 + 1));
  const cx = size / 2;
  const cy = size / 2;
  for (let y = 0; y < size; y++) {
    const row = y * (size * 4 + 1);
    raw[row] = 0;
    for (let x = 0; x < size; x++) {
      const i = row + 1 + x * 4;
      const nx = (x - cx) / size;
      const ny = (y - cy) / size;
      const radius = Math.sqrt(nx * nx + ny * ny);
      const t = (x / size + y / size) / 2;
      let r = Math.round(22 + 20 * t);
      let g = Math.round(93 + 55 * t);
      let b = Math.round(255 - 20 * t);
      let a = 255;
      if (radius > 0.49) a = 0;
      // rounded square mask
      const margin = size * 0.06;
      const corner = size * 0.2;
      const dx = Math.min(x - margin, size - margin - x);
      const dy = Math.min(y - margin, size - margin - y);
      if (dx < 0 || dy < 0) a = 0;
      else if (dx < corner && dy < corner) {
        const d = Math.hypot(corner - dx, corner - dy);
        if (d > corner) a = 0;
      }
      // fingerprint arcs
      const fx = (x - cx) / size;
      const fy = (y - cy) / size;
      const dist = Math.sqrt(fx * fx + fy * fy);
      const angle = Math.atan2(fy, fx);
      const arc = Math.abs(dist - 0.24) < 0.035 && Math.abs(angle) < 1.2;
      const arc2 = Math.abs(dist - 0.32) < 0.03 && Math.abs(angle) < 1.0;
      const arc3 = Math.abs(dist - 0.40) < 0.025 && Math.abs(angle) < 0.75;
      const dot = dist < 0.07;
      if (arc || arc2 || arc3 || dot) {
        r = 255; g = 255; b = 255;
      }
      raw[i] = r; raw[i + 1] = g; raw[i + 2] = b; raw[i + 3] = a;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function ico(pngBuffer) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(1, 4);
  const entry = Buffer.alloc(16);
  entry[0] = 0;
  entry[1] = 0;
  entry[2] = 0;
  entry[3] = 0;
  entry.writeUInt16LE(1, 4);
  entry.writeUInt16LE(32, 6);
  entry.writeUInt32LE(pngBuffer.length, 8);
  entry.writeUInt32LE(22, 12);
  return Buffer.concat([header, entry, pngBuffer]);
}

function icns(entries) {
  const chunks = entries.map(([type, data]) => {
    const header = Buffer.alloc(8);
    header.write(type, 0, 4, 'ascii');
    header.writeUInt32BE(data.length + 8, 4);
    return Buffer.concat([header, data]);
  });
  const body = Buffer.concat(chunks);
  const header = Buffer.alloc(8);
  header.write('icns', 0, 4, 'ascii');
  header.writeUInt32BE(body.length + 8, 4);
  return Buffer.concat([header, body]);
}

const p32 = png(32);
const p128 = png(128);
const p256 = png(256);
const p512 = png(512);
writeFileSync(join(outDir, '32x32.png'), p32);
writeFileSync(join(outDir, '128x128.png'), p128);
writeFileSync(join(outDir, '128x128@2x.png'), p256);
writeFileSync(join(outDir, 'icon.png'), p256);
writeFileSync(join(outDir, 'icon.ico'), ico(p256));
writeFileSync(join(outDir, 'icon.icns'), icns([
  ['ic07', p128],
  ['ic08', p256],
  ['ic09', p512],
]));
writeFileSync(join(root, 'frontend', 'public', 'logo.png'), p128);
writeFileSync(join(root, 'frontend', 'public', 'favicon.png'), p32);
console.log('icons generated');
