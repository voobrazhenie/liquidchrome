# Working notes for the native build

`README.md` says what the app is and how to use it. This file is the other half:
what a session picking this up needs to know before changing anything, and the
things that cost time to find out the first time.

---

## Where it stands

Everything here was written and checked in a Linux container **with no GPU**. The
shader is proved correct (see below) and the Windows build compiles, links and
passes its tests in CI — but nobody has watched the app draw a frame on real
hardware. The first person to run it is the user, on a Lenovo Legion with a
GeForce 4070 and Windows 11.

So: **treat the first report from that machine as the first real test.** Runtime
faults are more likely than rendering faults. The likely suspects, in order:

1. **Switching to OpenGL and back.** Windows lets a window's pixel format be set
   only once, so a window that has carried a WGL context may refuse the next one.
   This crashed on the user's machine going OpenGL → Vulkan. `App::build_gfx` now
   takes a **fresh window before it even asks for the device** whenever OpenGL is
   on either side of the switch, and `try_gfx` catches a panic on the way up so a
   bad driver costs the switch rather than the session. Fixed blind — worth
   re-testing on hardware, in both directions and twice each way.
2. **Exclusive full screen.** `Fullscreen::Exclusive` picks the monitor's largest,
   highest-refresh mode. Borderless is the default and the safe one.
3. **Uncapped presentation.** `PresentMode::Immediate` is asked for and quietly
   downgraded when a driver does not offer it; the console greys out what is not
   available.
4. **DPI.** The app never multiplies by a scale factor — winit's physical size is
   already device pixels — so a display at 125% or 150% should be right, but has
   not been seen.
5. **MIDI.** `midir` opens a WinMM input per port on a callback thread. None of it
   has met a controller: the mapping engine is tested, the device layer is not. If
   a controller is not found, or is found and does nothing, that is the first place
   to look. Hot-plug is a rescan about once a second, by port name.

It **has** now been watched drawing frames, but only under Xvfb on llvmpipe in this
container: the console, the camera group and the MIDI window all render and survive,
which is worth more than nothing and much less than a real GPU. Frame rates measured
that way are meaningless.

## Checking a change

Three checks, cheapest first. All of them run without a GPU.

```
cargo test                     # shaders, uniforms, benchmark, camera, sizing, MIDI, settings
NODE_PATH=/opt/node22/lib/node_modules node tools/parity.mjs
cargo check --target x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-gnu    # the only one that compiles midir
```

And a fourth, when the console itself has changed — the app will actually run here,
on Mesa's software Vulkan under a virtual X server:

```
apt-get install -y libxkbcommon-x11-0 mesa-vulkan-drivers x11-apps   # once
Xvfb :99 -screen 0 1500x950x24 &
DISPLAY=:99 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
  ./target/debug/cortical-flythrough --backend vulkan &
sleep 15 && DISPLAY=:99 xwd -root -silent > shot.xwd
```

A few frames per second, and the picture means nothing — but it is a real event
loop, a real device and a real egui pass, and it is what caught the `TexturesDelta`
panic that made every debug build die on frame one. Run it in **debug**: that panic
is a `debug_assert`, so a release build hides exactly the kind of fault this is for.

- **`cargo test`** is the one that matters most. `tests/shaders.rs` parses every
  object's WGSL with naga and then runs the **SPIR-V, HLSL and GLSL** backends over
  it — the same three translations wgpu performs for Vulkan, DirectX 12 and
  OpenGL — so a shader that would only fail on one backend fails here instead. It
  also checks the uniform block in the shader against `gfx::Uniforms` by size *and*
  field order; that pairing is the difference between a picture and garbage.
- **`tools/parity.mjs`** is the proof the port is faithful. It emits the ported
  shader as GLSL the way wgpu's GL backend would, then renders it and the original
  `#fs` shader out of `../index.html` in headless Chromium (SwiftShader) with
  identical uniforms and compares the two images. Six cases across the three
  objects, currently **exactly** identical. Run it whenever either shader changes.
  Playwright lives in the container's global modules, hence `NODE_PATH` and the
  `createRequire` at the top of the script — an ESM `import` cannot see `NODE_PATH`.
- **`cargo check --target x86_64-pc-windows-msvc`** type-checks the Windows tree,
  including the DirectX 12 backend, without needing a linker (`rustup target add`
  it first). For a real link, `apt-get install mingw-w64` and
  `cargo build --target x86_64-pc-windows-gnu` — that one links the whole thing.
