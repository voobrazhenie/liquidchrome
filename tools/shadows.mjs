// Shadows on the page, checked in a browser because there is no GPU here.
//
// Three things have to hold, and only the first two can be checked without eyes:
//   * every object still compiles and draws, with no page errors
//   * with shadows OFF the image is byte-identical to the baseline — the whole
//     no-op discipline, and what keeps parity.mjs green
//   * with shadows ON at full darkness the image changes, reaches real black, and
//     still matches its own stored shot — the march is easy to make subtly wrong
//     and the difference is a shape, not a number
//   * the left drag aims the sun, the right drag always turns the camera, and
//     L swaps the left one back and forth between them
//
// Usage: node shadows.mjs base   -> write baseline shots
//        node shadows.mjs check  -> compare against them
import { createRequire } from "node:module";
import { inflateSync } from "node:zlib";
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
// an ESM import cannot see NODE_PATH, so playwright comes in the old way
const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const here = dirname(fileURLToPath(import.meta.url));
const page_url = "file://" + join(here, "..", "index.html");
const OUT = join(here, "..", "pc", "target", "shadows");  // gitignored already
const mode = process.argv[2] || "check";
const W = 360, H = 240;

const OBJECTS = [
  { name: "brain", scene: 0 },
  { name: "neuron", scene: 1 },
  { name: "chrome", scene: 2 },
];

// The page animates, so every shot has to be taken at the same pose. Freezing the
// clocks is not enough: the frame-time controller moves the render scale and the
// march budget on its own, and a software rasteriser is slow enough to trip it. Pin
// both, or two runs of the same frame are not the same frame.
async function pose(page, over) {
  await page.evaluate((o) => {
    const s = window.__state;
    s.running = false; s.morph = false;
    s.clock = 12.0; s.mClock = 40.0; s.pClock = 6.0;
    s.fly = false; s.mode = 0; s.dragAz = 0.6; s.dragEl = 0.25; s.zoom = 1.0;
    // A software rasteriser is slow enough that even a pinned resolution gives way
    // at 250 ms, and the ramp down takes different numbers of frames for two
    // shaders of different cost. 0.30 is the controller's floor and so its one
    // fixed point: it will not go lower, and it can only climb at 13 ms a frame,
    // which will not happen here. Start there and nothing moves.
    s.resPin = 1; s.scaleQ = 0.30; s.stepsPin = true; s.steps = 64;
    s.aa = 0;
    s.velAz = 0; s.velEl = 0;        // a drag leaves inertia behind; the next one starts still
    s.matcapOn = 0; s.matcap = 0;    // every pose starts from the lit material
    s.shadowSrc = 0; s.shadowLevel = 0.5; s.shadowContrast = 0; s.shadowMc = 0;
    // A pose that leaves the geometry as the last one left it is not a pose. The
    // cube rig turns build steps off and the warp down, and the next case along
    // then quietly compares a different object with the baseline.
    s.lcOn = [1, 1, 1, 1, 1, 0]; s.warpOn = [1, 0, 0]; s.warpAmt = 0.74;
    s.lcOct = 2; s.warpOct = 1;    // the counts the page has always had
    s.eps = 12.00; s.omega = 1.30; s.bound = 2; s.boundPad = 0.05;
    s.lcRelief = 0.26; s.lcFreq = 4.00; s.lcThick = 0.022; s.lcTwirl = 1.90;
    // The film grain has a control now and it is off by default. The stored
    // baselines were taken before it existed, so the pose turns it back on:
    // that keeps "the sun switched off is the page as it was" a live claim
    // about the shading rather than a claim about the grain.
    s.noise = 1.0; s.haze = 1.0; s.glow = 1.0; s.shadowOnly = 0; s.shadowSteps = 96;
    s.normalOn = 0; s.normalMc = 0; s.normalAmt = 1.0; s.normalScale = 2.0;
    s.glassOn = 0; s.glassOpacity = 0.15; s.glassEdge = 0.60; s.glassIor = 1.45;
    s.glassTint = "#BFE9FF"; s.glassDensity = 0.90; s.glassDepth = 2;
    s.depthOn = 0; s.depthMc = 0; s.depthAmt = 0.12; s.depthDist = 1.20;
    s.depthPos = [0, 0, 1.80]; s.depthRot = [0, 0, 0, 1];
    s.depthSize = [0.80, 0.80]; s.depthPart = [0, 0, 0];
    s.mirror = [0, 0]; s.shift = 0;
    s.spriteOn = 0; s.sprite = 0; s.spriteSize = 0.20; s.spriteFrames = 4; s.spriteFps = 6;
    s.ball = 0;   // NOT s.fov: this page is narrow enough that the page calls it
                  // a phone and picks a wider lens, and every baseline was shot
                  // through that one
    s.dofOn = 0; s.dofFocus = 3.0; s.dofRange = 0.8; s.dofBlur = 4.0;
    s.walkOn = 0; s.walkSpeed = 0.45; s.jumpHeight = 0.22;
    s.jumpOn = 1; s.runOn = 1; s.stepsOn = 0; s.walkSize = 1.0;
    s.seed = 0;   // every baseline is the roll the page ships with
    s.flyMin = 0.030; s.flyMax = 12.0; s.lookSmooth = 0; s.moveSmooth = 0;
    Object.assign(s, o);
  }, over);
  // Software rendering runs at a few frames a second, so waiting on the clock is
  // waiting on almost nothing. Wait on drawn frames instead.
  await page.evaluate(() => new Promise((done) => {
    let n = 0;
    const tick = () => (++n < 8 ? requestAnimationFrame(tick) : done());
    requestAnimationFrame(tick);
  }));
}

