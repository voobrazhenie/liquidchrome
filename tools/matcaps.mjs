// The two built-in matcap spheres, generated rather than pasted.
//
// A matcap is a photograph of a sphere: the shader looks the pixel up by the
// screen-space normal, so whatever lighting the sphere was shot under comes
// along with it. Generating them keeps the page self-contained without carrying
// a binary nobody can diff — and this file says exactly what is in each one.
//
// Usage: node matcaps.mjs   -> writes the two data URIs to tools/matcaps.json
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const N = 256;

// ---- a minimal PNG writer ---------------------------------------------------
const TAB = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = TAB[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}
function png(w, h, rgb) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 2;                       // 8-bit, truecolour RGB
  // Row filters matter here: a smooth gradient stored raw barely deflates at all,
  // and the same picture with Sub or Up applied is a quarter of the size. Pick
  // per row by the usual heuristic — the filter whose output sums smallest.
  const stride = w * 3, bpp = 3;
  const raw = Buffer.alloc(h * (1 + stride));
  const cand = [Buffer.alloc(stride), Buffer.alloc(stride), Buffer.alloc(stride), Buffer.alloc(stride)];
  for (let y = 0; y < h; y++) {
    const row = rgb.subarray(y * stride, (y + 1) * stride);
    const up = y > 0 ? rgb.subarray((y - 1) * stride, y * stride) : null;
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? row[x - bpp] : 0;
      const b = up ? up[x] : 0;
      const c = up && x >= bpp ? up[x - bpp] : 0;
      cand[0][x] = row[x];
      cand[1][x] = (row[x] - a) & 255;
      cand[2][x] = (row[x] - b) & 255;
      cand[3][x] = (row[x] - ((a + b) >> 1)) & 255;
    }
    let best = 0, bestSum = Infinity;
    for (let f = 0; f < 4; f++) {
      let sum = 0;
      for (let x = 0; x < stride; x++) { const v = cand[f][x]; sum += v < 128 ? v : 256 - v; }
      if (sum < bestSum) { bestSum = sum; best = f; }
    }
    raw[y * (1 + stride)] = best;
    cand[best].copy(raw, y * (1 + stride) + 1);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// ---- helpers ----------------------------------------------------------------
const cl = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const sstep = (a, b, x) => { const t = cl((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const mix = (a, b, t) => a + (b - a) * t;
const norm = (v) => { const l = Math.hypot(...v) || 1; return v.map(c => c / l); };
const dot = (a, b) => a[0]*b[0] + a[1]*b[1] + a[2]*b[2];
const srgb = (v) => Math.round(255 * cl(Math.pow(cl(v), 1 / 2.2)));

// Each generator is handed the screen-space normal of the sphere and returns a
// LINEAR colour; the writer does the gamma.
function build(shade, outside) {
  const buf = Buffer.alloc(N * N * 3);
  for (let py = 0; py < N; py++) {
    for (let px = 0; px < N; px++) {
      const x = (px + 0.5) / N * 2 - 1;
      const y = 1 - (py + 0.5) / N * 2;           // +y up, as the image reads
      const r2 = x * x + y * y;
      const c = r2 <= 1 ? shade([x, y, Math.sqrt(1 - r2)], Math.sqrt(r2)) : outside(x, y);
      const i = (py * N + px) * 3;
      buf[i] = srgb(c[0]); buf[i + 1] = srgb(c[1]); buf[i + 2] = srgb(c[2]);
    }
  }
  return buf;
}

// ---- 1. polished metal ------------------------------------------------------
// A chrome ball on a dark stand: cool grey sky above the reflected horizon, warm
// brown floor below it, one hard key light and a couple of softer bounces.
const KEY  = norm([-0.52,  0.50, 0.69]);
const WARM = norm([ 0.22, -0.02, 0.98]);
const SIDE = norm([ 0.62, -0.28, 0.73]);
function metal(n, r) {
  const sky   = [0.50, 0.52, 0.55];
  const floor = [0.115, 0.072, 0.058];
  const h = sstep(-0.34, 0.30, n[1]);              // the reflected horizon
  let c = [0, 1, 2].map(i => mix(floor[i], sky[i], h));
  // the horizon itself is a soft dark seam
  const seam = Math.exp(-Math.pow((n[1] + 0.05) / 0.16, 2)) * 0.16;
  c = c.map(v => v * (1 - seam));

  const k = dot(n, KEY);
  const soft = sstep(0.62, 0.99, k) * 0.55;        // the glow around it
  const core = sstep(0.905, 0.965, k);             // the blown-out middle
  c = [0, 1, 2].map(i => c[i] + soft * [1.0, 0.99, 0.96][i] + core * 5.2);

  const w = sstep(0.90, 1.0, dot(n, WARM)) * 0.55;
  c = [0, 1, 2].map(i => c[i] + w * [1.0, 0.86, 0.70][i]);
  const s = sstep(0.93, 1.0, dot(n, SIDE)) * 0.30;
  c = [0, 1, 2].map(i => c[i] + s * [1.0, 0.90, 0.80][i]);

  // a bright streak low on the left, where the stand catches the key
  const st = Math.exp(-Math.pow((n[0] + 0.62) / 0.20, 2)) * Math.exp(-Math.pow((n[1] + 0.42) / 0.26, 2));
  c = [0, 1, 2].map(i => c[i] + st * 0.85 * [1.0, 0.95, 0.88][i]);

  // the edge turns away and goes dark, with the thinnest catch right on it
  const fall = 1 - 0.55 * sstep(0.80, 1.0, r);
  const lip = sstep(0.965, 0.999, r) * (1 - sstep(0.995, 1.0, r)) * 0.22;
  return c.map(v => v * fall + lip);
}
const metalBg = (x, y) => {
  const v = 0.030 - 0.012 * sstep(0.2, 1.5, Math.hypot(x, y));
  return [v, v, v];
};

// ---- 2. normals -------------------------------------------------------------
// Not a photograph at all: the normal itself, written straight into the colour.
// Nothing reads more directly for checking which way a surface is pointing.
const normals = (n) => [n[0] * 0.5 + 0.5, 0.5 - n[1] * 0.5, n[2] * 0.5 + 0.5]
  .map(v => Math.pow(v, 2.2));                     // build() will undo this
const normalsBg = () => [Math.pow(0.5, 2.2), Math.pow(0.5, 2.2), 1];

const out = {
  chrome:  "data:image/png;base64," + png(N, N, build(metal,   metalBg)).toString("base64"),
  normals: "data:image/png;base64," + png(N, N, build(normals, normalsBg)).toString("base64"),
};
writeFileSync(join(here, "matcaps.json"), JSON.stringify(out, null, 1));
for (const [k, v] of Object.entries(out)) console.log(k.padEnd(9), (v.length / 1024).toFixed(1) + " KB");