- CI (`.github/workflows/pc-windows.yml`) is the authority: `windows-latest`, tests
  then release build, and the `.exe` uploaded as an artifact. Documentation-only
  changes under `pc/` skip it.

## Things that will silently break the picture

Each of these was a deliberate decision. Undo one by accident and the app still
runs — it just looks wrong, or dies on one backend only.

- **The y flip in `fsMain`.** WGSL's `@builtin(position)` counts down from the top;
  GLSL's `gl_FragCoord` counts up from the bottom. The entry point flips it so the
  ported shader means the same thing as the page's. wgpu's GL backend sets naga's
  `ADJUST_COORDINATE_SPACE`, so `gl_FragCoord` lands where `@builtin(position)`
  does and the same flip is right on all three backends. It is also why
  `parity.mjs` compares row `y` against row `H-1-y`.
- **The surface format must not be sRGB.** The shader applies its own gamma; an
  sRGB surface applies it a second time and everything washes out. `Gfx::new`
  picks the first non-sRGB format the surface offers.
- **The offscreen target uses the surface's format**, not a fixed one. That is what
  lets the same pipeline draw either into the target or straight to the screen —
  the `direct` path that skips the post pass when nothing needs resolving.
- **`parity.mjs` also checks the one thing the page cannot do**: with every colour
  set to black the ported shader must render black (max channel 0), while the
  page's still reaches 197 because its rim and back light are constants. If that
  case starts failing, a tint has been hard-coded again.
- **The material and post controls must stay no-ops at their defaults.** Every one
  of them (smoothness, metalness, exposure, glow, fog, vignette, grain) replaced a
  constant in the page's shader, and the default reproduces that constant exactly —
  smoothness 0.5 is `exp2(1 + 9.169925*0.5)` = 48 to the last bit. That is what
  lets `parity.mjs` keep comparing against the page. Change a default and the
  comparison stops meaning anything.
- **The upscaler divides the base, and the rung is base sized.** `state::frame_size`
  decides base first and marched second; the first post pass reconstructs to base and
  a second stretches base to the window — `two_pass` in `Gfx::render`. Size the rung
  to the window instead and FHD plus Upscale silently stops meaning 1920x1080 again,
  which is the bug this replaced. Both post passes share one pipeline and differ only
  in their uniforms, which is why there are two uniform buffers and two bind groups
  rather than one.
- **The selected state in the console is an outline, never a fill.** A white fill
  put white text on white; `chip()` in `ui.rs` sets fill and stroke explicitly
  rather than leaning on egui's selected-widget styling, which is what regressed.
- **The march loop must stay impossible to unroll.** Its trip count comes from
  `U.steps` through a `clamp`, so no compiler knows it. Put a constant bound back
  and DirectX's older FXC compiler may try to lay 300+ iterations out flat — which
  is exactly the failure the web page carries a "retry with a shorter march"
  workaround for.
- **egui's texture deltas must be applied on every frame, even a skipped one.**
  egui hands over each change once and never repeats it; drop one and the console
  loses its font atlas for the rest of the session. `Gfx::render` uploads before
  acquiring the swapchain image and frees after submitting.
- **A `TexturesDelta` has to be emptied, not just read.** `Gfx::render` uploads every
  `set` and frees every `free`, and then calls `clear()` — the struct asserts on drop
  that someone dealt with it, and a debug build turns that into a panic on the very
  first frame. Without the `clear()` the app cannot be run in debug at all, which is
  how it went unnoticed: CI only builds release.
- **A new device needs a new egui `Context`.** The renderer lives inside `Gfx`, so
  switching backend throws it away — and the old context would never re-send the
  atlas to the new one. `App::adopt` rebuilds both together.
- **The benchmark's determinism** is a contract, written down in
  `../REQUIREMENTS.md` §11: four fixed shots, 150 frames each, clocks stepped by
  exactly 1/60 s per frame from fixed seeds, march budget frozen, adaptive
  controllers off, console not drawn. Change any of it and native numbers stop
  comparing with browser numbers — and with older native ones. `tests/behaviour.rs`
  runs the same benchmark at 250 fps and at 36 fps and demands identical poses.

## How a frame is sized

Three sizes decide every frame, and confusing them is where the bugs live.

| | |
| --- | --- |
| **window** | `Gfx.config.width/height`, the swapchain. Always the real window, and always what the last post pass writes to. |
| **base** | what the **Resolution** control asks for: FHD is a literal 1920×1080; Half, Native and Auto are `window × scale_q`. |
| **marched** | what the shader actually runs at — `rw`/`rh`, the offscreen target's size. |

