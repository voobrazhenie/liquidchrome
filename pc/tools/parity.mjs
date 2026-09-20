// Does the WGSL port draw the same picture as the page?
//
// There is no GPU in the build container, so the check runs where the original
// shader runs: the ported shader is emitted as GLSL ES 300 the way wgpu's OpenGL
// backend emits it, and both it and the original #fs shader from index.html are
// rendered in headless Chromium (SwiftShader) with identical uniforms. The two
// images are then compared pixel for pixel.
//
//   NODE_PATH=/opt/node22/lib/node_modules node tools/parity.mjs
//
// Frames are stored the other way up — wgpu's GL backend flips clip space so that
// gl_FragCoord lands where @builtin(position) does — so row y of one is compared
// with row H-1-y of the other.

import { createRequire } from "node:module";

// playwright lives in the container's global modules, so it is required rather
// than imported: NODE_PATH=/opt/node22/lib/node_modules node tools/parity.mjs
const { chromium } = createRequire(import.meta.url)("playwright");
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const pc = join(here, "..");
const out = join(pc, "target", "parity");
mkdirSync(out, { recursive: true });

const W = 220, H = 140, STEPS = 72;

// ---- the original shader, straight out of the page --------------------------
const page = readFileSync(join(pc, "..", "index.html"), "utf8");
const webFs = page.split('<script type="x-shader/x-fragment" id="fs">')[1].split("</script>")[0];
const DEFS = ["#define SCENE_BRAIN\n", "#define SCENE_NEURON\n", "#define SCENE_CHROME\n"];

// ---- the port, as wgpu's OpenGL backend would hand it to the driver ----------
function emit(scene) {
  const file = join(out, `scene${scene}.frag`);
  execFileSync("cargo", ["run", "--quiet", "--example", "emit_glsl", "--", String(scene), file], {
    cwd: pc,
    stdio: ["ignore", "inherit", "inherit"],
  });
  return readFileSync(file, "utf8");
}

// ---- one set of uniforms, in both shapes ------------------------------------
const R = [1.55, 1.15, 1.10];
function uniforms(scene, over = {}) {
  const r = R[scene];
  return {
    res: [W, H],
    time: 12.0,
    roll: 0.0,
    // the benchmark's "Diagonal" shot, a third of the way along
    eye: [-0.43 * r, 0.75 * r, -0.43 * r],
    tgt: [0.0, 0.05 * r, 0.0],
    colA: scene === 2 ? [0.0, 0.6156, 1.0] : [0.1843, 0.851, 0.7529],
    colB: scene === 2 ? [1.0, 0.2588, 0.4431] : [1.0, 0.7882, 0.5412],
    warpOn: [1, 0, 0],
    lcOn: [1, 1, 1, 1],
    scale: 15.0,
    spike: 0.55,
    steps: STEPS,
    fov: 1.30,
    shift: 0.0,
    morph: 40.0,
    pulse: 6.0,
    scene,
    thick: 0.028,
    warpAmt: 0.74,
    warpFreq: 3.75,
    lcTwirl: 1.90,
    lcRelief: 0.26,
    lcFreq: 4.0,
    lcThick: 0.022,
    eps: 1 / 12.0,
    omega: 1.30,
    bound: 2,
    boundPad: 0.05,
    warpMode: 0,
    matSmooth: 0.5,     // exp2(1 + 9.169925*0.5) = 48 exactly, the page's exponent
    matMetal: 0.0,
    postExposure: 1.0,
    postGlow: 1.0,
    postFog: 1.0,
    postVignette: 0.55,
    postGrain: 0.022,
    bgLow: [0.010, 0.015, 0.026],
    bgHigh: [0.020, 0.032, 0.055],
    rimCol: [0.620, 0.898, 1.000],
    spikeCol: [0.608, 0.482, 1.000],
    ...over,
  };
}

