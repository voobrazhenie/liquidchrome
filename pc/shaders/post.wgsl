// Post pass — a port of <script id="fsPost"> in index.html, plus the upscaler.
//
// Three modes, chosen by the console:
//   0  resolve — a plain read. When the texture is larger than the output the
//      bilinear sampler resolves it, which for an exact 2x is a box filter.
//   1  FXAA — the page's filter, unchanged.
//   2  upscale — the march ran below the window size on purpose; reconstruct it
//      with a Catmull-Rom kernel and a clamped sharpen rather than letting the
//      bilinear sampler smear it.

struct Post {
  outRes: vec2f,   // size being drawn
  texel: vec2f,    // 1 / size of the texture being read
  mode: f32,       // 0 resolve, 1 FXAA, 2 upscale
  sharpen: f32,    // how hard the upscaler pulls the edges back
  pad0: f32,
  pad1: f32,
};

@group(0) @binding(0) var<uniform> P: Post;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

const LUMA = vec3f(0.299, 0.587, 0.114);

@vertex
fn vsMain(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
  let x = select(-1.0, 3.0, vi == 1u);
  let y = select(-1.0, 3.0, vi == 2u);
  return vec4f(x, y, 0.0, 1.0);
}

// Nine bilinear taps arranged as a Catmull-Rom kernel: sharper than bilinear on
// a magnified image, and without the stair-stepping a nearest fetch would give.
fn catmullRom(uv: vec2f) -> vec3f {
  let src = vec2f(1.0) / P.texel;
  let inv = P.texel;
  let samplePos = uv * src;
  let texPos1 = floor(samplePos - vec2f(0.5)) + vec2f(0.5);
  let f = samplePos - texPos1;

  let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
  let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
  let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
  let w3 = f * f * (-0.5 + 0.5 * f);
  let w12 = w1 + w2;

  let p0 = (texPos1 - vec2f(1.0)) * inv;
  let p3 = (texPos1 + vec2f(2.0)) * inv;
  let p12 = (texPos1 + w2 / w12) * inv;

  var c = vec3f(0.0);
  c += textureSample(tex, samp, vec2f(p0.x, p0.y)).rgb * (w0.x * w0.y);
  c += textureSample(tex, samp, vec2f(p12.x, p0.y)).rgb * (w12.x * w0.y);
  c += textureSample(tex, samp, vec2f(p3.x, p0.y)).rgb * (w3.x * w0.y);
  c += textureSample(tex, samp, vec2f(p0.x, p12.y)).rgb * (w0.x * w12.y);
  c += textureSample(tex, samp, vec2f(p12.x, p12.y)).rgb * (w12.x * w12.y);
  c += textureSample(tex, samp, vec2f(p3.x, p12.y)).rgb * (w3.x * w12.y);
  c += textureSample(tex, samp, vec2f(p0.x, p3.y)).rgb * (w0.x * w3.y);
  c += textureSample(tex, samp, vec2f(p12.x, p3.y)).rgb * (w12.x * w3.y);
  c += textureSample(tex, samp, vec2f(p3.x, p3.y)).rgb * (w3.x * w3.y);
  return c;
}

@fragment
fn fsMain(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  // The scene pass wrote this texture with the same top-down convention the
  // sampler reads it in, so there is no flip here.
  let uv = pos.xy / P.outRes;
  let inv = P.texel;

  if (P.mode > 1.5) {
    let up = catmullRom(uv);
    // Unsharp against the source's own four neighbours, then clamped to them:
    // the sharpen cannot invent a value the neighbourhood does not already
    // bracket, which is what keeps a halo off every edge.
    let n = textureSample(tex, samp, uv + vec2f(0.0, -inv.y)).rgb;
    let s = textureSample(tex, samp, uv + vec2f(0.0, inv.y)).rgb;
    let e = textureSample(tex, samp, uv + vec2f(inv.x, 0.0)).rgb;
    let w = textureSample(tex, samp, uv + vec2f(-inv.x, 0.0)).rgb;
    let avg = (n + s + e + w) * 0.25;
    let lo = min(min(n, s), min(e, w));
    let hi = max(max(n, s), max(e, w));
    let sharp = up + (up - avg) * P.sharpen;
    return vec4f(clamp(sharp, min(lo, up), max(hi, up)), 1.0);
  }

  if (P.mode < 0.5) {
    return textureSample(tex, samp, uv);
  }

  let mC = textureSample(tex, samp, uv).rgb;
  let nw = textureSample(tex, samp, uv + vec2f(-1.0, -1.0) * inv).rgb;
  let ne = textureSample(tex, samp, uv + vec2f(1.0, -1.0) * inv).rgb;
  let sw = textureSample(tex, samp, uv + vec2f(-1.0, 1.0) * inv).rgb;
  let se = textureSample(tex, samp, uv + vec2f(1.0, 1.0) * inv).rgb;
  let lM = dot(mC, LUMA);
  let lNW = dot(nw, LUMA);
  let lNE = dot(ne, LUMA);
  let lSW = dot(sw, LUMA);
  let lSE = dot(se, LUMA);
  let lMin = min(lM, min(min(lNW, lNE), min(lSW, lSE)));
  let lMax = max(lM, max(max(lNW, lNE), max(lSW, lSE)));

  var dir = vec2f(-((lNW + lNE) - (lSW + lSE)), ((lNW + lSW) - (lNE + lSE)));
  let reduce = max((lNW + lNE + lSW + lSE) * 0.03125, 0.0078125);
  let rcp = 1.0 / (min(abs(dir.x), abs(dir.y)) + reduce);
  dir = clamp(dir * rcp, vec2f(-8.0), vec2f(8.0)) * inv;

  let a = 0.5
    * (textureSample(tex, samp, uv + dir * (1.0 / 3.0 - 0.5)).rgb
      + textureSample(tex, samp, uv + dir * (2.0 / 3.0 - 0.5)).rgb);
  let b = a * 0.5
    + 0.25
      * (textureSample(tex, samp, uv - dir * 0.5).rgb
        + textureSample(tex, samp, uv + dir * 0.5).rgb);
  let lB = dot(b, LUMA);
  let outc = select(b, a, lB < lMin || lB > lMax);
  return vec4f(outc, 1.0);
}
