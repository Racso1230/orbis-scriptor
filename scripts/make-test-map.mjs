// Generates an 8000x6000 PNG for the map performance check (no dependencies).
// Usage: node scripts/make-test-map.mjs [output.png]
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const W = 8000;
const H = 6000;
const out = process.argv[2] ?? "test-map-8000x6000.png";

// Raw RGB scanlines, each prefixed with filter byte 0. A coloured gradient
// with a grid keeps the PNG realistic (not trivially compressible to nothing).
const row = W * 3 + 1;
const raw = Buffer.alloc(row * H);
for (let y = 0; y < H; y++) {
  const base = y * row;
  raw[base] = 0;
  for (let x = 0; x < W; x++) {
    const i = base + 1 + x * 3;
    const grid = x % 250 === 0 || y % 250 === 0;
    const noise = ((x * 73856093) ^ (y * 19349663)) & 15;
    raw[i] = grid ? 40 : 90 + ((x * 120) / W) + noise;
    raw[i + 1] = grid ? 40 : 140 + ((y * 80) / H) + noise;
    raw[i + 2] = grid ? 40 : 100 + noise;
  }
}

const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(H, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 2; // colour type: RGB
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 6 })),
  chunk("IEND", Buffer.alloc(0)),
]);
writeFileSync(out, png);
console.log(`Wrote ${out} (${(png.length / 1e6).toFixed(1)} MB)`);