const CASES = [
  { name: "Brain, noise warp in space", scene: 0, over: {} },
  { name: "Brain, warp off, sphere bounds", scene: 0, over: { warpOn: [0, 0, 0], bound: 0 } },
  { name: "Neuron, twist and bend", scene: 1, over: { warpOn: [0, 1, 1], warpAmt: 0.45 } },
  { name: "Neuron, surface warp", scene: 1, over: { warpMode: 1 } },
  { name: "Chrome, all five steps", scene: 2, over: {} },
  { name: "Chrome, relief off", scene: 2, over: { lcOn: [1, 0, 1, 1] } },
];

const browser = await chromium.launch({
  args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
});
const tab = await browser.newPage();
tab.on("console", (m) => {
  if (m.type() === "error") console.error("  page:", m.text());
});
await tab.setContent("<!doctype html><meta charset=utf-8><body></body>");

const ported = [emit(0), emit(1), emit(2)];
let worst = 0;
let failed = 0;

/// Runs in the page: compiles both shaders, renders both, compares them.
const renderPair = async ([webSrc, portSrc, u, W, H]) => {
      const cv = document.createElement("canvas");
      cv.width = W;
      cv.height = H;
      const gl = cv.getContext("webgl2", { antialias: false, alpha: false, preserveDrawingBuffer: true });
      if (!gl) return { error: "no webgl2" };

      const compile = (type, src) => {
        const s = gl.createShader(type);
        gl.shaderSource(s, src);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
        return s;
      };
      const link = (vs, fs) => {
        const p = gl.createProgram();
        gl.attachShader(p, vs);
        gl.attachShader(p, fs);
        gl.bindAttribLocation(p, 0, "aPos");
        gl.linkProgram(p);
        if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
        return p;
      };

      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

      const draw = () => {
        gl.viewport(0, 0, W, H);
        gl.drawArrays(gl.TRIANGLES, 0, 3);
        const px = new Uint8Array(W * H * 4);
        gl.readPixels(0, 0, W, H, gl.RGBA, gl.UNSIGNED_BYTE, px);
        return px;
      };

      // --- the page's shader, ES 1.00 ---
      const vs1 = compile(gl.VERTEX_SHADER, "attribute vec2 aPos; void main(){ gl_Position = vec4(aPos, 0.0, 1.0); }");
      const pa = link(vs1, compile(gl.FRAGMENT_SHADER, webSrc));
      gl.useProgram(pa);
      const set = (n, f, ...v) => {
        const l = gl.getUniformLocation(pa, n);
        if (l) gl[f](l, ...v);
      };
      set("uRes", "uniform2f", u.res[0], u.res[1]);
      set("uTime", "uniform1f", u.time);
      set("uEye", "uniform3f", ...u.eye);
      set("uTgt", "uniform3f", ...u.tgt);
      set("uRoll", "uniform1f", u.roll);
      set("uScale", "uniform1f", u.scale);
      set("uSpike", "uniform1f", u.spike);
      set("uSteps", "uniform1f", u.steps);
      set("uFov", "uniform1f", u.fov);
      set("uShift", "uniform1f", u.shift);
      set("uMorph", "uniform1f", u.morph);
      set("uPulse", "uniform1f", u.pulse);
      set("uScene", "uniform1f", u.scene);
      set("uThick", "uniform1f", u.thick);
      set("uWarpAmt", "uniform1f", u.warpAmt);
      set("uWarpFreq", "uniform1f", u.warpFreq);
      set("uWarpOn", "uniform3f", ...u.warpOn);
      set("uColA", "uniform3f", ...u.colA);
      set("uColB", "uniform3f", ...u.colB);
      set("uLcOn", "uniform4f", ...u.lcOn);
      set("uLcTwirl", "uniform1f", u.lcTwirl);
      set("uLcRelief", "uniform1f", u.lcRelief);
      set("uLcFreq", "uniform1f", u.lcFreq);
      set("uLcThick", "uniform1f", u.lcThick);
      set("uEps", "uniform1f", u.eps);
      set("uOmega", "uniform1f", u.omega);
      set("uBound", "uniform1f", u.bound);
      // the page can turn its film grain off; the port has no control for it, so
      // the comparison is made with the page's grain switched on to match
      set("uNoise", "uniform1f", 1.0);
      // Same story as the grain: the port has no control for the air's
      // thickness, so the page is pinned to the multiple that means "as it has
      // always been". An unset uniform is zero, which would take the fog off
      // the page entirely and make every case differ.
      set("uHaze", "uniform1f", 1.0);
      set("uGlow", "uniform1f", 1.0);
      set("uMirror", "uniform2f", 0.0, 0.0);   // the port has no mirror; unfolded is zero
      set("uBall", "uniform1f", 0.0);          // nor the blend to a ball; none of it is zero
      set("uDofOn", "uniform1f", 0.0);         // nor a lens; everything is in focus
      set("uLcOct", "uniform1f", 2.0);         // the port has the two octaves of relief
      set("uWarpOct", "uniform1f", 1.0);       // and the one of warp, hard-coded
      set("uSeedOff", "uniform3f", 0.0, 0.0, 0.0);  // nor a seed; zero is the roll it ports
      set("uBoundPad", "uniform1f", u.boundPad);
      set("uWarpMode", "uniform1f", u.warpMode);
      const web = draw();

      // --- the port, ES 3.00, one std140 block ---
      const vs3 = compile(gl.VERTEX_SHADER, "#version 300 es\nin vec2 aPos; void main(){ gl_Position = vec4(aPos, 0.0, 1.0); }");
      const pb = link(vs3, compile(gl.FRAGMENT_SHADER, portSrc));
      gl.useProgram(pb);
      const f = new Float32Array(72);
      f.set(u.res, 0);
      f[2] = u.time;
      f[3] = u.roll;
      f.set([...u.eye, 0], 4);
      f.set([...u.tgt, 0], 8);
      f.set([...u.colA, 0], 12);
      f.set([...u.colB, 0], 16);
      f.set([...u.warpOn, 0], 20);
      f.set(u.lcOn, 24);
      f.set([u.scale, u.spike, u.steps, u.fov], 28);
      f.set([u.shift, u.morph, u.pulse, u.scene], 32);
      f.set([u.thick, u.warpAmt, u.warpFreq, u.lcTwirl], 36);
      f.set([u.lcRelief, u.lcFreq, u.lcThick, u.eps], 40);
      f.set([u.omega, u.bound, u.boundPad, u.warpMode], 44);
      // the native-only material and post controls. Their defaults are the values
      // the page hard-coded, so this comparison is also what proves they are
      // exact no-ops until somebody moves a slider.
      f.set([u.matSmooth, u.matMetal, u.postExposure, u.postGlow], 48);
      f.set([u.postFog, u.postVignette, u.postGrain, 0], 52);
      // the background gradient and the two lighting tints, at the values the page
      // hard-coded — the comparison is what keeps them honest
      f.set([...u.bgLow, 0], 56);
      f.set([...u.bgHigh, 0], 60);
      f.set([...u.rimCol, 0], 64);
      f.set([...u.spikeCol, 0], 68);
      const ubo = gl.createBuffer();
      gl.bindBuffer(gl.UNIFORM_BUFFER, ubo);
      gl.bufferData(gl.UNIFORM_BUFFER, f, gl.STATIC_DRAW);
      const blockName = /uniform\s+(\w+)\s*\{/.exec(portSrc)[1];
      const idx = gl.getUniformBlockIndex(pb, blockName);
      gl.uniformBlockBinding(pb, idx, 0);
      gl.bindBufferBase(gl.UNIFORM_BUFFER, 0, ubo);
      const port = draw();

      // --- compare, one image flipped ---
      let sum = 0, max = 0, over2 = 0, lit = 0;
      const diffs = [];
      for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) {
          const a = (y * W + x) * 4;
          const b = ((H - 1 - y) * W + x) * 4;
          let d = 0;
          for (let k = 0; k < 3; k++) d = Math.max(d, Math.abs(web[a + k] - port[b + k]));
          sum += d;
          if (d > max) max = d;
          if (d > 2) over2++;
          diffs.push(d);
          // the background is a dim blue gradient; anything this bright is the object
          if (Math.max(web[a], web[a + 1], web[a + 2]) > 70) lit++;
        }
      }
      diffs.sort((p, q) => p - q);
      const n = W * H;
      const chan = (px) => {
        let m = 0;
        for (let i = 0; i < px.length; i += 4) {
          m = Math.max(m, px[i], px[i + 1], px[i + 2]);
        }
        return m;
      };
      // keep the pair on disk so the picture itself can be looked at
      const png = (px) => {
        const c2 = document.createElement("canvas");
        c2.width = W; c2.height = H;
        const ctx = c2.getContext("2d");
        const img = ctx.createImageData(W, H);
        // readPixels is bottom-up
        for (let y = 0; y < H; y++) {
          for (let x = 0; x < W; x++) {
            const s = ((H - 1 - y) * W + x) * 4, d = (y * W + x) * 4;
            img.data[d] = px[s]; img.data[d + 1] = px[s + 1];
            img.data[d + 2] = px[s + 2]; img.data[d + 3] = 255;
          }
        }
        ctx.putImageData(img, 0, 0);
        return c2.toDataURL("image/png");
      };
      return {
        webPng: png(web),
        portPng: png(port).replace(/^data:image\/png;base64,/, ""),
        mean: sum / n,
        p99: diffs[Math.floor(0.99 * (n - 1))],
        max,
        over2: (100 * over2) / n,
        lit: (100 * lit) / n,
        webMax: chan(web),
        portMax: chan(port),
      };
};

