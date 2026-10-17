// Generates a procedural fantasy map PNG (no dependencies), used for the sample world.
// Usage: node scripts/make-fantasy-map.mjs <width> <height> <out.png> [seed]
// The bundled sample map was made with: 2000 1333 map.png 7
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const W = Number(process.argv[2] ?? 3000);
const H = Number(process.argv[3] ?? 2000);
const out = process.argv[4] ?? "fantasy-map.png";
let seed = Number(process.argv[5] ?? 1337);

// Seeded RNG + gradient-free value noise on a lattice.
function rng() {
  seed = (seed * 1664525 + 1013904223) >>> 0;
  return seed / 4294967296;
}
const P = 512;
const lattice = new Float32Array(P * P).map(() => rng());
const fade = (t) => t * t * (3 - 2 * t);
function noise(x, y) {
  const xi = Math.floor(x),
    yi = Math.floor(y);
  const xf = fade(x - xi),
    yf = fade(y - yi);
  const x0 = ((xi % P) + P) % P,
    y0 = ((yi % P) + P) % P;
  const x1 = (x0 + 1) % P,
    y1 = (y0 + 1) % P;
  const a = lattice[y0 * P + x0],
    b = lattice[y0 * P + x1];
  const c = lattice[y1 * P + x0],
    d = lattice[y1 * P + x1];
  return a + (b - a) * xf + (c - a) * yf + (a - b - c + d) * xf * yf;
}
function fbm(x, y, oct) {
  let v = 0,
    amp = 0.5,
    f = 1;
  for (let o = 0; o < oct; o++) {
    v += amp * noise(x * f, y * f);
    f *= 2.03;
    amp *= 0.5;
  }
  return v;
}

// Land centres give a couple of continents and some islands.
const blobs = [
  [0.35, 0.45, 0.32],
  [0.68, 0.55, 0.28],
  [0.55, 0.22, 0.14],
  [0.82, 0.2, 0.08],
  [0.15, 0.8, 0.09],
];
const scale = (6 / Math.max(W, H)) * 1.0;
const lerp = (a, b, t) => a + (b - a) * t;
const mix = (c1, c2, t) => [lerp(c1[0], c2[0], t), lerp(c1[1], c2[1], t), lerp(c1[2], c2[2], t)];

const row = W * 3 + 1;
const raw = Buffer.alloc(row * H);
for (let y = 0; y < H; y++) {
  raw[y * row] = 0;
  for (let x = 0; x < W; x++) {
    const u = x / W,
      v = y / H;
    let mask = 0;
    for (const [bx, by, r] of blobs) {
      const d = Math.hypot((u - bx) * (W / H), v - by) / r;
      mask = Math.max(mask, 1 - d);
    }
    const n = fbm(x * scale, y * scale, 7);
    const ridge = 1 - Math.abs(fbm(x * scale * 2 + 50, y * scale * 2 + 50, 5) - 0.5) * 2;
    const h =
      mask * 0.55 + (n - 0.5) * 1.5 + (ridge - 0.7) * 0.5 * Math.min(1, Math.max(0, mask * 4));
    const detail = fbm(x * scale * 8 + 99, y * scale * 8 + 99, 3) - 0.5;
    let c;
    if (h < -0.05) c = mix([30, 60, 100], [44, 88, 130], Math.min(1, (h + 0.6) / 0.55));
    else if (h < 0.05) c = mix([44, 88, 130], [90, 150, 170], (h + 0.05) / 0.1);
    else if (h < 0.08) c = [214, 198, 150];
    else if (h < 0.3) c = mix([120, 160, 85], [95, 140, 70], (h - 0.08) / 0.22);
    else if (h < 0.48) c = mix([60, 105, 55], [45, 85, 45], (h - 0.3) / 0.18);
    else if (h < 0.66) c = mix([120, 110, 95], [150, 140, 128], (h - 0.48) / 0.18);
    else c = mix([200, 200, 205], [245, 245, 250], Math.min(1, (h - 0.66) / 0.15));
    const shade = 1 + detail * 0.25;
    // Parchment vignette toward the edges.
    const edge = Math.min(u, v, 1 - u, 1 - v);
    const vig = edge < 0.03 ? 0.55 + edge * 15 : 1;
    const i = y * row + 1 + x * 3;
    const tint = [235, 220, 185];
    for (let k = 0; k < 3; k++) {
      const val = lerp(c[k] * shade, tint[k], 0.12) * vig;
      raw[i + k] = Math.max(0, Math.min(255, val));
    }
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
ihdr[8] = 8;
ihdr[9] = 2;
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 6 })),
  chunk("IEND", Buffer.alloc(0)),
]);
writeFileSync(out, png);
console.log(`Wrote ${out} (${W}x${H}, ${(png.length / 1e6).toFixed(1)} MB)`);