`state::frame_size` returns all three — it is a plain function of `(window, State,
warm, march_cap)` with no window handle in sight, which is what makes it testable
without a GPU. `App::frame_sizes` is a two-line wrapper that also stores `st.fit`.

- **Base.** FHD is a literal 1920x1080 and escapes the `max_px` clamps — chosen by
  hand it outranks them. Half, Native and Auto are `window x scale_q` and are
  clamped (software renderer, warm-up, Auto).
- **Marched.** `base x upscale_ratio x ss`. With no upscaler that is just `base x ss`.
- **Warm-up.** The first few frames after a start or a backend switch refuse both FHD
  and the upscaler and clamp hard, so nothing expensive is compiled into frame one.
- **`fit`** — the portrait compensation the orbit camera applies to its radius — comes
  from the **base** aspect, not the window. With FHD the picture is 16:9 however tall
  the window is, so asking the window would frame for a shape that is not being drawn.

`Gfx::render` is the chain, and it decides its stages from `PostMode`, `then_fxaa`
and whether base is already the window:

| chain | when | stages |
| --- | --- | --- |
| `direct` | Resolve, no FXAA, marched == window | march straight to the swapchain, no post pass at all |
| one pass | base == window and no FXAA after | march → target, then one post pass target → swapchain |
| two passes | a reconstruction that is not window sized, or FXAA on top of one | march → target, post → **rung**, then rung → swapchain |

The **rung** (`Gfx::mid`) is a **base**-sized texture: it is where the reconstruction
lands before it is fitted to the window, and where antialiasing finds something
already rebuilt to run on. `out_res` is the size being *written*; `texel` is 1 / the
size being *read*. Get those two the wrong way round and the image is subtly soft
rather than obviously broken.

So in full screen with **FHD + Performance**: march 960x540, reconstruct to
1920x1080, stretch that to the monitor — the same stretched 1080p look FHD gives on
its own, with the march costing a quarter of it.

FHD still asks the window to become 1920x1080 when it is not full screen
(`App::set_res`). That is deliberate and unrelated: it is how a windowed FHD gets one
render pixel per screen pixel. The upscaler no longer cares either way.

## MIDI

`src/midi.rs` is two halves with a hard line between them.

Above the line is arithmetic: `PARAMS` names every mappable control with the range
its console slider uses, `Mapping` is one wiring, and `Midi::poll` turns messages
into `Out` values. No I/O, so it is all unit tested here — which matters, because
this container has neither a controller nor the ALSA headers to build one against.

Below the line is `backend`, and it is **`cfg(windows)` on purpose**. `midir`'s Linux
path needs ALSA development headers the build container does not have, so an ungated
dependency would break `cargo test` for everyone working here. On anything but
Windows the backend is a stub that finds no ports; the half above cannot tell.
`cargo check --target x86_64-pc-windows-gnu` is what actually compiles the real one.

Three things are load bearing:

- **Mappings live outside `State`.** `reset_default`, `load_template` and
  `load_web_default` all replace `self.st` wholesale. A template is meant to change
  every setting; it is not meant to re-wire the controller, so `midi.json` is its own
  file and its own `App` field.
- **`PARAMS` and the `match` in `App::midi_set`/`midi_fire` are one list in two
  places.** An id in the table with no arm is a control that looks mappable and
  quietly does nothing. Two tests hold them together by reading `ui.rs` and `app.rs`
  as text — crude, and it catches exactly the drift that matters.
- **MIDI is dropped, not queued, during a benchmark run.** `Midi::poll` takes a
  `frozen` flag. Determinism is a contract (`../REQUIREMENTS.md` §11) and a knob moved
  half way through a run would make it measure two different pictures.

Smoothing is an exponential approach written against elapsed time, not per frame, so
a mapping settles identically at 20 fps and 200 — the same discipline the animation
clocks keep, and a test asserts it.

The console's controls became methods on `ui::Ctl`, which carries `&mut Midi`
alongside the `&mut Ui`. Each call passes `&mut self.st.<field>` separately, so the
two borrows are disjoint fields of `App`; anything that needs `&mut self` as a whole
(`set_res`, `fav`) has to be collected as an intent and run after the `Ctl` is
dropped, which is the same idiom the templates list already used. In map mode `Ctl`
draws a red target *instead of* the real widget rather than intercepting it, so a
half-finished drag cannot change a value on the way past.