for (const c of CASES) {
  const u = uniforms(c.scene, c.over);
  const res = await tab.evaluate(renderPair, [DEFS[c.scene] + webFs, ported[c.scene], u, W, H]);

  if (res.error) {
    console.error(`${c.name}: ${res.error}`);
    failed++;
    continue;
  }
  // an image that is all background would match trivially, so the object has to
  // actually be in frame for the case to count
  const slug = c.name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
  writeFileSync(join(out, `${slug}-web.png`), Buffer.from(res.webPng.replace(/^data:image\/png;base64,/, ""), "base64"));
  writeFileSync(join(out, `${slug}-port.png`), Buffer.from(res.portPng, "base64"));
  const ok = res.mean < 1.0 && res.p99 <= 8 && res.over2 < 2.0 && res.lit > 3.0;
  if (!ok) failed++;
  worst = Math.max(worst, res.mean);
  console.log(
    `${ok ? "ok  " : "FAIL"} ${c.name.padEnd(30)} mean ${res.mean.toFixed(3)}  p99 ${String(res.p99).padStart(3)}  max ${String(res.max).padStart(3)}  >2 ${res.over2.toFixed(2)}%  (object fills ${res.lit.toFixed(1)}% of frame)`,
  );
}

// ---- and one thing the page cannot do ---------------------------------------
// With every colour set to black the object must actually be black. The page's
// shader carries its rim and back light as constants, so it stays purple; this is
// the check that those two are really controls now.
{
  const u = uniforms(1, {
    colA: [0, 0, 0],
    colB: [0, 0, 0],
    rimCol: [0, 0, 0],
    spikeCol: [0, 0, 0],
    bgLow: [0, 0, 0],
    bgHigh: [0, 0, 0],
    postGrain: 0,
  });
  const res = await tab.evaluate(
    renderPair,
    [DEFS[1] + webFs, ported[1], u, W, H],
  );
  const ok = res.portMax <= 2;
  if (!ok) failed++;
  console.log(
    `${ok ? "ok  " : "FAIL"} ${"All colours black".padEnd(30)} port max channel ${res.portMax}  (page: ${res.webMax}, which is the tint that had no control)`,
  );
}

await browser.close();
console.log(failed ? `\n${failed} case(s) differ` : `\nall cases match (worst mean ${worst.toFixed(3)} levels of 255)`);
process.exit(failed ? 1 : 0);
