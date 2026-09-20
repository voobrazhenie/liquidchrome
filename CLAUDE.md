# Working notes

## How to answer

- **Short and plain.** Keep replies brief and non-technical unless a longer or
  deeper answer is asked for. Simplify: say what it means and what to do, not
  how it works underneath.
- Skip jargon, error codes, and internals unless they change what the user
  should do. If something is only interesting mechanically, leave it out.
- End every activity sequence with ✅✅✅.

## What is being worked on

**The desktop app (`pc/`) is the active surface.** The web page is not a priority:
leave `index.html` alone unless asked for it directly. Native-only work — MIDI, the
camera group, how a frame is sized — does not need a matching page change.

## What this project is

`index.html` is a single self-contained page: a ray-marched WebGL shader with
three objects (Brain, Neuron, LiquidChrome) and a control console. No build
step, no dependencies — everything is inline in the one file.

**This repository is the only home for it.** The page, the spec, the test
harness and the native build all live here, and there is nothing to mirror:

| Where | What | Branch |
| --- | --- | --- |
| `voobrazhenie/liquidchrome` | `index.html`, `pc/`, `tools/` | `main` (published at https://voobrazhenie.github.io/liquidchrome/) |

Until September 2026 a second copy lived in `voobrazhenie/Alphabet` as
`neurons.html`, kept in sync by hand. That copy has been removed; Alphabet is a
separate project now — a letter sequencer. **Do not push this work there.**

The page is also published as a Claude artifact at the same URL each time.

`REQUIREMENTS.md` in this repository is the written spec: what every control,
key, mode and the benchmark actually do. Read it before changing behaviour, and
update it in the same commit when behaviour changes.

## Known, not a bug

- **Artifact "403 / not watching"** — after each publish a notice says the
  artifact watch was not registered. It only means this chat will not be woken
  automatically if the page is edited somewhere else. The page itself is fine
  and nothing needs fixing. No need to mention it again unless asked.
- **No browser control** — this session runs in an isolated cloud container
  with its own headless browser. It cannot see or drive the user's real
  browser, so anything behind a login (Firebase console, dashboards) has to be
  done by the user, who then pastes the result here.
- **No GPU here** — the container renders in software, so frame rates measured
  in this session are meaningless. Correctness can be verified; speed cannot.
  Say so rather than quoting numbers. The native app *can* be run headless on
  software Vulkan to check the console renders — `pc/NOTES.md` has the recipe — but
  only in a debug build, and never for speed.
- **The chrome object has holes in it.** Its default *intersect* mode keeps only
  where the displaced slab and the plain one overlap, so straight down from the
  origin there is often no solid at all. A ground probe that reports nothing
  there is right; a test that needs ground under the camera has to union the two
  slabs (`lcOn[3] = 0`) or stand somewhere else.
- **No MIDI hardware here** — the mapping arithmetic is unit tested, the device
  layer cannot be. It is Windows-only code and the container has no controller.

## Verifying changes

Headless Chromium with SwiftShader, driven by Playwright:

```
NODE_PATH=/opt/node22/lib/node_modules node tools/shadows.mjs check
```

It draws all three objects, checks there are no page errors, and — the part that
matters — checks that the sun switched off is **byte-identical** to a stored
baseline. `... shadows.mjs base` writes that baseline, and it has to be written from
a build without the change being tested.

Three traps it already works around, worth knowing before writing another one of
these: a WebGL canvas is blank to `drawImage` once composited, so pixels have to
come from the screenshot; the frame-time controller keeps moving the render
scale, so the pose pins it to 0.30 — the controller's floor, and the only value it
will sit still at under software rendering; and Chromium hands back an opaque
canvas as **RGB, not RGBA**, so a decoder that assumes four bytes per pixel turns
the picture into noise that still looks like a picture. Every number it reports
was wrong for months because of that one. Read the colour type from the header.

A screenshot is also of the canvas at its CSS size, not its backing store, so
counting individual pixels means making the two match first — otherwise the
browser's resampling is what gets counted.

A fourth, for anything that has to be **dragged**: Playwright's `dragAndDrop`
does not synthesize HTML5 drag and drop in this headless build — not one
`dragstart` fires — so the console reorders from pointer events instead, which
`mouse.down/move/up` does drive (and which a touch screen has, and HTML5 drag
does not). `elementFromPoint` returns nothing outside the viewport, so a drag
only lands if both ends are actually on screen: fold the groups first, or give
that page a tall viewport. The console checks open a page of their own for
exactly that reason — the shader checks hide the interface and run at a size no
menu would fit in. Folding is not always enough, and it gets less enough every
time a group is added: the last module of an open group can sit past the bottom
of even a 1300-tall page. Scroll the thing being dragged into view first
(`scrollIntoView({block:"center"})`) rather than trusting that it fits.

A fifth, for anything that **slides, fades or eases**: this headless build
starts CSS transitions and then never advances them. The computed value sits at
the first frame for ever — `getAnimations()` reports nothing and no amount of
waiting helps — so a rule the cascade really is applying looks as though it is
not. Measure the resting state with the transition taken away
(`el.style.transition = "none"`), not after a wait.

Always check: all three objects still compile and render, and no page errors.

Two traps live in the shader rather than in the test:

- **Where a global goes.** The fragment shader is one file with each object's SDF
  inside its own `#ifdef`, and the neuron's sits high up — above `sceneLip`. A
  global or helper that every object needs has to be declared above *all three*
  `#ifdef` blocks, or two objects compile, the third says `undeclared identifier`,
  and the page shows a blank canvas for that one object only.
- **What `map` returns is read twice.** The march steps by it, and the volumetric
  glow multiplies it back up to recover the true distance. So a local feature that
  needs shorter steps must raise `lip` and leave the distance alone. Clamping the
  returned distance shortens the step too, but it also tells the glow the surface
  is near — which hangs a bright ghost of the clamping volume in empty air.

## One more thing about the shader

The page builds a **different program per object and per set of switched-on
features** (`FEATS` / `featMask` in `neurons.html`, `#ifdef F_*` in the shader).
Adding a feature means adding its define, gating its code, and adding it to
`FEATS` — otherwise it is compiled into every variant and the whole point is
lost. `window.__progs` reports how many variants have been built.

A new uniform also has to be added to `pc/tools/parity.mjs` by hand: that tool
sets every uniform itself, and one it does not know about stays at zero. A new
uniform whose "do nothing" value is 1.0 — `uHaze`, `uGlow` — silently breaks
every parity case until it is pinned there.

## The native build

`pc/` is a separate program: the same effect as a Windows app (Rust + wgpu), with
Vulkan / DirectX 12 / OpenGL as a live switch, the same keys and the same benchmark.
It is not part of the page and is not published anywhere — a GitHub Actions run
builds the .exe. `pc/README.md` says how it differs.

**Read `pc/NOTES.md` before changing anything in `pc/`.** It carries what is not
obvious from the code: how to check a change without a GPU, the decisions that look
arbitrary and are not (the y flip, the non-sRGB surface, the loop that must not be
unrollable), the pinned crate versions and their traps, and what has never been run
on real hardware yet.

The shader there is a port of the page's; `pc/tools/parity.mjs` renders both in
headless Chromium and compares them pixel for pixel. Keep it passing when either
shader changes.
