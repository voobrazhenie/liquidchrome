# LiquidChrome

A single self-contained HTML page. Everything on screen is ray marched per
pixel in a WebGL fragment shader — no meshes, no scene graph, no libraries.

**Live:** https://voobrazhenie.github.io/liquidchrome/

## Objects

| | |
|---|---|
| **Brain** | A brain-shaped shell filled with a neuropil of neurites and somata, built from intersecting gyroid sheets with domain-repeated cell centres. |
| **Neuron** | One cell from 16 quadratic Bézier segments — soma, five unequal dendrites, a Y fork on each, a thicker axon — with a two-scale noise membrane. |
| **LiquidChrome** | A 20 × 20 × 0.5 slab displaced by Perlin noise into a landscape, intersected with an undeformed copy of itself, then twirled. Each of the five steps toggles on its own. |

## Controls

A **domain warp** (noise / twist / bend) deforms the coordinate space ahead of
the whole scene, so every object inside it distorts consistently. Resolution
and antialiasing (off / FXAA / SSAA ×4) are switchable, colours are per object.

Keys: `space` flight · `M` animate the field · `A` antialiasing · `F` full
screen · `H` panels · `U` everything · drag to steer · scroll or pinch to range.

Render scale adapts to whatever frame rate the GPU sustains. If the page runs
slowly, check that hardware acceleration is on — the page says so in the hint
line when it detects a software renderer.

## What is in here

| | |
|---|---|
| `index.html` | The whole page: markup, CSS, both fragment shaders and the renderer, in that order. |
| `REQUIREMENTS.md` | The written spec — what every control, key, mode and the benchmark do. |
| `pc/` | The same effect as a native Windows app (Rust + wgpu), with Vulkan / DX12 / OpenGL as a live switch. Built by GitHub Actions; `pc/README.md` says how it differs. |
| `tools/` | Headless Chromium checks: `node tools/shadows.mjs check` renders all three objects and compares against stored baselines. |

## Editing

Push to `main` and GitHub Pages redeploys. This repository is the only home for
the project — the page used to be mirrored as `neurons.html` in
`voobrazhenie/Alphabet`, and that copy has been removed.