// A WebGL canvas is cleared once it has been composited, so drawImage gives back
// nothing. The screenshot is the only honest copy of the pixels — decode that.
function decode(png) {
  let i = 8, w = 0, h = 0, colour = 6, idat = [];
  while (i < png.length) {
    const len = png.readUInt32BE(i);
    const type = png.toString("ascii", i + 4, i + 8);
    // Chromium hands back an opaque canvas as RGB, not RGBA, so the stride is
    // three bytes and not four. Reading it as four decodes to noise — and noise
    // that still looks like a picture, which is how it went unnoticed.
    if (type === "IHDR") { w = png.readUInt32BE(i + 8); h = png.readUInt32BE(i + 12); colour = png[i + 17]; }
    if (type === "IDAT") idat.push(png.subarray(i + 8, i + 8 + len));
    i += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const bpp = { 0: 1, 2: 3, 4: 2, 6: 4 }[colour];
  if (!bpp) throw new Error("unsupported PNG colour type " + colour);
  const stride = w * bpp;
  const out = Buffer.alloc(h * stride);
  let pos = 0;
  for (let y = 0; y < h; y++) {
    const f = raw[pos++];
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? out[y * stride + x - bpp] : 0;
      const b = y > 0 ? out[(y - 1) * stride + x] : 0;
      const c = x >= bpp && y > 0 ? out[(y - 1) * stride + x - bpp] : 0;
      let v = raw[pos++];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) {
        const pa = Math.abs(b - c), pb = Math.abs(a - c), pc = Math.abs(a + b - 2 * c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      out[y * stride + x] = v & 255;
    }
  }
  return { w, h, bpp, data: out };
}

// How much of the frame the second shot took away from the first. Asking instead
// whether anything reaches pure black is asking the wrong question: film grain,
// the vignette and the glow are all added after the shading, so nothing in this
// picture is ever near zero, and a test for it passes or fails on the decoder
// rather than on the shadows.
function darkened(a, b, drop) {
  const A = decode(a), B = decode(b);
  const lum = (d, i) => 0.299 * d[i] + 0.587 * d[i + 1] + 0.114 * d[i + 2];
  let n = 0, total = 0;
  for (let i = 0; i + 2 < A.data.length; i += A.bpp) {
    total++;
    if (lum(A.data, i) - lum(B.data, i) > drop) n++;
  }
  return n / total;
}

// How far a frame is from being its own reflection about the vertical centre.
// A folded object seen square-on has to be symmetric, and nothing else here is
// a test of whether the fold actually folds rather than merely changes things.
function lopsided(png, drop) {
  const A = decode(png);
  let n = 0, total = 0;
  for (let y = 0; y < A.h; y++) {
    for (let x = 0; x < (A.w >> 1); x++) {
      const i = y * A.w * A.bpp + x * A.bpp;
      const j = y * A.w * A.bpp + (A.w - 1 - x) * A.bpp;
      total++;
      for (let c = 0; c < 3; c++) {
        if (Math.abs(A.data[i + c] - A.data[j + c]) > drop) { n++; break; }
      }
    }
  }
  return n / total;
}

// Switching object or resolution costs a few warm-up frames at a different size,
// and a screenshot taken during them is not the picture. Shoot until two in a row
// agree, and that one is settled.
async function shot(page, name) {
  let prev = null;
  for (let i = 0; i < 12; i++) {
    const buf = await page.locator("canvas").first().screenshot();
    if (prev && prev.equals(buf)) { prev = buf; break; }
    prev = buf;
    await page.evaluate(() => new Promise((done) => {
      let n = 0;
      const tick = () => (++n < 4 ? requestAnimationFrame(tick) : done());
      requestAnimationFrame(tick);
    }));
  }
  const buf = prev;
  writeFileSync(join(OUT, name + ".png"), buf);
  return buf;
}

const run = async () => {
  mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch({
    args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
  });
  const page = await browser.newPage({ viewport: { width: W, height: H } });
  const errors = [];
  // the page looks for its settings in the cloud and fails soft when there is no
  // network, which is the normal state of this container — not a fault
  const ours = (t) => !/ERR_|net::|Failed to load resource|firestore|googleapis/i.test(t);
  page.on("pageerror", (e) => { if (ours(String(e))) errors.push(String(e)); });
  page.on("console", (m) => { if (m.type() === "error" && ours(m.text())) errors.push(m.text()); });

  await page.goto(page_url);
  await page.waitForTimeout(1200);
  // the console is drawn over the canvas and an element screenshot picks it up —
  // live frame rates and all, which would never compare equal
  await page.keyboard.press("h");
  await page.waitForTimeout(300);

  let bad = 0;
  const say = (ok, msg) => { if (!ok) bad++; console.log((ok ? "ok  " : "FAIL") + " " + msg); };

  const hasState = await page.evaluate(() => !!window.__state);
  if (!hasState) {
    console.log("FAIL the page does not expose __state — the harness needs it");
    await browser.close();
    process.exit(1);
  }

  for (const o of OBJECTS) {
    await pose(page, { scene: o.scene, shadowOn: 0 });
    const off = await shot(page, o.name + "-off");
    const size = await page.evaluate(() => {
      const c = document.querySelector("canvas");
      return c.width + "x" + c.height;
    });
    say(errors.length === 0, `${o.name}: draws with no page errors (canvas ${size})`);

    const compare = (buf, file, what) => {
      const path = join(OUT, file);
      if (mode !== "check") {
        writeFileSync(path, buf);
        console.log(`--   ${o.name}: wrote ${file}`);
      } else if (existsSync(path)) {
        say(readFileSync(path).equals(buf), `${o.name}: ${what}`);
      } else {
        console.log(`--   ${o.name}: no ${what} to compare against`);
      }
    };
    compare(off, o.name + "-base.png", "shadows off is byte-identical to the baseline");

    // Hard, full strength, and deliberately low: a sun near the horizon is what
    // makes shadow rays leave at a shallow angle, which is the whole difficulty.
    await pose(page, {
      scene: o.scene, shadowOn: 1, shadowSoft: 0.0, shadowDark: 1.0,
      shadowReach: 4.0, sunAz: 2.2, sunEl: 0.35,
    });
    const on = await shot(page, o.name + "-on");
    say(!on.equals(off), `${o.name}: shadows on changes the image`);
    compare(on, o.name + "-on-base.png", "the shadows themselves are the stored ones");
  }

  // The grain, the lit/unlit view and standing where the sun does. None of the
  // three is subtle, so the useful thing to hold them to is that each does
  // something and that the last one gives the camera back untouched.
  if (mode === "check") {
    await pose(page, { scene: 2, shadowOn: 0 });
    const grainy = await shot(page, "noise-on");
    await pose(page, { scene: 2, shadowOn: 0, noise: 0.0 });
    say(!(await shot(page, "noise-off")).equals(grainy), "the noise slider changes the frame");

    await pose(page, { scene: 2, shadowOn: 1 });
    const normal = await shot(page, "sunview-off");
    const cam = () => page.evaluate(() => {
      const s = window.__state;
      return JSON.stringify([s.dragAz, s.dragEl, s.zoom, s.fly, s.mode]);
    });
    const before = await cam();
    await page.keyboard.press("NumpadMultiply");
    say(await page.evaluate(() => !!window.__sunView), "* stands the camera where the sun is");
    say(!(await shot(page, "sunview-on")).equals(normal), "and the view is not the one it left");
    await page.keyboard.press("NumpadMultiply");
    say(!(await page.evaluate(() => !!window.__sunView)), "* gives the camera back");
    say((await cam()) === before, "with nothing about it changed");

    await pose(page, { scene: 2, shadowOn: 1, shadowOnly: 1, shadowSrc: 2,
                       shadowContrast: 1.0, shadowLevel: 0.60 });
    say(!(await shot(page, "lit-unlit")).equals(normal), "the lit / unlit view draws the shadow itself");
  }

  // Three ways to arrive at a shadow. Only the first spends a ray; all three go
  // through the same shaping and the same application, so the useful thing to
  // hold them to is that they are genuinely three answers and not one answer
  // wearing three hats — and that the hard end of the shaping really is hard.
  if (mode === "check") {
    // relief so the two textureless sources have normals to work with, and the
    // cube so the marched one has something to actually cast
    const rig = {
      scene: 2, shadowOn: 1, shadowContrast: 1.0, sunAz: 0.9, sunEl: 0.90,
      warpOn: [0, 0, 0], warpAmt: 0, lcOn: [1, 1, 0, 0, 0, 1],
      fly: 0, mode: 0, dragAz: 0.0, dragEl: 0.50, zoom: 0.46,
    };
    const shots = [];
    for (const [name, over] of [
      ["ray march", { shadowSrc: 0, shadowLevel: 0.50 }],
      ["ray trace", { shadowSrc: 3, shadowLevel: 0.50 }],
      ["sphere",    { shadowSrc: 1, shadowLevel: 0.50, shadowMc: 0 }],
      ["sun angle", { shadowSrc: 2, shadowLevel: 0.76 }],
    ]) {
      const file = "shadow-" + name.replace(" ", "-");
      // the same source with the shadow worked out and then not applied, so what
      // is being measured is the shadow and not the lighting changing underneath
      await pose(page, { ...rig, ...over, shadowDark: 0.0 });
      const none = await shot(page, file + "-none");
      await pose(page, { ...rig, ...over, shadowDark: 1.0 });
      const full = await shot(page, file);
      shots.push(full);
      const d = darkened(none, full, 40);
      say(d > 0.01, `${name}, full contrast and amount, really darkens (${(d * 100).toFixed(1)}%)`);
    }
    let same = 0;
    for (let i = 0; i < shots.length; i++)
      for (let j = i + 1; j < shots.length; j++) if (shots[i].equals(shots[j])) same++;
    say(same === 0, `and the ${shots.length} are ${shots.length} answers, not one in ${shots.length} hats`);

    // the budget is a control now, and it has to be one that does something
    await pose(page, { ...rig, shadowSrc: 0, shadowDark: 1.0, shadowSteps: 12 });
    const few = await shot(page, "shadow-steps-few");
    await pose(page, { ...rig, shadowSrc: 0, shadowDark: 1.0, shadowSteps: 128 });
    say(!few.equals(await shot(page, "shadow-steps-many")),
        "and the shadow step budget changes what the march finds");
  }

  // A matcap replaces the material outright, so there are only two things worth
  // asking of it: that it does something, that the two built-in spheres do
  // different things, and that switching it off puts the page back exactly as it
  // was — the same no-op discipline the sun is held to.
  if (mode === "check") {
    const base = join(OUT, "chrome-base.png");
    await pose(page, { scene: 2, shadowOn: 0, matcapOn: 1, matcap: 0 });
    const mcA = await shot(page, "matcap-chrome");
    await pose(page, { scene: 2, shadowOn: 0, matcapOn: 1, matcap: 1 });
    const mcB = await shot(page, "matcap-normals");
    say(!mcA.equals(mcB), "the two built-in matcaps give different pictures");
    if (existsSync(base)) {
      const off = readFileSync(base);
      say(!mcA.equals(off), "a matcap replaces the lit material");
      await pose(page, { scene: 2, shadowOn: 0, matcapOn: 0 });
      say((await shot(page, "matcap-off")).equals(off),
          "and switching it off is byte-identical to the baseline");
    }

    // and one loaded from disk, which is the half of this that has no built-in
    // to fall back on: eight pixels of flat magenta, so the object drawn with it
    // could not be mistaken for anything else.
    const loaded = join(OUT, "loaded.png");
    writeFileSync(loaded, Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR42mP4r3ECK2IYWhIAaFh7wVa+/gkAAAAASUVORK5CYII=",
      "base64"));
    await page.setInputFiles("#mcFile", loaded);
    await page.waitForTimeout(500);
    const list = await page.evaluate(() =>
      Array.prototype.map.call(document.querySelectorAll("#mcSeg button"), b => b.textContent));
    say(list.length === 3 && list[2] === "loaded", `a loaded sphere joins the list (${list.join(", ")})`);
    say(await page.evaluate(() => window.__state.matcapOn === 1 && window.__state.matcap === 2),
        "and is switched on and selected");
    const drawn = await shot(page, "matcap-loaded");
    say(!drawn.equals(mcA) && !drawn.equals(mcB), "and the object is drawn with it");
  }

  // The normal map and seeing through things. Both are off out of the box and
  // both have an exact no-op inside them — Amount at nothing, Opacity at one —
  // which is the half of each that a picture cannot show.
  if (mode === "check") {
    const base = join(OUT, "chrome-base.png");
    const off = existsSync(base) ? readFileSync(base) : null;
    const flat = { scene: 2, shadowOn: 0 };

    // the magenta square loaded above is still in the list, and the normal-map
    // chooser is fed by that same one list
    const maps = await page.evaluate(() =>
      Array.prototype.map.call(document.querySelectorAll("#nmSeg button"), b => b.textContent));
    say(maps.length === 3 && maps[2] === "loaded",
        `the normal map picks from the same list (${maps.join(", ")})`);

    await pose(page, { ...flat, normalOn: 1, normalMc: 1, normalScale: 2.0 });
    const bumped = await shot(page, "normal-on");
    if (off) say(!bumped.equals(off), "a normal map moves the surface");
    await pose(page, { ...flat, normalOn: 1, normalMc: 1, normalScale: 9.0 });
    say(!(await shot(page, "normal-fine")).equals(bumped), "and Scale changes how fine it is");
    await pose(page, { ...flat, normalOn: 1, normalMc: 1, normalAmt: 0.0 });
    if (off) say((await shot(page, "normal-none")).equals(off),
                 "and Amount at nothing is byte-identical to the baseline");

    await pose(page, { ...flat, glassOn: 1, glassOpacity: 1.0 });
    if (off) say((await shot(page, "glass-solid")).equals(off),
                 "Opacity at 1 is byte-identical to a solid object");

    // nothing of its own left, no bending and no colour: the object should be
    // very nearly the background it is standing in front of
    await pose(page, { ...flat, glassOn: 1, glassOpacity: 0.0, glassEdge: 0.0,
                       glassDensity: 0.0, glassIor: 1.0, glassDepth: 1 });
    const clear = await shot(page, "glass-clear");
    if (off) {
      const d = darkened(off, clear, 20);
      say(d > 0.05, `clear through to the background takes the object out (${(d * 100).toFixed(1)}%)`);
    }

    await pose(page, { ...flat, glassOn: 1, glassDepth: 2 });
    const two = await shot(page, "glass-depth-2");
    say(!two.equals(clear), "tint, edge and refraction all land");
    await pose(page, { ...flat, glassOn: 1, glassDepth: 4 });
    say(!(await shot(page, "glass-depth-4")).equals(two), "and Depth changes how far the eye gets");
    await pose(page, { ...flat, glassOn: 1, glassDepth: 2, glassIor: 1.0 });
    say(!(await shot(page, "glass-straight")).equals(two), "and Refraction bends what is behind");
    await pose(page, { ...flat, glassOn: 1, glassDepth: 2, glassDensity: 4.0 });
    const thick = await shot(page, "glass-dense");
    say(!thick.equals(two), "and Density deepens the tint with the crossing");

    // The light gathered around the surfaces, which is the haze a decal's beam
    // fills a room with. 1.00 is the page as it was.
    await pose(page, { ...flat, glow: 1.0 });
    if (off) say((await shot(page, "glow-one")).equals(off),
                 "Glow at 1 is byte-identical to the baseline");
    await pose(page, { ...flat, glow: 0.0 });
    const dark = await shot(page, "glow-none");
    if (off) {
      const d = darkened(off, dark, 4);
      say(d > 0.05, `and at nothing the gathered light goes (${(d * 100).toFixed(1)}% of the frame)`);
    }

    // Every feature switched off is a smaller program, not the same one taking
    // a different branch — which is the whole point of the exercise. There are
    // three objects, so anything past three is a feature set of its own; by
    // this point the run has been through a good many of them.
    await pose(page, { ...flat, shadowOn: 1 });
    await shot(page, "feat-shadow");
    const built = await page.evaluate(() => window.__progs);
    say(built > 3, `a feature switched off is a shader of its own (${built} built so far)`);

    // The air the object stands in. The slider multiplies each object's own
    // thickness, so 1.00 has to be the page exactly as it was.
    await pose(page, { ...flat, haze: 1.0 });
    if (off) say((await shot(page, "haze-one")).equals(off),
                 "Haze at 1 is byte-identical to the baseline");
    await pose(page, { ...flat, haze: 0.0 });
    const clearAir = await shot(page, "haze-none");
    if (off) say(!clearAir.equals(off), "and at nothing the air clears");
    await pose(page, { ...flat, haze: 3.0 });
    const thickAir = await shot(page, "haze-thick");
    // end to end, and in the right direction: thicker air takes the far side of
    // the object toward the background it is standing in front of
    const d = darkened(clearAir, thickAir, 6);
    say(d > 0.03, `and thickening it takes the distance away (${(d * 100).toFixed(1)}% of the frame)`);

    // The lens. Off is the page as it was — the blur is carried in the alpha
    // channel, which nothing was reading, so switching it on is invisible until
    // the post pass is told to look.
    await pose(page, { ...flat, dofOn: 0 });
    if (off) say((await shot(page, "dof-off")).equals(off),
                 "depth of field switched off is byte-identical to the baseline");
    await pose(page, { ...flat, dofOn: 1, dofFocus: 1.0, dofRange: 0.2, dofBlur: 8 });
    const soft = await shot(page, "dof-near");
    say(off && !soft.equals(off), "and switched on it softens what is out of the band");
    await pose(page, { ...flat, dofOn: 1, dofFocus: 1.0, dofRange: 0.2, dofBlur: 0 });
    if (off) say((await shot(page, "dof-noblur")).equals(off),
                 "no blur at all is the sharp frame again, to the byte");
    // The lens is the one thing the pose deliberately leaves alone — every
    // baseline was shot through whatever lens this narrow window picked on
    // load — so put it back afterwards, or every case from here on is framed
    // through a different one and none of them matches its baseline.
    const lens = await page.evaluate(() => window.__state.fov);
    await pose(page, { ...flat, fov: 2.2 });
    say(off && !(await shot(page, "fov-long")).equals(off), "and the lens changes what it takes in");
    await pose(page, { ...flat, fov: lens });

    // Travelling from the object toward a plain ball. At nothing it is the page
    // as it was; all the way it is a ball, whichever object it started from.
    await pose(page, { ...flat, ball: 0 });
    if (off) say((await shot(page, "ball-none")).equals(off),
                 "no blend to the sphere is byte-identical to the baseline");
    await pose(page, { ...flat, ball: 0.5 });
    const half = await shot(page, "ball-half");
    await pose(page, { ...flat, ball: 1.0 });
    const full = await shot(page, "ball-full");
    say(off && !half.equals(off) && !full.equals(half),
        "and it travels the whole way rather than jumping");
    // Square-on, with the light straight down the view so nothing shades it
    // sideways: this is the rig that can tell a round thing from a lopsided one.
    const square = { scene: 1, fly: 1, flyPos: [0, 0, 2.6], flyYaw: Math.PI, flyPitch: 0,
                     shift: 0, noise: 0, warpOn: [0, 0, 0], warpAmt: 0,
                     shadowOn: 1, shadowSrc: 2, shadowOnly: 1, sunAz: 0, sunEl: 0.6 };
    await pose(page, { ...square, ball: 0 });
    const notRound = lopsided(await shot(page, "ball-square-off"), 4);
    await pose(page, { ...square, ball: 1.0 });
    const round = lopsided(await shot(page, "ball-square-on"), 4);
    say(notRound > 0.05, `square-on, the neuron is nothing like round (${(notRound * 100).toFixed(1)}%)`);
    say(round < 0.04 && round < notRound / 4,
        `and all the way across it is a ball (${(notRound * 100).toFixed(1)}% -> ${(round * 100).toFixed(2)}%)`);
    // each object starts somewhere different and so arrives at a ball of its own
    await pose(page, { ...flat, ball: 1.0 });
    const ballC = await shot(page, "ball-chrome");
    await pose(page, { ...flat, scene: 0, ball: 1.0 });
    say(!ballC.equals(await shot(page, "ball-brain")),
        "each object arrives at a ball of its own size");

    // Walking: put the camera in the air over the chrome and let go of it. The
    // ground it lands on comes back from the shader, one ray at a time.
    // The two slabs are unioned rather than intersected for this one: the
    // default intersects them, and where the height field lifts one clear of
    // the other there is no solid at all — including the column straight down
    // from the origin, which is where this drops the camera.
    await pose(page, { scene: 2, shadowOn: 0, fly: 1, walkOn: 1, lookSmooth: 0, moveSmooth: 0,
                       flyPos: [0, 3, 0], flyYaw: Math.PI, flyPitch: 0,
                       warpOn: [0, 0, 0], warpAmt: 0, lcOn: [1, 1, 1, 0, 1, 0] });
    const frames = () => page.evaluate(() => new Promise((d) => {
      let n = 0; const t = () => (++n < 12 ? requestAnimationFrame(t) : d()); requestAnimationFrame(t);
    }));
    // A fall of three units under this gravity is a bit over a second, and a
    // frame here is worth about 20 ms of it, so give it enough of them to land.
    for (let i = 0; i < 10; i++) await frames();
    const landed = await page.evaluate(() => ({ y: window.__state.flyPos[1], w: window.__walk }));
    await frames();
    const still = await page.evaluate(() => window.__state.flyPos[1]);
    say(landed.w.ground >= 0, `the shader reports what is underfoot (${landed.w.ground.toFixed(3)})`);
    say(landed.y < 1.0 && landed.y > -0.2, `and gravity brings the camera down onto it (3.0 -> ${landed.y.toFixed(3)})`);
    say(landed.w.onGround && Math.abs(still - landed.y) < 1e-6, "and it stays there rather than sinking through");
    // a jump leaves the ground and comes back
    await page.evaluate(() => window.dispatchEvent(
      new KeyboardEvent("keydown", { code: "Space", key: " ", bubbles: true })));
    const up = await page.evaluate(() => ({ vy: window.__walk.vy, y: window.__state.flyPos[1] }));
    say(up.vy > 0, `space lifts it off the ground (${up.vy.toFixed(2)} up)`);
    await frames(); await frames(); await frames(); await frames();
    const down = await page.evaluate(() => ({ y: window.__state.flyPos[1], on: window.__walk.onGround }));
    say(down.on && Math.abs(down.y - landed.y) < 0.02,
        `and it comes back down to the same ground (${down.y.toFixed(3)})`);

    // Controller size. The same ground, and a walker a tenth as tall stands a
    // tenth as high off it.
    await pose(page, { scene: 2, shadowOn: 0, fly: 1, walkOn: 1, lookSmooth: 0, moveSmooth: 0,
                       flyPos: [0, 3, 0], flyYaw: Math.PI, flyPitch: 0, walkSize: 0.1,
                       warpOn: [0, 0, 0], warpAmt: 0, lcOn: [1, 1, 1, 0, 1, 0] });
    for (let i = 0; i < 10; i++) await frames();
    const small = await page.evaluate(() => ({ y: window.__state.flyPos[1], on: window.__walk.onGround }));
    say(small.on && small.y > 0 && small.y < landed.y * 0.35,
        `a smaller controller stands lower on the same ground (${landed.y.toFixed(3)} -> ${small.y.toFixed(3)})`);

    // Standing still is exactly still. The ray starts at the eye, so the answer
    // depends on where the eye is, and correcting the eye by it moves the next
    // ray: chasing it never converges. This spot used to sit in a two-frame
    // cycle a millimetre and a half wide, for ever.
    await pose(page, { scene: 2, shadowOn: 0, fly: 1, walkOn: 1, lookSmooth: 0, moveSmooth: 0,
                       flyPos: [-0.75, 1.4, -0.50], flyYaw: 0, flyPitch: 0,
                       warpOn: [1, 0, 0], warpAmt: 0.79, eps: 5.2, omega: 1.68,
                       bound: 0, boundPad: 0.6, lcRelief: 0.28, lcFreq: 5,
                       lcThick: 0.052, lcTwirl: 2.1, lcOn: [1, 1, 1, 1, 1, 0] });
    for (let i = 0; i < 14; i++) await frames();
    const held = await page.evaluate(() => new Promise((done) => {
      const ys = []; let n = 0;
      const t = () => { ys.push(window.__state.flyPos[1]);
        if (++n < 30) requestAnimationFrame(t); else done(ys); };
      requestAnimationFrame(t);
    }));
    const swing = Math.max(...held) - Math.min(...held);
    say(swing === 0, `a walker standing still does not drift or shiver (${swing.toExponential(1)})`);

    // ...and holding still is not the same as being stuck: a floor that is
    // really moving is followed, because two readings in a row agree about it.
    await page.evaluate(() => { window.__state.morph = true; window.__state.mClock = 0; });
    const alive = await page.evaluate(() => new Promise((done) => {
      const ys = []; let n = 0;
      const t = () => { ys.push(window.__state.flyPos[1]);
        if (++n < 40) requestAnimationFrame(t); else done(ys); };
      requestAnimationFrame(t);
    }));
    const rode = Math.max(...alive) - Math.min(...alive);
    say(rode > 1e-4, `and a floor that is really moving is ridden, not ignored (${rode.toExponential(1)})`);
    await page.evaluate(() => { window.__state.morph = false; });

    // Every shortcut is read by where the key is, not by what it prints: on a
    // Russian layout P prints з, and the page has to take it just the same.
    const foreign = await page.evaluate(() => {
      const was = window.__state.morph;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "\u0437", code: "KeyP", bubbles: true }));
      const flipped = window.__state.morph !== was;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "\u0437", code: "KeyP", bubbles: true }));
      // and the fly keys, which are held rather than pressed
      window.__state.fly = 1;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "\u0446", code: "KeyW", bubbles: true }));
      const walking = !!window.__held.w;
      window.dispatchEvent(new KeyboardEvent("keyup", { key: "\u0446", code: "KeyW", bubbles: true }));
      return { flipped, walking, let_go: !window.__held.w, back: window.__state.morph === was };
    });
    say(foreign.flipped && foreign.back, "a key on another layout still plays and pauses the clock");
    say(foreign.walking && foreign.let_go, "and still drives the walk, down and up again");

    // The seed. Zero is the roll every baseline was shot at, and the picture is
    // that one to the byte; any other seed is another object of the same kind.
    await pose(page, { ...flat, seed: 0 });
    if (off) say((await shot(page, "seed-zero")).equals(off),
                 "seed zero is byte-identical to the baseline");
    await pose(page, { ...flat, seed: 7 });
    const roll7 = await shot(page, "seed-7");
    say(off && !roll7.equals(off), "and another seed is another object of the same kind");
    await pose(page, { ...flat, seed: 8 });
    say(!roll7.equals(await shot(page, "seed-8")), "and no two seeds are the same roll");
    // it reaches the other two objects as well
    await pose(page, { ...flat, scene: 1, seed: 0 });
    const nSeed0 = await shot(page, "seed-neuron-0");
    await pose(page, { ...flat, scene: 1, seed: 5 });
    say(!nSeed0.equals(await shot(page, "seed-neuron-5")), "and it re-rolls the neuron too");

    // Time. P plays and pauses the field's own clock, and the clock can be put
    // anywhere by hand.
    await pose(page, { ...flat, morph: 1, mClock: 5.0 });
    const tick = await page.evaluate(async () => {
      const key = () => window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "p", code: "KeyP", bubbles: true }));
      const wait = () => new Promise((d) => {
        let n = 0; const t = () => (++n < 10 ? requestAnimationFrame(t) : d()); requestAnimationFrame(t);
      });
      const a = window.__state.mClock;
      await wait();
      const b = window.__state.mClock;          // running
      key();
      const playing = window.__state.morph;
      await wait();
      const c = window.__state.mClock;          // held
      await wait();
      const d = window.__state.mClock;
      const held = document.getElementById("tTimeOut").textContent;
      key();
      await wait();
      const e = window.__state.mClock;          // running again
      return { a, b, c, d, e, playing, held,
               reads: document.getElementById("tTimeOut").textContent };
    });
    say(tick.b > tick.a, `the field's clock runs while it is playing (${tick.a.toFixed(2)} -> ${tick.b.toFixed(2)})`);
    say(!tick.playing && tick.d === tick.c, `and P holds it exactly still (${tick.c.toFixed(2)})`);
    say(tick.e > tick.d, "and P again lets it go");
    say(tick.held === tick.d.toFixed(2) + " s" && tick.reads === tick.e.toFixed(2) + " s",
        `and the console shows the clock, held and running (${tick.held} / ${tick.reads})`);

    // ---- octaves --------------------------------------------------------
    // The relief has always been two layers of noise and the warp one. Making
    // that a number has to leave those two counts drawing exactly what they
    // drew, or every baseline in this file is a baseline for something else.
    await pose(page, { ...flat, lcOct: 2, warpOct: 1 });
    if (off) say((await shot(page, "oct-as-was")).equals(off),
                 "two octaves of relief and one of warp is the page as it was");

    await pose(page, { ...flat, lcOct: 4 });
    const deep = await shot(page, "oct-relief-4");
    say(off && !deep.equals(off), "more octaves of relief put finer detail on the landscape");
    await pose(page, { ...flat, lcOct: 1 });
    const flatr = await shot(page, "oct-relief-1");
    say(!flatr.equals(deep), "and one octave is smoother again");

    // Adding octaves must add detail and not height: normalised back to what
    // the old two summed to, so the silhouette stays where Relief put it.
    const bulk = (png) => {
      const d = decode(png);
      let n = 0, total = 0;
      for (let i = 0; i + 2 < d.data.length; i += d.bpp) {
        total++;
        if (0.299 * d.data[i] + 0.587 * d.data[i + 1] + 0.114 * d.data[i + 2] > 60) n++;
      }
      return n / total;
    };
    const b1 = bulk(flatr), b4 = bulk(deep);
    say(Math.abs(b1 - b4) < 0.05,
        `and octaves add detail rather than size (${(b1 * 100).toFixed(1)}% vs ${(b4 * 100).toFixed(1)}% of the frame)`);

    // The finer octaves are steeper, and a march not told so walks through the
    // surface: holes read as background where the object plainly is.
    await pose(page, { ...flat, scene: 1, warpOct: 1, warpAmt: 0.9 });
    const w1 = await shot(page, "oct-warp-1");
    await pose(page, { ...flat, scene: 1, warpOct: 4, warpAmt: 0.9 });
    const w4 = await shot(page, "oct-warp-4");
    say(!w1.equals(w4), "more octaves of warp crease the object more finely");
    const solid = (png) => {
      const d = decode(png), stride = d.w * d.bpp;
      let n = 0, total = 0;
      const y0 = Math.floor(d.h * 0.35), y1 = Math.floor(d.h * 0.65);
      const x0 = Math.floor(d.w * 0.35), x1 = Math.floor(d.w * 0.65);
      for (let y = y0; y < y1; y++)
        for (let x = x0; x < x1; x++) {
          const i = y * stride + x * d.bpp; total++;
          if (0.299 * d.data[i] + 0.587 * d.data[i + 1] + 0.114 * d.data[i + 2] > 45) n++;
        }
      return n / total;
    };
    say(solid(w4) > solid(w1) * 0.85,
        `and the march still finds the surface it carved (${(solid(w1) * 100).toFixed(1)}% -> ${(solid(w4) * 100).toFixed(1)}% solid)`);

    // The mirror. Off is the page as it was; on, the object is folded in half
    // about the plane and the half that is left is drawn on both sides.
    await pose(page, { ...flat, mirror: [0, 0] });
    if (off) say((await shot(page, "mirror-off")).equals(off),
                 "no fold is byte-identical to the baseline");
    await pose(page, { ...flat, mirror: [1, 0] });
    const mx = await shot(page, "mirror-x");
    await pose(page, { ...flat, mirror: [0, 1] });
    const my = await shot(page, "mirror-y");
    await pose(page, { ...flat, mirror: [1, 1] });
    const mb = await shot(page, "mirror-both");
    say(off && !mx.equals(off) && !my.equals(off) && !mx.equals(my) && !mb.equals(mx) && !mb.equals(my),
        "each fold, and the pair of them, is its own shape");

    // Square-on to a folded object, the picture has to be its own reflection.
    // The neuron is the least symmetric thing here, so it is the one to ask.
    await pose(page, { ...square, mirror: [0, 0] });
    const bare = lopsided(await shot(page, "mirror-square-off"), 4);
    await pose(page, { ...square, mirror: [1, 0] });
    const fold = lopsided(await shot(page, "mirror-square-on"), 4);
    say(bare > 0.05, `square-on, the neuron is not symmetric to start with (${(bare * 100).toFixed(1)}%)`);
    // the shot is the canvas at its CSS size and not its backing store, so a
    // perfectly symmetric render still resamples to a few uneven edge pixels
    say(fold < bare / 4 && fold < 0.04,
        `and folded about x it is its own reflection (${(bare * 100).toFixed(1)}% off -> ${(fold * 100).toFixed(2)}%)`);

    // The depth decal. A beam with no bite in it has to cost the march nothing
    // whatsoever: its bounding box is evaluated per sample and a box that only
    // ever shortens a step would still move where a ray lands.
    await pose(page, { ...flat, depthOn: 1, depthMc: 1, depthAmt: 0.0 });
    if (off) say((await shot(page, "decal-none")).equals(off),
                 "a decal with no Intensity is byte-identical to the baseline");

    await pose(page, { ...flat, depthOn: 1, depthMc: 1, depthAmt: 0.40 });
    const dec = await shot(page, "decal-on");
    if (off) say(!dec.equals(off), "and with some, the picture pushes the surface out");

    await pose(page, { ...flat, depthOn: 1, depthMc: 1, depthAmt: -0.40 });
    say(!(await shot(page, "decal-carve")).equals(dec), "Intensity below zero carves in instead");

    await pose(page, { ...flat, depthOn: 1, depthMc: 1, depthAmt: 0.40, depthDist: 0.12 });
    say(!(await shot(page, "decal-short")).equals(dec), "and Distance sets how far the beam reaches");

    await pose(page, { ...flat, depthOn: 1, depthMc: 1, depthAmt: 0.40,
                       depthPos: [0, 0, 3.40] });
    if (off) say((await shot(page, "decal-away")).equals(off),
                 "a card standing clear of the object touches nothing");

    // Aiming it. The cube and the slab are separate parts of this one object,
    // and a decal aimed at either has to leave the other exactly as it was.
    const aim = { ...flat, depthOn: 1, depthMc: 1, depthAmt: 0.22, steps: 150,
                  lcOn: [1, 1, 1, 1, 1, 1], depthPos: [0, 0.58, 1.20], depthDist: 1.60 };
    await pose(page, { ...aim, depthPart: [0, 0, 0] });
    const pAll = await shot(page, "decal-part-all");
    await pose(page, { ...aim, depthPart: [0, 0, 1] });
    const pSlab = await shot(page, "decal-part-slab");
    await pose(page, { ...aim, depthPart: [0, 0, 2] });
    const pCube = await shot(page, "decal-part-cube");
    say(!pAll.equals(pSlab) && !pAll.equals(pCube) && !pSlab.equals(pCube),
        "the slab, the cube and the two together each take the decal differently");
  }

  // The cube is the one shadow anybody can check by eye, and the only test here
  // that asks whether the feature WORKS rather than whether it changed. A box
  // over flat ground with the sun well up: take the caster away and nothing else,
  // and the pixels that got lighter are its shadow and nothing else.
  {
    const rig = {
      scene: 2, warpOn: [0, 0, 0], warpAmt: 0, shadowOn: 1,
      shadowSoft: 0.0, shadowDark: 1.0, shadowReach: 4.0, sunAz: 0.9, sunEl: 0.9,
      fly: 0, mode: 0, dragAz: 0.0, dragEl: 0.50, zoom: 0.46,
    };
    await pose(page, { ...rig, lcOn: [1, 0, 0, 0, 0, 0] });
    const noCube = await shot(page, "cube-absent");
    await pose(page, { ...rig, lcOn: [1, 0, 0, 0, 0, 1] });
    const withCube = await shot(page, "cube-present");
    const sh = darkened(noCube, withCube, 40);
    say(sh > 0.01, `the test cube casts a shadow on the slab (${(sh * 100).toFixed(1)}% of the frame)`);

    // and it is the sun that does it, not the cube merely being in the way
    await pose(page, { ...rig, lcOn: [1, 0, 0, 0, 0, 1], shadowOn: 0 });
    const cubeUnlit = await shot(page, "cube-nosun");
    const gone = darkened(cubeUnlit, withCube, 40);
    say(gone > 0.01, `and the shadow is gone with the sun switched off (${(gone * 100).toFixed(1)}%)`);
  }

  if (mode === "check") {
    // Who owns which button. The sun has the left drag from the start, the camera
    // always has the right one, and L swaps the left one back and forth.
    await pose(page, { scene: 2, shadowOn: 1 });
    const box = await page.locator("canvas").first().boundingBox();
    const cx = box.x + box.width / 2, cy = box.y + box.height / 2;
    const look = () => page.evaluate(() => {
      const s = window.__state;
      return { az: s.sunAz, el: s.sunEl, dAz: s.dragAz, dEl: s.dragEl };
    });
    const drag = async (button) => {
      await page.mouse.move(cx, cy);
      await page.mouse.down({ button });
      await page.mouse.move(cx + 90, cy + 40, { steps: 6 });
      await page.mouse.up({ button });
    };
    const inMode = () => page.evaluate(() => !!window.__sunMode);

    say(await inMode(), "sun mode is on out of the box");

    let a = await look(); await drag("left"); let b = await look();
    say(Math.abs(b.az - a.az) > 1e-3 && Math.abs(b.el - a.el) > 1e-3,
        "left-drag swings the sun");
    say(b.dAz === a.dAz && b.dEl === a.dEl, "and the camera is left exactly where it was");

    await pose(page, { scene: 2, shadowOn: 1 });
    a = await look(); await drag("right"); b = await look();
    say(b.dAz !== a.dAz && b.dEl !== a.dEl, "right-drag turns the camera even so");
    say(b.az === a.az && b.el === a.el, "and the sun stays where it was put");

    await page.keyboard.press("l");
    say(!(await inMode()), "L hands the left button back to the camera");
    await pose(page, { scene: 2, shadowOn: 1 });
    a = await look(); await drag("left"); b = await look();
    say(b.dAz !== a.dAz, "left-drag turns the camera once the sun has let go");
    say(b.az === a.az && b.el === a.el, "and the sun does not move with it");
    await page.keyboard.press("l");
    say(await inMode(), "L takes it back again");
  }

  // ---- the console itself ----------------------------------------------
  // Its own page, because the checks above hide the interface and work at a
  // size no menu would fit in. Nothing here touches the shader: it is about
  // whether the thing can be taken apart and put back together.
  if (mode === "check") {
    const ui = await browser.newPage({ viewport: { width: 1000, height: 1300 } });
    ui.on("pageerror", (e) => { if (ours(String(e))) errors.push("console: " + e); });
    ui.on("dialog", (d) => d.accept("Night"));
    await ui.goto(page_url);
    await ui.waitForTimeout(1400);

    const only = async (list) => {
      await ui.evaluate((l) => {
        document.querySelectorAll(".grpt").forEach((h) => {
          const want = l.indexOf(+h.dataset.grp) >= 0;
          if ((h.getAttribute("aria-expanded") === "true") !== want) h.click();
        });
        document.querySelector(".console").scrollTop = 0;
      }, list);
      await ui.waitForTimeout(220);
    };
    // by the grip, or by the header — a real pointer, because that is the only
    // gesture there is: the browser's own drag and drop leaves touch out
    const drag = async (from, to, atBottom) => {
      const a = await ui.locator(from).boundingBox();
      const b = await ui.locator(to).boundingBox();
      await ui.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
      await ui.mouse.down();
      await ui.mouse.move(a.x + a.width / 2, a.y + a.height / 2 + 12, { steps: 3 });
      const y = atBottom ? b.y + b.height - 4 : b.y + 4;
      await ui.mouse.move(b.x + b.width / 2, y, { steps: 10 });
      await ui.mouse.move(b.x + b.width / 2, y, { steps: 2 });
      await ui.mouse.up();
      await ui.waitForTimeout(180);
    };

    // a slider offers its way back, and only while there is one to offer
    const chip = await ui.evaluate(() => {
      const c = document.querySelector('.dflt[data-dflt="shSoft"]');
      const before = c.hidden;
      const i = document.getElementById("shSoft");
      i.value = 70; i.dispatchEvent(new Event("input"));
      const shown = !c.hidden, txt = c.textContent;
      c.click();
      const back = window.__state.shadowSoft;
      i.value = 70; i.dispatchEvent(new Event("input"));
      i.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
      return { before, shown, txt, back, afterDouble: window.__state.shadowSoft };
    });
    say(chip.before && chip.shown, "the default only shows once a slider has been moved");
    say(Math.abs(chip.back - 0.25) < 1e-9, `clicking it puts the default back (${chip.txt})`);
    say(Math.abs(chip.afterDouble - 0.25) < 1e-9, "and so does double-clicking the slider");

    say(await ui.evaluate(() => document.querySelectorAll(".gico").length ===
                                document.querySelectorAll(".grpt").length),
        "every group header carries its own icon");
    const heights = await ui.evaluate(() => {
      const h = document.querySelector(".grph").getBoundingClientRect().height;
      // a button that is definitely on screen: the first one is inside a module
      // this object does not show, and a hidden element measures zero
      const b = document.getElementById("benchBtn").getBoundingClientRect().height;
      return [Math.round(h), Math.round(b)];
    });
    say(heights[0] === heights[1], `a header is exactly as tall as a button (${heights.join(" / ")})`);

    const m = await ui.evaluate(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "m", code: "KeyM", bubbles: true }));
      const shut = window.__state.groups.every((g) => !g);
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "m", code: "KeyM", bubbles: true }));
      const open = window.__state.groups.every((g) => !!g);
      const was = window.__state.morph;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "M", code: "KeyM", shiftKey: true, bubbles: true }));
      return { shut, open, morph: window.__state.morph !== was };
    });
    say(m.shut && m.open, "M folds every group away, and brings them all back");
    say(m.morph, "and the field's own animation is on shift+M");

    await ui.evaluate(() => document.getElementById("rgOn").click());
    say(await ui.evaluate(() => getComputedStyle(
          document.querySelector('.mod[data-mod="sphere"] .mgrip')).display !== "none"),
        "arranging shows a grip on every module");

    await only([4, 5]);
    await drag('.mod[data-mod="sphere"] .mgrip', '.mod[data-mod="gopacity"]', true);
    const moved = await ui.evaluate(() => ({
      gid: document.querySelector('.mod[data-mod="sphere"]').closest(".grp").dataset.gid,
      dirty: !document.getElementById("dirty").hidden,
      inState: ((window.__state.uiMods || {})["5"] || []).indexOf("sphere") >= 0,
    }));
    say(moved.gid === "5", "a module can be dragged into another group");
    say(moved.inState, "and the move is in the state, not only on screen");
    say(moved.dirty, "and the console says so until it is saved");

    await only([5]);
    // The console grows every time a group is added, and a drag only lands if
    // BOTH ends are on screen: the last module of a group can sit past the
    // bottom of the page even with everything else folded away.
    await ui.evaluate(() => document.querySelector('.mod[data-mod="gdepth"]').scrollIntoView({ block: "center" }));
    await ui.waitForTimeout(250);
    await drag('.mod[data-mod="gdepth"] .mgrip', '.mod[data-mod="gopacity"]', false);
    const inside = await ui.evaluate(() => (window.__state.uiMods["5"] || []).join(","));
    say(inside.indexOf("gdepth") < inside.indexOf("gopacity"), "and reordered inside one group");

    await only([]);
    await drag('.grp[data-gid="6"] .grph', '.grp[data-gid="0"]', false);
    const order = await ui.evaluate(() => window.__state.uiOrder.join(","));
    say(order.indexOf("6") < order.indexOf("0"), `a group can be dragged above another (${order})`);
    say(await ui.evaluate(() => document.querySelectorAll(".grp .grp").length === 0),
        "and no group ever ends up inside another");

    await only([5]);
    await ui.evaluate(() => {
      const go = (pen, name, text) => {
        pen.click();
        name.textContent = text;
        name.dispatchEvent(new FocusEvent("blur"));
      };
      go(document.querySelector('.grp[data-gid="5"] .grph .pen'),
         document.querySelector('.grp[data-gid="5"] .grpn'), "Glassy");
      go(document.querySelector('.mod[data-mod="gedge"] .pen'),
         document.querySelector('.mod[data-mod="gedge"] .modt'), "Rim");
    });
    const named = await ui.evaluate(() => [window.__state.uiNames["g:5"], window.__state.uiNames["m:gedge"]]);
    say(named[0] === "Glassy" && named[1] === "Rim", `a group and a module can be renamed (${named.join(", ")})`);

    const trip = await ui.evaluate(() => {
      const keep = JSON.stringify({ o: window.__state.uiOrder, m: window.__state.uiMods, n: window.__state.uiNames });
      document.getElementById("rgOn").click();
      document.getElementById("saveBtn").click();
      const clean = document.getElementById("dirty").hidden;
      const k = JSON.parse(keep);
      window.__state.uiOrder = k.o; window.__state.uiMods = k.m; window.__state.uiNames = k.n;
      document.getElementById("resetBtn").click();
      return { clean,
               gid: document.querySelector('.mod[data-mod="sphere"]').closest(".grp").dataset.gid,
               name: document.querySelector('.grp[data-gid="5"] .grpn').textContent };
    });
    say(trip.clean, "saving as the default takes that notice away");
    say(trip.gid === "5" && trip.name === "Glassy", "and the arrangement comes back out of the saved settings");

    await only([3, 8]);
    await ui.evaluate(() => { window.__state.shadowDark = 0.11; });
    await ui.click("#tplAdd");
    await ui.waitForTimeout(350);
    const tpl = await ui.evaluate(() => ({
      n: document.querySelectorAll("#tplSeg button").length,
      label: (document.querySelector("#tplSeg button") || {}).textContent }));
    say(tpl.n === 1 && /Night/.test(tpl.label || ""), `a template is kept under its name (${tpl.label})`);
    const recall = await ui.evaluate(() => {
      window.__state.shadowDark = 0.99;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "1", code: "Digit1", altKey: true, bubbles: true }));
      return window.__state.shadowDark;
    });
    say(Math.abs(recall - 0.11) < 1e-9, "alt+1 recalls it");
    const digits = await ui.evaluate(() => {
      const was = window.__state.resPin;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "3", code: "Digit3", altKey: true, bubbles: true }));
      const held = window.__state.resPin === was;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "3", code: "Digit3", bubbles: true }));
      return { held, plain: window.__state.resPin === 2 };
    });
    say(digits.held, "and alt+3 does not also do what 3 does");
    say(digits.plain, "while 3 on its own still does");

    const reset = await ui.evaluate(() => {
      const i = document.getElementById("shLevel");
      i.value = 20; i.dispatchEvent(new Event("input"));
      document.getElementById("saveBtn").click();
      const saved = window.__state.shadowLevel;
      i.value = 80; i.dispatchEvent(new Event("input"));
      document.getElementById("resetBtn").click();
      return { saved, now: window.__state.shadowLevel,
               tpls: document.querySelectorAll("#tplSeg button").length };
    });
    say(Math.abs(reset.now - reset.saved) < 1e-9, "Reset goes back to the last saved default");
    say(reset.tpls === 1, "and leaves the templates alone");

    // ---- the camera group ----------------------------------------------
    await only([10]);
    const cam = await ui.evaluate(() => {
      const s = window.__state;
      s.flyMin = 0.030; s.flyMax = 12.0; s.flySpeed = 0.20;
      // wind the floor up past where the speed is and the speed comes with it
      const i = document.getElementById("camMin");
      i.value = 1000; i.dispatchEvent(new Event("input"));
      const dragged = s.flySpeed;
      // and a number typed into the reading goes below what the slider reaches
      const out = document.getElementById("camMinOut");
      out.textContent = "0.0004";
      out.dispatchEvent(new FocusEvent("blur"));
      return { dragged, floor: s.flyMin, min: s.flyMin };
    });
    say(Math.abs(cam.dragged - 1.0) < 1e-6,
        `raising the floor past the speed carries the speed with it (${cam.dragged})`);
    say(cam.floor < 0.001, `and a typed number goes below the slider (${cam.floor})`);

    // ---- the overlay ----------------------------------------------------
    // A plain element over the canvas, so the check is where it stands and how
    // big it is rather than anything about pixels.
    await only([12]);
    await ui.evaluate(() => {
      const s = window.__state;
      // this page draws about a frame a second at full size, and the sheet is
      // only picked up on a frame: make frames cheap before waiting on one
      s.resPin = 1; s.scaleQ = 0.30; s.stepsPin = true; s.steps = 48; s.aa = 0;
      s.sprite = 0; s.spriteFrames = 4; s.spriteSize = 0.20;
      document.getElementById("spOn").click();
    });
    await ui.evaluate(() => new Promise((done) => {
      let n = 0;
      const tick = () => (++n < 10 ? requestAnimationFrame(tick) : done());
      requestAnimationFrame(tick);
    }));
    await ui.waitForTimeout(600);
    const ov = await ui.evaluate(() => {
      const el = document.getElementById("sprite");
      return { on: el.classList.contains("on"),
               h: parseFloat(el.style.height) || 0,
               size: getComputedStyle(el).backgroundSize,
               bottom: parseFloat(getComputedStyle(el).bottom) || 0,
               H: document.getElementById("stage").clientHeight };
    });
    say(ov.on, "the overlay puts the sprite on screen");
    say(Math.abs(ov.h - ov.H * 0.20) < 1.5,
        `a fifth of the window tall (${ov.h.toFixed(0)} of ${ov.H})`);
    say(Math.abs(ov.bottom - ov.H * 0.05) < 1.5,
        `standing a twentieth of it clear of the bottom (${ov.bottom.toFixed(0)})`);
    say(/400%/.test(ov.size), `and four frames wide in the sheet (${ov.size})`);
    const grew = await ui.evaluate(() => {
      const i = document.getElementById("spSize");
      i.value = 40; i.dispatchEvent(new Event("input"));
      return parseFloat(document.getElementById("sprite").style.height) || 0;
    });
    say(grew > ov.h * 1.8, `and Size makes him bigger (${ov.h.toFixed(0)} -> ${grew.toFixed(0)})`);

    // A sheet is a cut-out, and a cut-out that comes back as a JPEG has a black
    // box round it. Load one too big to keep as it is — noise, so it will not
    // compress away — with a clear half, and see whether the clear half lived.
    await ui.evaluate(() => new Promise((done) => {
      const c = document.createElement("canvas");
      c.width = 1200; c.height = 400;
      const g = c.getContext("2d");
      const d = g.createImageData(600, 400);
      for (let i = 0; i < d.data.length; i += 4) {
        d.data[i] = Math.random() * 255; d.data[i + 1] = Math.random() * 255;
        d.data[i + 2] = Math.random() * 255; d.data[i + 3] = 255;
      }
      g.putImageData(d, 0, 0);          // left half opaque noise, right half untouched
      c.toBlob((blob) => {
        const dt = new DataTransfer();
        dt.items.add(new File([blob], "sheet.png", { type: "image/png" }));
        const inp = document.getElementById("mcFile");
        document.getElementById("spAdd").click();
        inp.files = dt.files;
        inp.dispatchEvent(new Event("change"));
        done();
      }, "image/png");
    }));
    await ui.waitForTimeout(1500);
    await ui.evaluate(() => new Promise((done) => {
      let n = 0;
      const tick = () => (++n < 10 ? requestAnimationFrame(tick) : done());
      requestAnimationFrame(tick);
    }));
    const kept = await ui.evaluate(() => new Promise((done) => {
      const bg = getComputedStyle(document.getElementById("sprite")).backgroundImage;
      const m = /url\("?(data:[^"')]+)"?\)/.exec(bg);
      if (!m) return done({ err: "no sheet on the element" });
      const isPng = m[1].indexOf("data:image/png") === 0;
      const img = new Image();
      img.onload = () => {
        const c = document.createElement("canvas");
        c.width = img.naturalWidth; c.height = img.naturalHeight;
        const g = c.getContext("2d");
        g.drawImage(img, 0, 0);
        const px = g.getImageData(Math.round(c.width * 0.85), Math.round(c.height * 0.5), 1, 1).data;
        done({ isPng, a: px[3], w: c.width, len: m[1].length });
      };
      img.onerror = () => done({ err: "will not decode" });
      img.src = m[1];
    }));
    say(kept.isPng === true, `a sprite sheet too big to keep whole stays a PNG (${kept.err || kept.w + "px, " + Math.round(kept.len / 1024) + "kB"})`);
    say(kept.a !== undefined && kept.a < 10,
        `and its see-through half is still see-through (alpha ${kept.a})`);
    await ui.evaluate(() => { document.getElementById("spOn").click(); });
    say(await ui.evaluate(() => !document.getElementById("sprite").classList.contains("on")),
        "and off takes him away again");

    // ---- a template remembers when, not just what -----------------------
    await only([8]);
    const when = await ui.evaluate(() => {
      const s = window.__state;
      // stop the clocks, or the frame between the recall and the reading
      // moves them on and there is nothing exact left to compare
      s.running = false; s.morph = false;
      s.clock = 77.5; s.mClock = 21.25; s.pClock = 3.5; s.shadowDark = 0.42;
      return 1;
    });
    await ui.click("#tplUpd");
    await ui.waitForTimeout(350);
    const back = await ui.evaluate(() => {
      const s = window.__state;
      s.clock = 5; s.mClock = 5; s.pClock = 5; s.shadowDark = 0.9;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "1", code: "Digit1", altKey: true, bubbles: true }));
      return { clock: s.clock, m: s.mClock, p: s.pClock, dark: s.shadowDark,
               n: document.querySelectorAll("#tplSeg button").length };
    });
    say(when === 1 && back.n === 1, "Update writes over the template you are on rather than adding one");
    say(Math.abs(back.dark - 0.42) < 1e-9, "and what it writes is what comes back");
    say(Math.abs(back.clock - 77.5) < 1e-9 && Math.abs(back.m - 21.25) < 1e-9 &&
        Math.abs(back.p - 3.5) < 1e-9,
        `a template lands on the same frame of the animation (${back.clock} / ${back.m} / ${back.p})`);

    // ---- and the fader across to the next one ---------------------------
    await ui.evaluate(() => { window.__state.shadowDark = 0.90; window.__state.scene = 0; });
    await ui.click("#tplAdd");
    await ui.waitForTimeout(400);
    const fade = await ui.evaluate(() => {
      const s = window.__state;
      const mix = document.getElementById("tplMix");
      // sitting on the second, so the next one round is the first: 0.42
      const from = s.shadowDark;
      mix.value = 50; mix.dispatchEvent(new Event("input"));
      const half = s.shadowDark;
      const scene = s.scene;
      mix.value = 100; mix.dispatchEvent(new Event("input"));
      return { from, half, scene, end: s.shadowDark, back: +mix.value, land: s.scene };
    });
    say(Math.abs(fade.half - 0.66) < 0.02,
        `the fader crosses from here to the next (${fade.from} -> ${fade.half.toFixed(2)} -> ${fade.end})`);
    say(fade.scene === 0 && fade.land === 0, "and leaves the object where it is, the whole way across");
    say(Math.abs(fade.end - 0.42) < 1e-9 && fade.back === 0,
        "all the way across lands on it and the fader goes back to nothing");

    // A heading has no size, only a direction: crossing from 6.2 to 0.2 is a
    // sixth of a turn, not five sixths of one the other way.