## Versions, and how to survive a bump

Pinned in `Cargo.toml`: **wgpu 30.0, winit 0.30.13, egui / egui-wgpu / egui-winit
0.36**. These have to move together — `egui-wgpu 0.36` requires `wgpu ^30`, and
egui 0.36 needs rustc 1.95 or newer (the container shipped 1.94; `rustup update
stable` fixed it).

Both crates change their API freely between versions, and the changes are not
guessable. **Read the crate source instead of guessing** —
`/root/.cargo/registry/src/*/wgpu-30.0.1/src/api/` and the matching `egui-0.36.1`
— then compile early and let the errors finish the job. For the pinned versions,
the traps already paid for:

| | |
| --- | --- |
| `Instance::new` | takes the descriptor by value; `InstanceDescriptor::new_without_display_handle()` |
| `RequestAdapterOptions` | carries `apply_limit_buckets` |
| `SurfaceConfiguration` | carries `color_space` (`SurfaceColorSpace::Auto`) |
| `PipelineLayoutDescriptor` | `&[Option<&BindGroupLayout>]`, and `immediate_size` where push constants used to be |
| render pass / pipeline | both want `multiview_mask`; colour attachments want `depth_slice` |
| `get_current_texture()` | returns the `CurrentSurfaceTexture` **enum**, not a `Result` |
| presenting | `queue.present(frame)`, not `frame.present()` |
| error scopes | `push_error_scope` returns a guard; `guard.pop()` is the future |
| egui | `Context::run_ui` (not `run`), `egui_wants_keyboard_input`, `all_styles_mut`, `content_rect`, `Button::selectable` (there is no `SelectableLabel`), and `TexturesDelta.set` holds a `SmallVec` of deltas per id |

`rustfmt.toml` is tuned to how the code is already written, so `cargo fmt` is a
no-op rather than a forty-file diff. Keep it that way.

## Deliberately not done

- **No DirectX 11.** wgpu has no DX11 backend any more. DX12, Vulkan and OpenGL
  are the comparison.
- **DXC is not statically linked.** `wgpu`'s `static-dxc` feature would bundle
  Microsoft's newer shader compiler (it only affects the MSVC target), but it
  downloads a prebuilt binary at build time and the old compiler handles this
  shader — the page already proves that through ANGLE. If DirectX 12 ever refuses
  to build an object on real hardware, that feature is the first thing to try.
- **No network, no cloud sync.** The page's Firestore path is not ported and
  should not be. `settings.json`, `templates.json` and `midi.json` are their own
  format, live beside the executable rather than under `%APPDATA%`, and are *not*
  interchangeable with the page's saved settings. There is no fallback location on
  purpose: an app in a folder it cannot write says so.
- **No MIDI out, and no OSC.** Input only, and only what a control surface sends:
  continuous controllers and notes. Clock, aftertouch, pitch bend and sysex are
  parsed as "not for us" and dropped.
- **The GL backend passes no display handle** to `InstanceDescriptor`, which is
  fine on Windows and would need `new_with_display_handle` to work on Wayland.
- **No installer, no code signing.** SmartScreen warns once on an unsigned binary.
- **Not DLSS.** The Upscale control renders below the chosen resolution and rebuilds
  it with a Catmull-Rom kernel and a clamped sharpen. Real DLSS needs NVIDIA's NGX SDK and
  its DLLs, a Vulkan/D3D12 device built by hand with extensions wgpu will not let a
  program add, and depth plus motion vectors that a ray march does not produce —
  and it would only exist on one of the three backends, so the comparison would
  stop being like for like. `README.md` says the same thing to the user.

## Layout reminder

`presets/web-default.json` is the page's own saved settings, snapshotted from the
cloud document at build time and read by `store::from_web_json`. It is what a fresh
install opens on, and the reader takes all three shapes the page saves in (bare,
the browser's `{data, at}`, and the cloud document). Nothing fetches it at runtime.

`src/state.rs` mirrors the page's `state` object field for field — start there when
adding a control, then `src/ui.rs` for the console row. `src/app.rs` holds the frame
loop, the keys and the adaptive controllers; `src/gfx.rs` is the only file that
knows about wgpu. The three-line `//#ifdef` preprocessor in `src/shaderpp.rs` is
what keeps one WGSL file serving three objects, one pipeline each.

**The web page is not ours to touch.** `index.html` and `REQUIREMENTS.md` describe
the browser build; this folder follows them, and a behaviour change that belongs to
both is made on the page first.
