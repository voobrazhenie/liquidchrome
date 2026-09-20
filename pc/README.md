# Cortical Flythrough — PC build

The ray-marched page from `index.html`, running natively on Windows instead of in
a browser tab. Same three objects, same console, same keys, same benchmark — but on
the GPU directly, with **Vulkan, DirectX 12 and OpenGL as a live switch** so you can
measure which one your machine likes best.

Nothing here touches the network. Settings, named templates and the six saved
viewpoints are JSON files on your own disk.

The web page is untouched: this folder is a separate program that reads the same
shader, ported to WGSL.

---

## Getting it

**Download the built .exe.** Every push builds it: open the repository's **Actions**
tab, pick the newest **PC build (Windows)** run, and download the
`cortical-flythrough-windows-x64` artifact at the bottom of the page. Unzip, run.
Nothing to install — no Rust, no runtime, no DLLs.

Windows SmartScreen will warn about an unsigned binary the first time: *More info →
Run anyway*.

**Or build it yourself.** Install [Rust](https://rustup.rs) (which asks for the
Visual Studio build tools on Windows), then:

```
cd pc
cargo run --release
```

## Keys

Everything the page had, unchanged:

| | |
| --- | --- |
| `space` | Flight and morph together |
| `M` / `I` | Morph · impulses |
| `R` | New viewpoint |
| `1` `2` `3` `4` | Auto / Half / Native / FHD resolution |
| `A` | Cycle antialiasing |
| `F` or `F11` | Full screen |
| `H` or `U` | Hide the panels |
| `` shift+` `` | Fly mode |
| `5`–`0` | Recall a viewpoint |
| `shift`+`5`–`0` | Store a viewpoint |
| `W A S D Q E`, `shift` | Fly (only while flying) |
| `Esc` | Leave mapping, close the report, stop a run, or leave full screen |

and four that only a native build can have:

| | |
| --- | --- |
| `ctrl+1` `ctrl+2` `ctrl+3` | Vulkan · DirectX 12 · OpenGL, switched live |
| `V` | Present mode: vsync, mailbox, uncapped |
| `ctrl+S` | Save as default, mappings included |
| `ctrl+Q` | Quit |

Drag to steer, wheel to range in. In fly mode: right-drag to look, wheel for speed.
Keys are ignored while a text field has focus.

## Which graphics API is faster?

That is the question this build exists to answer, and the answer is your machine's,
not mine. The three are the same frame, the same shader and the same settings — only
the driver underneath changes:

1. Set the object and the resolution you care about.
2. Set **Present** to **Uncapped**. A frame rate sitting exactly on your refresh rate
   is measuring the display, not the render.
3. Run **Benchmark** on each backend (`ctrl+1`, `ctrl+2`, `ctrl+3`) and compare.

The benchmark is the same fixed run as the page: four shots, 150 frames each, the
clocks stepped by exactly 1/60 s per frame from fixed seeds, the march budget frozen,
and the adaptive controllers off — so frame *N* is the same picture everywhere and
the numbers compare across machines, backends and the browser. The report names the
backend, the adapter, the present mode and every setting that produced it. **Copy**
puts it on the clipboard; **Save to file** drops it next to your settings.

A run switches to uncapped presentation by itself where the driver allows it, and
puts your setting back afterwards.

## What is different from the browser

Everything below is why a native build is worth having; the picture is the same.

- **No compositor between you and the screen.** Vsync can be turned off outright, so
  the frame rate is what the GPU can do, not what the display will show.
- **No ANGLE.** The browser retranslates the shader to HLSL, with a march budget short
  enough to survive that; here the shader goes to the driver as SPIR-V, HLSL or GLSL
  and the ray-steps slider runs to 768 instead of 320. 320 is still the default so a
  native number stays comparable with a browser one.
- **The march loop cannot be unrolled.** Its length comes from a uniform, so the
  page's "retry with a shorter march" fallback has nothing left to survive.
- **One filter step instead of two.** With antialiasing off at Native resolution the
  march writes straight to the screen and the post pass is skipped entirely; the
  console's *post* readout names whichever chain ran — `direct`, `resolve`, `FXAA`,
  `upscale`, `upscale+fit`, or a reconstruction with antialiasing after it.
- **FHD resizes the window** to 1920 × 1080 for real, which a browser tab refuses.
  In full screen it cannot resize anything, so it marches a literal 1920 × 1080 and
  the post pass stretches that to the monitor.
- **The discrete GPU is asked for by name.** On a laptop with switchable graphics the
  binary exports the two symbols the NVIDIA and AMD drivers look for, so it runs on
  the 4070 rather than the integrated chip.

DirectX 12 uses Microsoft's newer DXC shader compiler when `dxcompiler.dll` is on the
PATH, and the one built into Windows otherwise. Both work; DXC is quicker to compile.

## Upscaling — and why it is not DLSS

**Upscale** in the post-processing group is the performance lever: the march runs at
a fraction of the size the image is shown at, and the missing pixels are rebuilt.

That fraction is measured against the **Resolution** you chose, not against the
window. So FHD + Performance marches 960 × 540, rebuilds it to a real 1920 × 1080,
and only then meets the monitor — the picture stays the size you pinned and only the
march gets cheaper. Half, Native and Auto work the same way: the upscaler divides
whichever base is live. The console prints the chain under the Resolution row.

| | | |
| --- | --- | --- |
| Quality | 67% of each axis | ~44% of the pixels |
| Balanced | 59% | ~35% |
| Performance | 50% | ~25% |

A per-pixel ray march costs its pixels almost exactly linearly, so Performance is
close to four times the frames. Reconstruction is a nine-tap Catmull-Rom kernel and
a sharpen clamped to the neighbourhood it came from, so it cannot ring or halo;
**Sharpen** sets how hard it pulls.

**Antialiasing still works with it.** FXAA runs as a second pass, on the
reconstructed image rather than instead of it, so the edges come out smooth at the
chosen resolution. SSAA still means what it says — march more pixels than the output — so
it multiplies the upscaler's render size and hands the reconstruction a cleaner
image, at the cost of the saving. The console's *post* readout names whichever
chain ran.

**This is not literally DLSS, and could not be here.** DLSS is NVIDIA's own
library: it needs the NGX SDK and its redistributable DLLs, a Vulkan or D3D12
device built by hand with extensions wgpu does not let a program add, and — to
beat a good spatial filter — a depth buffer and per-pixel motion vectors, neither
of which a ray march produces. It is also NVIDIA-only, which would leave the
DirectX/Vulkan/OpenGL comparison measuring three different pictures. What is here
is the same lever without the vendor lock: render fewer pixels, rebuild the rest,
on all three APIs. If you want true DLSS afterwards, say so — it is a project of
its own, not a switch.

## Templates and viewpoints

- **Save as default** stores everything the console can set, the fly camera, the six
  viewpoint slots and which groups are open. It is what the app opens with.
- **The web page's settings ship with the binary.** A fresh install opens on the
  object, the framing and the six saved views the browser was last left on, so the
  first frame you see is the picture you already know. **Web page settings + views**
  in the Templates group puts them back at any time. To bring over a newer set,
  export the page's settings to a file and start the app with
  `cortical-flythrough.exe --import saved.json` — it reads the page's own format,
  including a copy pasted straight out of the browser or the cloud document.
- **Templates** are the same snapshot under a name. Type a name, press **Save**, and
  it joins the list with **Load** and **×**. Each template carries its own six
  viewpoints, so a template restores a look and the shots that show it off together.
- **Reset** goes back to the built-in settings and the MIDI mappings, and leaves the
  templates alone.

The **Camera** group in the console holds the same six slots as buttons: click one to
go there, shift-click to store the view you are on. Range and steering — or speed,
yaw and pitch in fly mode — are there too, and **New view** is the `R` key. Every one
of them can be put on a controller.

Everything lives **next to the executable** — `settings.json`, `templates.json` and
`midi.json` in the folder the `.exe` is in. Copy that folder to a USB stick or
another machine and it opens on the same picture with the same controller wiring.
Plain JSON: back them up, edit them. Benchmark reports saved from the report window
land there too. Put the app somewhere it cannot write, such as `Program Files`, and
saving says so rather than quietly writing somewhere you will not find it.

## MIDI

Press **MIDI** at the bottom of the console. The window lists every controller that
is plugged in — several at once is fine, they are picked up on their own, and one
that is unplugged and plugged back in reattaches to what it was driving.

Mapping works the way Ableton Live's does:

1. Press **Map**. Every control that can be mapped turns red.
2. Click one. It says it is waiting.
3. Move a knob or press a pad. That is the mapping.
4. Press **Map** again, or `Esc`, when you are done.

Each mapping is a card in the window: which channel and CC or note it listens to,
which control it drives, and for a continuous one a **Min**, a **Max** and a
**Smoothing** amount. Min above Max is not a mistake — it turns the knob round the
other way. Smoothing is how long the value takes to catch up, up to half a second; it
is measured in time, not frames, so a controller feels the same at 30 fps and at 300.
Click a card and press **Delete** to drop it.

Notes work on continuous controls too: a pad slams a slider between its Min and its
Max, which with a little smoothing is worth having. Switches, segmented rows and the
buttons — the six views, **New view**, **Save as default** — take notes, and a note
does its thing on the way down only. A control that is not on screen cannot be
mapped, so switch to the object or the camera mode that shows it first.

Mappings are saved with **Save as default** (or `ctrl+S`), and live in `midi.json`
beside the app. They are deliberately not part of a template: loading a template
changes every setting, and it should not also re-wire the controller under your
hands.

## The surface and the passes after it

The page had one fixed highlight and four hard-coded post values. Both are controls
here, and every one of them defaults to exactly what the page did — the parity
check in `tools/parity.mjs` is what proves it.

- **Object modifications** gains **Smoothness** (the width of the highlight, on a
  log scale — 0.5 is the page's) and **Metalness** (a metal drops its diffuse and
  tints what it reflects with the body colour).
- **Post-processing** collects **Antialiasing**, **Upscale**, **Background**,
  **Exposure**, **Glow**, **Fog**, **Vignette** and **Grain** — most of which had
  no control at all before. Grain is the dither that keeps the gradients from
  banding; at 0 the banding comes back, which is worth seeing once.
- **Rim** and **Back light** sit next to the object's own two colours, and they
  are the answer to "why is my black object purple?". The page's shader carried
  both tints as constants: the fresnel rim in pale blue, and a violet back light
  that also colours the impulses and half the glow. Set all four to black and the
  object really is black — measured, not assumed.

Every slider takes a typed value as well as a dragged one, and typing goes past
the ends of the slider: the range is where the control is useful, not where it is
allowed.

## Layout

| | |
| --- | --- |
| `shaders/scene.wgsl` | The march, ported line by line from the page's `#fs` shader |
| `shaders/post.wgsl` | FXAA and the plain resolve |
| `src/gfx.rs` | wgpu: one instance per API, two passes, three lazily built pipelines |
| `src/state.rs` | Everything the console can set, and how a frame is sized |
| `src/camera.rs` | The three auto paths, free flight, the six slots |
| `src/bench.rs` | The deterministic run, per `REQUIREMENTS.md` §11 |
| `src/ui.rs` | The console, the graph, the report and the MIDI window |
| `src/midi.rs` | What can be mapped, and what a message does to it |
| `src/store.rs` | `settings.json`, `templates.json` and `midi.json`, beside the app |
| `src/shaderpp.rs` | The three-line `//#ifdef` preprocessor WGSL does not have |

## Checking it

```
cargo test                                     # no GPU needed
NODE_PATH=/opt/node22/lib/node_modules \
  node tools/parity.mjs                        # the port vs the page, pixel for pixel
```

`cargo test` validates every object's shader and runs naga's SPIR-V, HLSL and GLSL
backends over it — the same three translations wgpu performs for Vulkan, DirectX 12
and OpenGL — checks that the uniform block in the shader still matches the Rust
struct that fills it, and covers the benchmark's determinism, the camera and the
saved settings.

`tools/parity.mjs` is the real proof the port is faithful: it emits the ported shader
as GLSL the way wgpu's OpenGL backend would, renders it and the original page shader
side by side in headless Chromium with identical uniforms, and compares the two
images pixel for pixel across six cases. They currently match exactly.