const spin = await ui.evaluate(() => {
      const s = window.__state;
      const mix = document.getElementById("tplMix");
      s.flyYaw = 6.20;                       // just short of all the way round
      mix.value = 0; mix.dispatchEvent(new Event("input"));
      mix.value = 50; mix.dispatchEvent(new Event("input"));
      const half = s.flyYaw;
      mix.value = 0; mix.dispatchEvent(new Event("input"));
      return { half, from: 6.20 };
    });
    // Taken the short way, the half-way heading can never be more than a
    // quarter turn from either end, whatever the far end happens to be.
    const TAU = Math.PI * 2;
    const wrap = (a) => { a = (a + Math.PI) % TAU; if (a < 0) a += TAU; return a - Math.PI; };
    const swung = Math.abs(wrap(spin.half - spin.from));
    say(swung <= Math.PI / 2 + 1e-6,
        `a heading crosses the short way round (a ${swung.toFixed(2)} rad swing to the half-way point)`);

    // Renaming one, and pointing at one without loading it
    const renamed = await ui.evaluate(() => {
      const s = window.__state;
      s.shadowDark = 0.5;
      const was = window.prompt;
      window.prompt = () => "Dawn";
      document.getElementById("tplRen").click();
      window.prompt = was;
      return { labels: Array.prototype.map.call(
                 document.querySelectorAll("#tplSeg button"), (b) => b.textContent).join(" | "),
               dark: s.shadowDark };
    });
    say(/Dawn/.test(renamed.labels) && Math.abs(renamed.dark - 0.5) < 1e-9,
        `Rename changes the name and nothing else (${renamed.labels})`);

    const pick = await ui.evaluate(() => {
      const s = window.__state;
      s.shadowDark = 0.77;
      const btns = document.querySelectorAll("#tplSeg button");
      const want = btns[0].getAttribute("aria-pressed") === "true" ? 1 : 0;
      btns[want].dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
      // the list is rebuilt by the click, so ask the new buttons, not the old
      const now = document.querySelectorAll("#tplSeg button");
      return { chosen: now[want].getAttribute("aria-pressed") === "true", dark: s.shadowDark };
    });
    say(pick.chosen && Math.abs(pick.dark - 0.77) < 1e-9,
        "shift-clicking one points at it without loading it");

    // Moving one along the list moves the number it answers to with it.
    const tplOrder = await ui.evaluate(() => {
      const read = () => Array.prototype.map.call(
        document.querySelectorAll("#tplSeg button"), (b) => b.textContent).join(" | ");
      const before = read();
      // point at the second without loading it, then walk it to the front
      const btns = document.querySelectorAll("#tplSeg button");
      btns[1].dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
      document.getElementById("tplUp").click();
      return { before, after: read() };
    });
    say(tplOrder.before !== tplOrder.after && /^1 /.test(tplOrder.after),
        `a template can be moved along the list (${tplOrder.before}  ->  ${tplOrder.after})`);

    // A number typed past the end of its slider is the whole point of being
    // able to type one, so a template has to give it back as it was put in —
    // not as the slider's own end.
    const beyond = await ui.evaluate(() => {
      const s = window.__state;
      const btns = document.querySelectorAll("#tplSeg button");
      btns[0].dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
      const out = document.getElementById("pGlowOut");
      out.textContent = "10";                       // the slider stops at 4
      out.dispatchEvent(new FocusEvent("blur"));
      const typed = s.glow;
      document.getElementById("tplUpd").click();
      s.glow = 1;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "1", code: "Digit1", altKey: true, bubbles: true }));
      return { typed, back: s.glow, reading: document.getElementById("pGlowOut").textContent };
    });
    say(Math.abs(beyond.typed - 10) < 1e-9,
        `a value can be typed past the end of its slider (${beyond.typed})`);
    say(Math.abs(beyond.back - 10) < 1e-9,
        `and a template gives it back as it was put in (${beyond.back}, reading "${beyond.reading}")`);

    // A sheet with nothing see-through in it says so, rather than quietly
    // standing in a black box and leaving it to be guessed at.
    const flat = await ui.evaluate(() => new Promise((done) => {
      const c = document.createElement("canvas");
      c.width = 256; c.height = 64;
      const g = c.getContext("2d");
      g.fillStyle = "#000"; g.fillRect(0, 0, 256, 64);
      g.fillStyle = "#fff"; g.fillRect(8, 8, 48, 48);
      c.toBlob((blob) => {
        const dt = new DataTransfer();
        dt.items.add(new File([blob], "flat.png", { type: "image/png" }));
        const inp = document.getElementById("mcFile");
        document.getElementById("spAdd").click();
        inp.files = dt.files;
        inp.dispatchEvent(new Event("change"));
        // the list itself carries it, where a toast cannot be overwritten
        setTimeout(() => {
          const btns = document.querySelectorAll("#spSeg button");
          done((btns[btns.length - 1] || {}).title || "");
        }, 900);
      }, "image/png");
    }));
    say(/no see-through/.test(flat), `a flat sheet is called out rather than guessed at ("${flat}")`);

    // and the regression: a saved view is obeyed with no smoothing asked for
    await ui.evaluate(() => {
      const s = window.__state;
      s.lookSmooth = 0; s.moveSmooth = 0; s.fly = 1;
      s.flyPos = [1.5, 2.5, 3.5]; s.flyYaw = 0.5; s.flyPitch = 0.2;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "5", code: "Digit5", shiftKey: true, bubbles: true }));
      s.flyPos = [9, 9, 9]; s.flyYaw = 2.5; s.flyPitch = -0.4;
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "5", code: "Digit5", bubbles: true }));
    });
    await ui.evaluate(() => new Promise((done) => {
      let n = 0;
      const tick = () => (++n < 8 ? requestAnimationFrame(tick) : done());
      requestAnimationFrame(tick);
    }));
    const got = await ui.evaluate(() => {
      const g = { pos: window.__state.flyPos.slice(), yaw: window.__state.flyYaw };
      window.__state.fly = 0;          // hand the camera back before the gizmo checks
      return g;
    });
    say(Math.abs(got.pos[0] - 1.5) < 1e-6 && Math.abs(got.pos[2] - 3.5) < 1e-6 &&
        Math.abs(got.yaw - 0.5) < 1e-6,
        `a saved view stays put with no smoothing (${got.pos.map((v) => v.toFixed(1)).join(",")})`);

    // ---- the decal group, and the gizmo that aims it -------------------
    // The parts a decal can be aimed at belong to the object, so the list has
    // to change when the object does.
    await only([9]);
    const parts = await ui.evaluate(() => {
      const read = () => Array.prototype.map.call(
        document.querySelectorAll("#dmTgt button"), (b) => b.textContent).join(" ");
      document.querySelector("[data-scene='1']").click();
      const neuron = read();
      document.querySelector("[data-scene='2']").click();
      const chrome = read();
      document.querySelector("[data-scene='0']").click();
      return { brain: read(), neuron, chrome };
    });
    say(parts.brain === "Everything Shell Cells", `the brain is a shell and cells (${parts.brain})`);
    say(parts.neuron === "Everything Soma Dendrites Axon", `the neuron is three parts (${parts.neuron})`);
    say(parts.chrome === "Everything Slab Cube", `and the chrome is two (${parts.chrome})`);

    // Place stands the card in front of whatever is on screen, which is also
    // the only way to be sure the gizmo is somewhere a pointer can reach it.
    await ui.evaluate(() => {
      const s = window.__state;
      s.running = false; s.morph = false; s.mode = 0;
      s.dragAz = 0.6; s.dragEl = 0.25; s.zoom = 1.0; s.clock = 12.0;
      s.velAz = 0; s.velEl = 0;                 // a drag leaves inertia behind
      s.resPin = 1; s.scaleQ = 0.30; s.stepsPin = true; s.steps = 48; s.aa = 0;
      s.depthMc = 1;
    });
    const uiFrames = () => ui.evaluate(() => new Promise((done) => {
      let n = 0;
      const tick = () => (++n < 8 ? requestAnimationFrame(tick) : done());
      requestAnimationFrame(tick);
    }));
    await uiFrames();
    await ui.evaluate(() => document.getElementById("dmPlace").click());
    await uiFrames();
    const handles = await ui.evaluate(() => Array.prototype.map.call(
      document.querySelectorAll("#giz [data-h]"), (e) => e.getAttribute("data-h")));
    // three rings, and an arrow and a square for each axis that is not pointing
    // straight at the eye — which the beam's is, square to the view
    say(handles.length >= 7 && handles.indexOf("c") >= 0 &&
        handles.indexOf("r0") >= 0 && handles.indexOf("m0") >= 0 && handles.indexOf("s0") >= 0,
        `the gizmo puts up its handles (${handles.join(" ")})`);
    say(await ui.evaluate(() => !!document.querySelector("#giz .card"),),
        "and draws the card itself, semi-transparent");

    const gizDrag = async (h, dx, dy) => {
      const b = await ui.locator('#giz [data-h="' + h + '"]').boundingBox();
      if (!b) return false;
      const x = b.x + b.width / 2, y = b.y + b.height / 2;
      await ui.mouse.move(x, y);
      await ui.mouse.down();
      await ui.mouse.move(x + dx * 0.4, y + dy * 0.4, { steps: 4 });
      await ui.mouse.move(x + dx, y + dy, { steps: 6 });
      await ui.mouse.up();
      await ui.waitForTimeout(200);
      return true;
    };
    const was = await ui.evaluate(() => ({
      pos: window.__state.depthPos.slice(), dist: window.__state.depthDist }));
    await gizDrag("c", 70, 0);
    const now = await ui.evaluate(() => window.__state.depthPos.slice());
    const slid = Math.hypot(now[0] - was.pos[0], now[1] - was.pos[1], now[2] - was.pos[2]);
    say(slid > 0.05, `dragging the middle of it moves the card (${slid.toFixed(2)})`);

    const wide = await ui.evaluate(() => window.__state.depthSize[0]);
    await gizDrag("s0", 60, 0);
    const wider = await ui.evaluate(() => window.__state.depthSize[0]);
    say(Math.abs(wider - wide) > 0.05,
        `the square on its edge makes the card wider (${wide.toFixed(2)} \u2192 ${wider.toFixed(2)})`);

    // A ring is a stroke, and a bounding box is not on it: press a point the
    // ring itself reports, which is the only one guaranteed to be under it.
    const q0 = await ui.evaluate(() => window.__state.depthRot.slice());
    const rp = await ui.evaluate(() => {
      const el = document.querySelector('#giz [data-h="r2"]');
      if (!el) return null;
      const q = el.getAttribute("points").split(" ")[6].split(",");
      return [+q[0], +q[1]];
    });
    if (rp) {
      await ui.mouse.move(rp[0], rp[1]);
      await ui.mouse.down();
      await ui.mouse.move(rp[0] + 20, rp[1] - 40, { steps: 5 });
      await ui.mouse.move(rp[0] + 45, rp[1] - 85, { steps: 5 });
      await ui.mouse.up();
      await ui.waitForTimeout(200);
    }
    const q1 = await ui.evaluate(() => window.__state.depthRot.slice());
    const turned = Math.max(...q0.map((v, i) => Math.abs(v - q1[i])));
    say(turned > 0.01, `and a ring turns it about that axis (${turned.toFixed(3)})`);

    const shut = await ui.evaluate(() => {
      const fake = new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true });
      window.dispatchEvent(fake);
      document.dispatchEvent(fake);
      return document.getElementById("dmEdit").getAttribute("aria-pressed");
    });
    say(shut === "false", "and Esc puts the gizmo away");

    // ---- T, and the console that comes down from the top ----------------
    const tkey = await ui.evaluate(() => {
      // hide the panels first, so T has to bring them back as well
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "h", code: "KeyH", bubbles: true }));
      const away = document.getElementById("hud").classList.contains("hidden");
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "t", code: "KeyT", bubbles: true }));
      const open = Array.prototype.filter.call(document.querySelectorAll(".grpt"),
        (h) => h.getAttribute("aria-expanded") === "true").map((h) => +h.dataset.grp);
      return { away, back: !document.getElementById("hud").classList.contains("hidden"), open };
    });
    say(tkey.away && tkey.back, "T brings the menu back when the panels are away");
    say(tkey.open.length === 1 && tkey.open[0] === 8,
        `and leaves only Templates open (${tkey.open.join(",") || "none"})`);

    const term = await ui.evaluate(async () => {
      const key = (code, shift) => window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "`", code, shiftKey: !!shift, bubbles: true }));
      const el = document.getElementById("term");
      key("Backquote");
      const opened = el.classList.contains("on");
      const focused = document.activeElement === document.getElementById("termIn");
      // It slides down on a CSS transition, and this headless build starts
      // transitions without ever advancing them: the computed transform sits
      // at the first frame for ever. So take the transition away and measure
      // where it actually comes to rest.
      el.style.transition = "none";
      void el.offsetWidth;
      const r = el.getBoundingClientRect();
      const m = document.querySelector(".console").getBoundingClientRect();
      el.style.transition = "";
      return { opened, focused, left: Math.round(r.left), top: Math.round(r.top),
               gap: Math.round(m.left - r.right) };
    });
    say(term.opened && term.focused, "the ` key brings the console down, ready to type in");
    say(term.left === 0 && term.top === 0, "from the top left corner");
    say(term.gap === 0, `and across to the menu's edge, exactly (${term.gap}px over)`);

    const cmd = await ui.evaluate(async () => {
      const type = (text) => {
        const inp = document.getElementById("termIn");
        inp.value = text;
        inp.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true }));
      };
      window.__state.glow = 1;
      type("set glow 3");
      const set = window.__state.glow;
      type("get glow");
      type("nonsense");
      const lines = document.getElementById("termLog").textContent;
      const before = document.querySelectorAll("#termLog > div").length;
      type("clear");
      return { set, lines, before, after: document.querySelectorAll("#termLog > div").length };
    });
    say(cmd.set === 3, `set changes a setting by name (glow ${cmd.set})`);
    say(/glow = 3/.test(cmd.lines), "get reads it back");
    say(/do not know/.test(cmd.lines), "and it says so when it does not know a word");
    say(cmd.before > 3 && cmd.after === 0, "clear empties the log");

    const dumped = await ui.evaluate(async () => {
      const inp = document.getElementById("termIn");
      inp.value = "dump";
      inp.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true }));
      await new Promise((d) => setTimeout(d, 120));
      const text = document.getElementById("termLog").textContent;
      const m = text.match(/\{"scene".*\}/);
      let parsed = null;
      try { parsed = JSON.parse(m ? m[0] : ""); } catch (e) { parsed = null; }
      return { has: !!m, keys: parsed ? Object.keys(parsed).length : 0,
               named: !!(parsed && parsed.walkSize !== undefined && parsed.seed !== undefined) };
    });
    say(dumped.has && dumped.keys > 80,
        `dump writes every setting out as JSON (${dumped.keys} of them)`);
    say(dumped.named, "including the ones added last");

    // a notice the page gives is written down as well
    const noted = await ui.evaluate(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "1", code: "Digit1", altKey: true, bubbles: true }));
      return document.getElementById("termLog").textContent;
    });
    say(/Night/.test(noted), "and every notice the page gives lands in it");

    const shutTerm = await ui.evaluate(() => {
      const inp = document.getElementById("termIn");
      inp.value = "";
      inp.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true }));
      return document.getElementById("term").classList.contains("on");
    });
    say(!shutTerm, "and Esc puts it away again");

    // The menu's width is one value for the page, not one per template.
    const oneWidth = await ui.evaluate(() => {
      window.__state.uiWidth = 300;
      document.documentElement.style.setProperty("--menu-w", "300px");
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "1", code: "Digit1", altKey: true, bubbles: true }));
      return window.__state.uiWidth;
    });
    say(oneWidth === 300, `a template leaves the menu's width alone (${oneWidth})`);

    await ui.close();
  }

  // ---- settings older than the controls they never mention ---------------
  // A template or a saved default written before a control existed says
  // nothing about it, and what it means is the page as it shipped — not
  // whatever happens to be set at the moment. Everything from before walking
  // was flying, so recalling one has to hand flight back.
  if (mode === "check") {
    const old = await browser.newPage({ viewport: { width: 900, height: 1100 } });
    old.on("pageerror", (e) => { if (ours(String(e))) errors.push("legacy: " + e); });
    // its own saved default, and no cloud copy allowed to outrank it
    await old.route("**/firestore.googleapis.com/**", (r) => r.abort());
    await old.addInitScript(() => {
      localStorage.setItem("cortical.defaults.v2", JSON.stringify({
        at: Date.now(),
        data: { scene: 1, fly: 1, shadowDark: 0.11, flyPos: [0, 0, 3], flyYaw: 0, flyPitch: 0,
                // which picture each slot wanted, by name: one that is here
                // under a different number, and one that is not here at all
                matcap: 0, imgRef: { matcap: "Normals", normalMc: "Nowhere At All" } },
      }));
    });
    await old.goto(page_url);
    await old.waitForTimeout(1400);
    const back = await old.evaluate(() => {
      const s = window.__state;
      s.walkOn = 1; s.walkSize = 0.25; s.seed = 42; s.dofOn = 1;
      document.getElementById("resetBtn").click();
      return { walkOn: s.walkOn, fly: s.fly, size: s.walkSize, seed: s.seed,
               dof: s.dofOn, dark: s.shadowDark };
    });
    say(back.dark === 0.11 && !!back.fly, "a setting saved before walking existed still loads");
    say(!back.walkOn, "and comes back flying rather than walking");
    say(back.size === 1 && back.seed === 0 && !back.dof,
        "with everything it never mentioned as the page ships it");

    // A picture is remembered by name, because the number is a position in one
    // browser's own list and means something else in another browser's.
    const pics = await old.evaluate(() => ({
      matcap: window.__state.matcap,
      log: document.getElementById("termLog").textContent,
    }));
    say(pics.matcap === 1,
        `a picture is found again by name, not by number (slot now ${pics.matcap})`);
    say(/Not on this machine: Nowhere At All/.test(pics.log),
        "and one that is not on this machine is said, not silently swapped");
    await old.close();
  }

  if (errors.length) {
    console.log("\npage errors:");
    for (const e of errors.slice(0, 6)) console.log("  " + e);
    bad++;
  }
  await browser.close();
  console.log(bad ? `\n${bad} failed` : "\nall good");
  process.exit(bad ? 1 : 0);
};
run();
