# Functional requirements

What the page does today, written down so it can be checked, changed on
purpose, and rebuilt somewhere else (a native app) without guessing.

`index.html` is one self-contained file: a ray-marched WebGL shader with three
objects and a control console. No build step, no dependencies. The same file is
published as `index.html` in `voobrazhenie/liquidchrome`; that repository carries
only the built page, and this document lives here.

---

## 1. The render

- One full-screen fragment shader marches a signed distance field, one ray per
  pixel. There is no mesh and no fallback image: without WebGL the page shows a
  short explanation instead.
- Two passes. The scene is marched into an offscreen target, then a post pass
  puts it on screen — FXAA, or a plain resolve that lets the bilinear filter
  down-sample a 2x target.
- One shader program per object **and per set of switched-on features**, built
  the first time that combination is asked for and then kept. See §26. Link
  status is checked: ANGLE translates GLSL to HLSL at link time, so a shader
  that compiles can still fail. A failure retries once with a shorter march;
  a feature set that will not build falls back to the plain one, and only an
  object whose plainest build fails is marked unavailable.

## 2. Objects

Exactly one is shown at a time.

| Object | What it is |
| --- | --- |
| **Brain** | A lattice of somata and neurites inside a brain-shaped shell. Cell density is adjustable. |
| **Neuron** | One cell: soma, five dendrites with a fork on each, and an axon, built from Bézier tubes. Line width is adjustable. |
| **Chrome** (LiquidChrome) | A 20x20x0.5 slab, displaced by Perlin into a landscape (two octaves of it, or as many as §35 asks for), intersected with an undeformed copy of itself, then twirled. Each of the five construction steps can be switched off on its own. A sixth switch, off by default, hangs a plain cube over the slab. |

Each object keeps its own two colours (structure and accent).

## 3. Camera

Two modes, switched in the console or with `shift+\``.

**Auto cam** flies one of three paths — Orbit, Fly-through, Drift. Dragging
steers relative to the path; the wheel or a pinch changes range.

**Fly** is free navigation, Blender-style:

- `W A S D` move, `Q` `E` down and up in world coordinates, `shift` sprints.
- `Tab` puts the camera on the ground and takes it off again. See §30.
- Right-drag looks; pitch stops just short of the poles.
- The wheel (or a pinch) sets speed, between the two limits in the **Camera**
  group. The floor reaches down to a thousandth of a unit a second on the
  slider, and lower still if a number is typed into the reading — a crawl is a
  legal speed.
- Taking over from auto cam starts exactly where the auto path left the camera.

`R` jumps to a new viewpoint. It always frames the object rather than empty
space, and stays close enough that the object overruns the frame.

**The Camera group.**

| Control | Range | Default |
| --- | --- | --- |
| **Slowest** | 0.001-1.000, or typed down to 0.0001 | 0.030 |
| **Fastest** | 0.5-60, or typed up to 400 | 12.0 |
| **Look smoothing** | 0-1 | 0.00 |
| **Move smoothing** | 0-1 | 0.00 |

The same group also carries walking (§30), and the lens and depth of field
(§31).

The two limits are what the wheel may wind the speed down and up to; `shift`
still sprints past the ceiling. Moving a limit past where the speed currently
sits carries the speed with it.

**Smoothing** makes the hand set a target the camera then walks toward, by the
same fraction of what is left every second however long a frame happens to be —
20 ms of lag at the bottom of the slider, 0.9 s at the top. Look and movement
have one each. **At nothing the target is the camera**: there is no easing step
at all, and flight is exactly what it always was. Being put somewhere — `R`, a
saved viewpoint, a template, taking over from auto cam — is a jump, never an
ease.

**The framing never moves because of the interface.** Folding a console group,
hiding the panels or resizing the console must not shift the view by a pixel.

## 4. Saved viewpoints

Six slots. `shift`+`5 6 7 8 9 0` stores, the bare digit recalls. A slot holds
the whole camera state — fly position and heading, path mode, path clock,
steering and zoom — and recalling one restores the navigation mode it was saved
in. Slots are part of "Save as default".

## 5. Console

**Thirteen folding groups, plus an ungrouped footer.** Every group is a filled
header bar exactly one row high — icon, name, and the fold arrow — with a box of
the same width hanging off it holding everything the group contains. A clear gap
sits between the last thing in one group and the next group's header.

**Inside a group the layout is two columns**, the way a property sheet is: a
narrow left column with the module's name, and everything that module controls
on the right. A **module** is one named thing — *Sphere*, *Softness*, *Bounds* —
and it is the unit the console is built out of. Module names are in the accent
colour, which is what makes the left column scannable.

| Group | What it holds |
| --- | --- |
| **Construction** | Object, Navigation, Flight path |
| **Camera** | how slow and how fast flight goes, and how much it lags the hand (§3); walking, jumping and step sounds (§30); the lens and depth of field (§31) |
| **Object modifications** | the chrome build steps and their sliders, **Blend to sphere**, **Seed** (§32), **Time** (§8), ray steps, surface precision, step relaxation, bounds and padding, cell density, domain warp, noise warp mode, warp strength and scale, line width, spike rate, colours, and Flight / Morph / Impulses |
| **Rendering** | Resolution, Antialias, Frame rate |
| **Shadows** | the Shadows switch, where the sun stands, and how its shadows fall. See §15 |
| **Material** | MatCap and its sphere, the normal map and its. See §16 and §20 |
| **Depth map** | an image thrown at one part of the object, and the gizmo that aims it. See §24 |
| **Mirror** | fold the object in half about a plane. See §27 |
| **Overlay** | a sprite standing over the picture. See §28 |
| **Transparency** | whether the eye goes through the surface, and what it finds. See §21 |
| **Post** | the air the object stands in, and what happens to the finished frame. See §17 |
| **UI** | how the console itself looks, and Arrange. See §19 and §22 |
| **Templates** | whole settings under a name. See §23 |
| Footer | Benchmark, Save as default / Reset, and two status lines |

The console (§33) is not one of the groups: it is a window of its own, on `` ` ``.

Controls that belong to one object only are shown only for that object. Which
groups are open is remembered, and **`M` folds them all away** — or brings them
all back if they are already away. The console is the one panel that scrolls,
with a styled vertical scrollbar and no horizontal one.

**None of that arrangement is fixed.** Groups and modules can be reordered,
moved between groups and renamed; see §22.

**`T` is the shortcut to the one group most often wanted on its own**: it brings
the menu back if the panels are away, folds every group, and opens **Templates**.

**How wide the menu is is one value for the whole page.** It is part of a saved
default, but it is *not* part of a template and a fader never touches it:
picking a different look must not resize the interface under your hand. The
console window (§33) reaches exactly to whatever that width leaves.

## 6. Rendering controls

| Control | Range | Default |
| --- | --- | --- |
| Resolution | Auto, Half, Native, FHD | Half |
| Antialiasing | Off, FXAA, SSAA x4 | FXAA |
| Ray steps | 48–320 | 320 desktop, 68 mobile |
| Surface precision | 1.0–12.0 | 12.0 |
| Step relaxation | 1.00–1.90 | 1.30 |
| Bounds | Sphere, Box, Auto | Auto |
| Bound padding | 0–0.60 | 0.05 |
| Warp octaves | 1–4 | 1 |
| Relief octaves (chrome) | 1–4 | 2 |

- **Auto** hands the render scale to a frame-time controller. **Half** and
  **Native** pin it. **FHD** renders exactly 1920x1080 whatever size the window
  is, and asks the browser to match the window to it — which an ordinary tab
  refuses, so the page says which of the two happened. Chosen by hand FHD
  outranks the software-renderer clamp; restored from saved settings it does
  not.
- **Step relaxation** is over-relaxed sphere tracing: each step reaches past the
  safe radius by this factor, and a step whose sphere fails to touch the
  previous one is walked back. Above 1 it converges in fewer steps; too far and
  it spends them backing out.
- **Bounds** is the volume the march runs inside. Auto picks a fitted box for
  the chrome slab and a sphere for the rest.
- Distances are divided by an upper bound on the warp's Lipschitz constant, so
  a warped field is still a valid lower bound on the true distance and the
  march cannot step through a surface.

## 7. Deformation

Three deformations, each switched on its own: **Noise**, **Twist** (whirl about
Y), **Bend** (about Z). Strength 0–1.00, scale 0.30–4.00 per unit.

The noise deformation has two modes:

- **Space** bends the coordinate space the object sits in. Everything shears
  together — the melted, torn look. Every march step pays for it.
- **Object** leaves the space straight and pushes the object's own surface in
  and out along its normal by the same noise field. Real geometry: the
  silhouette moves and the normals follow. The height is a fraction of one noise
  lobe rather than a fixed length, so the slope stays constant across the scale
  slider, and the step penalty is only paid within reach of a lobe — 1.4x to
  1.7x faster than Space.

Twist and bend are space warps in both modes.

## 8. Animation

- **Flight** (`space`) stops the camera and the field together.
- **Morph** (`shift+M`) animates the field.
- **Impulses** (`I`) are travelling lights, rate 0–11 Hz.

**Time** is the field's own clock, in seconds, shown as a number that runs.
**Play** (`P`) runs it and holds it still, **Rewind** puts it back to the start,
and a number typed into the reading puts the field at that moment. Play and
Morph are one switch shown twice — the transport beside the clock, and the
button in Animation — so both light up together. The impulse clock goes
wherever the field's clock is put, because the lights belong to the same moment
as the shape.

The clock is part of a saved setting and of a template, so a template comes back
to the moment it was saved at as well as to the shape (§23).

## 9. Saved settings

**Save as default** stores everything the console can set, the fly camera's
position and heading, the six viewpoint slots, which groups are open, and how
the console is arranged (§22).

**A saved setting is a complete description.** One written before a control
existed says nothing about that control, and what it means is **the page as it
ships** — not whatever happens to be set at the moment it is recalled. So
every template from before walking existed comes back **flying**, with the
walker at its standard size and the seed at zero, rather than leaving walking on
because walking happened to be on. Two things are exempt, because neither is
what the picture looks like: the console's own arrangement (§22) and the six
viewpoint slots. A setting that never mentioned those leaves them alone.

**Reset** goes back to whatever was last saved as the default — not to the
built-in settings — and it deletes nothing: the saved default stays saved and
every template (§23) is left exactly where it was. With nothing ever saved, the
built-in settings are what it goes back to, which is the same thing.

Two stores behind one seam. The browser's own copy lands first so there is no
flash of the wrong settings; a Firestore document
(`claudecode-3bb06`, `settings/default`) overrides it only if it is newer, and
the same defaults then follow the page to any computer. The whole cloud path
fails soft: no network, no permission, no cloud, and the page carries on with
the local copy. The web API key is public by design; the Firestore rules are
what protect the data.

**Templates sync the same way but in both directions** (§23). On load the page
reads the `templates` collection and settles three things at once: what the
cloud has and this browser does not comes down; what both have, where the
cloud's copy was touched later, comes down over the local one; and what this
browser has and the cloud does not goes up. Every template carries the moment
it was last touched, which is the only thing that can tell two copies apart.
The third case is what hands over a browser that has been keeping templates to
itself — the first time it is allowed to, they all go up.

**That needs the rules to allow `list`, not only `get`.** A single named
document like `settings/default` is a `get`; reading a whole collection is a
`list`, and a rule that grants one does not grant the other. Until the rules
say so, templates are written to the cloud and can never be read back.

## 10. Statistics panel

Current frame rate, a true five-second mean (frames divided by the time they
took, not the mean of per-frame rates), the render size, and a graph of the last
five seconds.

The graph maps its x axis to time rather than to sample count, so it scrolls at
a steady rate instead of stretching as the frame rate changes. Each column keeps
the worst frame that landed in it. The scale is labelled down the left and steps
between 60, 120, 240 and 480 off the 90th percentile of what is drawn, with
hysteresis so it does not flip back and forth. A yellow line marks the
five-second mean and carries its own reading.

## 11. Benchmark

Runs from the console. The point is that the same settings give the same result
on the same machine, and that two machines do identical work — so the numbers
can be compared across machines, browsers, resolutions and modes.

### What makes it deterministic

- The camera follows a **fixed path**. The user's view, the saved slots and the
  navigation mode have no influence, and are restored untouched afterwards.
- The three animation clocks step by a **fixed 1/60 s per frame**, never by
  elapsed time, from fixed seeds. Frame *N* therefore shows the same geometry on
  every machine at every frame rate.
- The **march budget is frozen** at whatever the settings ask for when the run
  starts, and the adaptive resolution and step controllers are switched off for
  the duration.
- The frame-rate graph and the statistics panel do not update during a run, so
  they cost nothing and cost the same nothing everywhere.

Only wall-clock frame time is measured.

### The run

| | |
| --- | --- |
| Warm-up | 1.5 s holding the opening pose, discarded — shader caches and GPU clocks ramping |
| Shots | 4 |
| Frames per shot | 150 (600 measured frames) |
| Clock step | 1/60 s per frame |
| Clock seeds | camera 12.0, morph 40.0, impulse 6.0 |
| Time limit | 45 s, after which the run reports PARTIAL |

Typical total is 5–15 s. A machine below roughly 14 fps will not finish; that is
reported as PARTIAL and is explicitly not comparable with a complete run.

Every position is in units of the object's own radius (Brain 1.55, Neuron 1.15,
Chrome 1.10), and every shot passes above the object looking down at the middle.
`u` runs 0 to 1 across a shot.

| Shot | Path |
| --- | --- |
| Pass | line, `(-1.65, 0.55, 0.10)` to `(1.65, 0.55, -0.10)`, target `(0, 0, 0)` |
| Diagonal | line, `(-1.30, 0.95, -1.30)` to `(1.30, 0.35, 1.30)`, target `(0, 0.05, 0)` |
| Orbit | arc at radius 1.05, height 0.45, angle 0.60 to 2.70 rad, target `(0, 0, 0)` |
| Graze | line, `(0.15, 0.30, -1.75)` to `(-0.10, 0.22, 1.15)`, target `(0, 0.05, 0)` |

Roll is 0 throughout.

### The report

Average, median, 1% low (99th percentile frame time) and best 1% frame rates;
the frame count and elapsed time; the average for each shot; and the full
settings that produced them — object, resolution and actual render size, AA,
march budget, precision, relaxation, bounds, warp, the object's own parameters,
the GL renderer string, window size and device pixel ratio, and the user agent.
The whole report can be copied as text.

If the frame time sits steadily on a common refresh rate the report says so: the
display is the limit, not the render, and the resolution needs raising before
the number means anything.

### Reimplementing it elsewhere

A native build reproduces this run by following the table above: the same four
shots, 150 frames each, the same clock seeds and the same 1/60 s step, the march
budget held fixed, and no adaptive quality. Numbers are then comparable with the
browser's, given the same object and settings.

## 12. Keyboard

| Key | |
| --- | --- |
| `space` | Flight and morph together |
| `L` | Hand the left drag between the sun and the camera |
| `G` | Show the depth decal's gizmo, or put it away |
| `*` | Stand where the sun does, and come back |
| `M` | Fold every group away, or open them all |
| `T` | The menu, with nothing open but Templates |
| `` ` `` | The console, down from the top and away again |
| `shift+M` | Morph |
| `P` | Play and pause the field's clock |
| `I` | Impulses |
| `R` | New viewpoint |
| `1` `2` `3` `4` | Auto / Half / Native / FHD resolution |
| `A` | Cycle antialiasing |
| `F` | Full screen |
| `H` or `U` | Hide the panels |
| `` shift+` `` | Fly mode |
| `Tab` | Walking, on and off (only while flying) |
| `space` | Jump (only while walking) |
| `5`–`0` | Recall a viewpoint |
| `shift`+`5`–`0` | Store a viewpoint |
| `alt`+`1`–`0` | Recall a template |
| `W A S D Q E`, `shift` | Fly (only while flying); walking drops `Q` `E` |
| `Esc` | Close or stop the benchmark |

**Every key is read by where it sits on the keyboard**, not by the letter its
layout prints. On a Russian layout `W` prints ц and `P` prints з; the page takes
both exactly as it takes `W` and `P`, in any language the system is set to.
`shift+8` and the numpad's `*` are both the sun's own view, for the same reason.

Keys are ignored while a text input has focus, and while a name is being typed
into. `W A S D Q E` belong to fly mode while it is on, so `A` does not also cycle
antialiasing there.

**`alt` is its own alphabet.** A key with `alt` held reaches only the template
list, and nothing else on the page reads a key while `alt` is down — so `alt+1`
does one thing and `1` does another.

## 13. Constraints and behaviour under stress

- Opening frames are capped hard: nothing has been measured yet, and a
  full-size first frame on a machine without acceleration looks exactly like a
  page that renders only its panels.
- A software renderer (SwiftShader, llvmpipe, Basic Render Driver) is detected
  and told to the user, with the march budget clamped.
- On a phone the march budget is lower and left to the frame-time controller.
- Everything degrades rather than fails: no WebGL, no network, no clipboard, no
  full-screen permission, no local storage — each is handled and said plainly.
- `prefers-reduced-motion` starts the page with the camera and field still.

## 14. Deliberately not done

- The page has no mesh export, no recording, and no MIDI or OSC input.
- No depth-aware upscaling; the resolution control is a straight render scale.
- The post pass still runs when antialiasing is off (a plain resolve).
- A pinned resolution can still be given up if the frame time becomes
  untenable (over 250 ms), and does not climb back on its own while pinned.

## 15. The sun and its shadows

On by default, and off is exact: with **Shadows** unset the key light is the
one that follows the view, no shadow ray is cast, and the frame is the one the
page drew before the sun existed, to the byte.

Switched on, a **sun** takes over as the key light. It stands still in the
world while the camera moves, which is the whole point — a light fixed to the
view throws its shadows behind the things that cast them, where nobody can see
them.

| Control | Range | Default | Applies to |
| --- | --- | --- | --- |
| **Shadows** | off / on | on | all |
| **Lit / unlit** | off / on | off | all — draws the shadow term itself, white where the sun reaches and black where it does not, with no material at all |
| **How** | March / Trace / Sphere / Sun | March | — |
| **Sun across** | 0-360 degrees | 52 | all — the sun is the key light in every mode |
| **Sun up** | -89 to 89 degrees | 34 | all |
| **Softness** | 0-1 | 0.25 | ray march |
| **Reach** | 0.5-6 units | 3.0 | march, trace |
| **Shadow steps** | 8-128 | 96 | march, trace |
| **Shadow sphere** | any loaded sphere | Chrome | sphere |
| **Level** | 0-1 | 0.50 | all |
| **Contrast** | 0-1 | 0.00 | all |
| **Amount** | 0-1 | 0.85 | all |

### Three ways to arrive at one number

Whichever is chosen, the answer is a single value — 1 is full light, 0 is
black — and all three go through the same shaping and the same application, so
the controls mean the same thing in each and switching between them is a fair
comparison.

- **March** — a second sphere-traced march from the lit surface toward the sun,
  with a soft edge out of how close it passed. Throws a shadow from one thing
  onto another.
- **Trace** — the same line, walked in even steps, asking at each one whether it
  is inside anything. Nothing is sphere traced, so the warp's Lipschitz bound
  cannot slow it to a crawl and there is no budget to run out of: the cost is
  exactly **Shadow steps**, whatever the scene is doing. No near-miss either, so
  no penumbra and no grey — occluded or not. **Softness** does not apply.
- **Sphere** — the brightness of a painted sphere, read the same way a matcap
  is (§16), in linear light. Whatever is painted on it lands on the object, for
  one texture fetch. It cannot know about anything else in the scene.
- **Sun angle** — how square-on the surface is to the sun, opened out across the
  whole range. No ray and no texture: a dot product. It cannot throw a shadow
  onto anything else either, but it turns with the sun and with **Contrast** up
  it draws a hard terminator wherever it is aimed.

For **Sphere** and **Sun angle** the console draws the result on a small sphere
beside the controls, after Level, Contrast and Amount — so what the sliders are
doing can be read straight off rather than hunted for on whichever part of the
object happens to point the right way.

Whichever source is chosen, the shadow also sits over a **MatCap** material
(§16) when both are on: the sphere is a picture, so it is multiplied in display
terms.

**Level** slides the edge between light and shadow; **Contrast** squeezes the
ramp around it until it is a step. That is where a soft grey shadow becomes a
hard black one with a boundary you can see. At **Contrast 0 the shaping is the
identity**, so the ray march is exactly what it was unless the slider is reached
for.

The ray march itself: a second march from the lit surface toward the sun, at most 64
steps and never further than **Reach**. It steps by the distance the field
guarantees is clear and judges a near miss by the distance the surface probably
is — the two are not the same number wherever a warp or the relief is on, and
stepping by the second one lands the ray inside the surface it set off from,
which reads as a hit and blackens faces the sun is plainly shining at. Being
inside is the sign of that second number and not a small value of it: calling a
hit at a hundredth of a unit is a cliff, and on a ridge lit edge-on by a low sun
neighbouring pixels fall either side of it and come back opposite. The ray also
starts clear of the surface by two things added together — more the lower the
sun sits on that face, since a ray leaving at a shallow angle stays inside the
surface's own roughness a long way out, and more again by however far the main
march may have overshot in landing, since a shadow ray born underground is black
before it starts. The closest that ray passes to anything, against how far it had
travelled, is how much of the sun it hides —
so the penumbra comes out of the same march rather than out of more rays.
**Softness** sets how wide the sun reads: 0 is a point and a hard edge, 1 is a
broad source. A ray that actually touches the surface is black whatever the
softness says, so a sharp shadow is properly black and not merely dark.

**Darkness** is how much a shadow is allowed to take. It scales the key light
and the highlight, and by the same amount the fill, the rim and the back
light — so at 1 a fully shadowed pixel is black, and at 0 nothing changes at
all. The glow is gathered along the view ray rather than at the surface, and is
left alone.

A surface facing away from the sun has no sun on it, which is what being in
shadow means — so it is in shadow, at once and without a ray. That is what makes
an object shadow itself rather than merely shade itself.

A shadow ray's step is the true distance divided by the warp's Lipschitz bound,
and that bound can be twenty-something. A ray that only ever takes those steps
never arrives: it spends its whole budget in the first tenth of a unit and gives
up — and giving up reads as **lit**, which is light leaking through the middle of
a shadow. Both ways of failing leak light, so the step also grows with how far
the ray has come — by however much it takes for the budget it has been given to
cross the whole **Reach** whatever the warp is doing, worked out from those two
sliders once per frame rather than per pixel. Measured on a warped scene, rays
that ran out of budget went from 45% to none, and stay at none at 16, 32, 96 and
128 steps.

## The sun's own view

`*` stands the camera where the sun is and looks back along it, and switches the
projection to **parallel** at the same time — the sun is a direction and not a
place, so a perspective eye put at one would show a view no shadow in this scene
was ever worked out from. From there nothing can be seen in shadow, which is the
point of it: it is what the sun sees. Dragging moves the sun, the wheel widens
the view, and `*` or `Esc` gives the camera back exactly as it was, because
nothing about it was touched.

Dragging the sun follows the mouse: down takes it down. The **right** drag turns the camera in every mode, so aiming the light never
costs the view. The **left** drag belongs to the sun, and does from the moment
the page opens: swinging it moves the sun and the camera does not budge, and
the sun is drawn in the sky where it stands so it can be aimed by eye. `L` or
`Esc` hands the left drag back to the camera, `L` again takes it. Turning the
mode on turns **Shadows** on, since it would otherwise do nothing that can be
seen, and the drawn sun goes away with either of them. The mode itself is not
saved.

A finger has no second button, so on a touch screen every drag is the camera's
and the sun is aimed with **Sun across** and **Sun up**.

A run with shadows on reports them, because a second ray per lit pixel is not
the same work as one.

**The test cube** is build step 6 on the Chrome object, off by default. It is a
plain box hanging clear of the slab, and it exists to be checked by eye: put the
relief away, leave the slab flat, and its shadow is one anybody can predict. It
is taken in the untwirled coordinates so the twirl does not drag it, it is
carried by the same distance bound as the slab, and the bounding volume grows to
reach it. Nothing about it is clever, which is the point.

A sun low on the horizon is not the same thing as a shadow. On a relief steep
enough, most of the surface faces away from a sun a few degrees up, and looking
toward that sun shows almost nothing but the sides that do — dark because they
are turned away, with no ray cast at all. **Sun up** is the control that
separates the two.

## 16. MatCap

A matcap is a photograph of a sphere. The surface is coloured by looking that
photograph up by which way it points **on screen** — so whatever lit the sphere
lights the object, key light, reflections, horizon and all, out of one texture
fetch and no lights at all.

| Control | |
| --- | --- |
| **MatCap** | off / on. Off by default, and off is exact: not a texel is read and the frame is the lit one, to the byte. |
| **Sphere** | Which one. Two are built in — **Chrome**, a polished ball on a dark stand, and **Normals**, which is not a photograph but the surface direction written straight into the colour. |
| **Add…** | Load an image from this computer. It joins the list and is used at once, brought down to 512 across — or 1600 for a sprite sheet, which needs the room. An image that is **see-through** keeps its transparency: it is made smaller until it fits rather than re-encoded as a JPEG, which has no alpha channel and would put a black box around a cut-out. |
| **Keep** | Hold on to the chosen sphere — in this browser, and in the cloud so it follows the page to another computer. The two built-in ones are already there. |

An image loaded with nothing see-through anywhere in it is **marked as such in
the list** — hover it and the name says so. A sheet exported flat looks exactly
like one this page has flattened, and only one of those is anybody's to fix.
| **Delete** | Remove the chosen sphere from both. The built-in two stay. |

The same list feeds the **Shadow sphere** picker in §15, the **Map** picker in
§20 and the depth decal's own in §24, so an image loaded once can be used as a
material, as a shadow, as a normal map, as a decal, or as all of them. **Add** in §20 is the same loader; it differs only
in which of the three the new image lands in, and each of the two groups keeps
and deletes what its own chooser is pointing at.

Switched on it **replaces the material outright**: no key light, no sun, no
shadow ray, no rim, no specular. Fog and the volumetric glow still apply — they
are the air between the camera and the object, not the object's surface. A
benchmark run says which sphere was used, and does not report the sun, because
the sun took no part in it.

The lookup is the surface normal in the camera's own axes, with the camera roll
taken back out so the sphere stays locked to the screen rather than leaning with
the horizon. The texture is already in display colours and the frame is tone
mapped and gamma corrected after shading, so the sample is fed through the tone
map's own inverse first and the two cancel: the pixel that comes out is the
pixel that went in.

The two built-in spheres are generated by `tools/matcaps.mjs` rather than pasted
in as blobs — that file is the record of what is in each one.

A loaded sphere lasts for the session unless it is **Kept**, which writes it to
this browser and to one small cloud document of its own — one document per
sphere, because an image will not fit alongside the settings. On opening, the
browser's copies land first and the cloud's are merged in by id, so a sphere
kept on another machine arrives without doubling up one already here. The saved
defaults carry which sphere was chosen, never the image.

## 17. Post

The air the object stands in, and what happens to the frame after it is drawn.

| Control | Range | Default |
| --- | --- | --- |
| **Haze** | 0-3 | 1.00 |
| **Glow** | 0-4 | 1.00 |
| **Noise** | 0-2 | 0.00 |

**Haze** is the distance fade: whatever is further from the eye is blended
toward the background, which is what makes depth read as depth rather than as a
flat cut-out. Each object was given the thickness that suited it — the brain's
air is more than twice the chrome's — and the slider **multiplies** that rather
than replacing it, so every object keeps its own character across the whole
range. **1.00 is the page exactly as it was**, to the byte, and at **nothing**
the air is perfectly clear and the far side of the object comes back.

It is not the same thing as the **Glow**, which is added around the surfaces
rather than laid over the distance.

**Glow** is the light that gathers near a surface. It is summed as the ray
marches, **one reading per step**, so anything that makes the march take smaller
steps — a domain warp, or a depth map's beam — gathers more of it over the same
distance and fills that volume with a soft veil. That is an accident of how it
is measured rather than a property of the scene, and it is also the prettiest
thing on the page, so it has a control of its own rather than a correction.
**1.00 is the page exactly as it was**, to the byte; at **nothing** the veil is
gone and the geometry is bare. It is also multiplied by the **Spike rate**,
which is why impulses brighten it.

**Noise** is film grain, laid on last, per pixel and per frame. It used to be on
at a fixed strength and there was no way to turn it off; it is now off unless it
is asked for. The native port has no control for it, so `pc/tools/parity.mjs`
compares the two shaders with the page's grain switched back on.

## 18. Typing a value

Every slider's reading can be clicked and typed into. The slider is a
convenience and not the range: what is typed is taken as it is, even when the
thumb has to sit at one end to show it, so a value past either end of a slider
is reached by typing it. `Enter` commits, `Esc` puts the old reading back.

**And it survives being put away and fetched back.** A value typed past the end
of its slider is saved and restored as it was typed, not as the slider's own
end — by the saved default, by a template, and by the cross-fade. Showing such a
value on its slider clamps it, and the slider's own handler would otherwise
write that clamp back into the settings, which is what used to turn a Glow of 10
into a Glow of 4 the moment its template was recalled.

**And a way back to the default.** Beside the reading, while the two differ,
sits what the value would be in the saved default (§9), written the same way —
so the same small number is both the way back and the mark that says this one
has been touched. Clicking it restores that value, and so does double-clicking
the slider.

## 19. The console itself

It is fixed to the top right and flush with three edges of the window — no
margin at the top, the right or the bottom — and it is narrow: about half what
it used to be. **Its left edge is a handle**: drag it to set the width, between
120 and 680 pixels. The rest of the interface keeps clear of whatever that
width is.

A group's header is a filled bar with an icon and dark text on it, exactly one
row high, and what it holds is a box of the same width hanging off it — so a
group is one object on the page rather than a heading with loose rows under it.
There is a gap before the next header, more air on the right than on the left,
and button borders are brighter than panel borders because a button is the thing
being pointed at.

| Control | Range | Default |
| --- | --- | --- |
| **Font size** | 8-26 px | 12 |
| **Name column** | 18-62 % | 36 |
| **Opacity** | 0-1 | 0.94 |
| **Back** / **Accent** | any colour | #04070c / #2fd9c0 |
| **Headers** | any colour | #2fd9c0 |
| **Arrange** | off / on | off |

**Headers** is its own colour, separate from the accent: it fills every group
header and tints the box each group is drawn in. Its text flips between dark and
light by the colour's own brightness, so a header is legible whatever it is set
to. **Name column** is how much of the width the left column of names takes; the
controls get the rest.

**There are no hints.** The line at the foot appears only to say that something
happened — a view stored, a sphere kept — and goes away again.

**Buttons fill a row and wrap only when they cannot.** Each carries its own
border with a gap around it, so two side by side always read as two; a shared
edge stops doing that the moment a row wraps. A label that will not fit is
trimmed on one line rather than broken mid-word or stacked a letter at a time —
widen the menu and the whole word comes back.

**Font size** drives the statistics panel as well as the console.

| Control | Range | Default |
| --- | --- | --- |
| **Font size** | 8-26 px | 12 |
| **Opacity** | 0-1 | 0.94 |
| **Background** | any colour | #04070c |
| **Colour** | any colour | #2fd9c0 |

Each writes one CSS custom property and nothing else knows about it: the font
size is the console's own `font-size` and every size inside it is in `em`; the
colour is the accent the whole console is drawn from; background and opacity
together are the panel fill.

**Uncapped** (in Rendering) hands the draw loop to a timer instead of the
display's own beat, so the frame rate stops being the refresh rate and starts
being how fast the render actually is. The picture tears — that is the trade —
and a benchmark run says which one it was. Only one loop ever runs: switching
clocks invalidates whatever was in flight on the old one rather than starting a
second chain beside it.

## 20. Normal map

An image laid over the surface as a normal map: it moves the normal itself and
nothing else, so the matcap lookup, the sun, the shadow and the rim all pick the
detail up together and agree about it.

| Control | Range | Default |
| --- | --- | --- |
| **Normal map** | off / on | off |
| **Map** | which image, from the one shared list of §16 | Chrome |
| **Add / Keep / Delete** | as §16, but on the chosen **map** rather than the chosen sphere | |
| **Amount** | 0-1 | 1.00 |
| **Scale** | 0.10-16 | 2.00 |

Off is exact: not a texel is read. **Amount** at nothing is exact too — the
normal is handed straight back rather than being put through a `normalize` that
could move its last digit — so the slider covers the whole range with no step
at either end of it.

An SDF has no seams to unwrap, so the map is projected **down all three axes**
and the three readings blended by how square-on the surface is to that axis, to
the fourth power, so a face that clearly belongs to one plane is not muddied by
the other two. The blend is the whiteout one: the map's sideways push is added
to the surface's own normal before the three are summed, which keeps detail
through the blend instead of averaging it flat where the planes meet.

Tiling is **mirrored** rather than wrapped, so any image at all repeats without
a seam whatever its size — a photograph that was never made to tile included.

**Add** loads into the same list §16 keeps, and this group has its own **Keep**
and **Delete** so an image loaded as a normal map can be held on to without
having to be the material as well. Removing an image slides every picker that
pointed past it back with it, so none of the three is ever left pointing at the
wrong thing.

## 21. Transparency

The eye does not have to stop at the surface. It bends into the body, takes the
body's colour on over the distance the crossing costs it, and ends on whatever
lies beyond — the far wall, the next object, or the sky.

| Control | Range | Default |
| --- | --- | --- |
| **Transparent** | off / on | off |
| **Opacity** | 0-1 | 0.15 |
| **Edge** | 0-1 | 0.60 |
| **Refraction** | 1.00-2.50 | 1.45 |
| **Tint** | any colour | #BFE9FF |
| **Density** | 0-4 | 0.90 |
| **Depth** | 1-4 | 2 |

Off is exact: not one extra ray is cast and the frame is the one it always was.
**Opacity at 1** is exact as well — the surface keeps all of itself and nothing
goes on through — so the slider covers the whole range with a real solid at the
top of it.

**Opacity** is how much of the surface still shows *square on*. **Edge** is how
much of it comes back at a grazing angle, by Fresnel: that second term is the
part that reads as glass rather than as a faded object, and turning it off gives
a flat film instead.

**Refraction** bends the view on the way in and straightens it on the way out —
1.00 straight through, 1.33 water, 1.52 glass, 2.42 diamond. Where the angle is
too shallow to leave by, the ray reflects off the inside of the wall instead,
which is what makes a thick edge go bright.

**Tint** and **Density** are Beer's law, written as the tint raised to the
distance crossed: the colour is what survives one unit of body, so white is
water, and anything else deepens the further the crossing had to go. Density at
0 is perfectly clear whatever the tint is.

**Depth** is how many surfaces the eye may see through. 1 is the front face and
the sky behind it; 2 adds the far wall; 3 and 4 reach whatever stands beyond the
object. Each one costs another march, and that is where the whole cost of this
is — off, there is none.

Inside the body the field is negative and its size is still the distance to the
nearest wall, so the same safe step works with the sign turned round: the
crossing is sphere traced from the inside and stops the moment the field comes
back up to nothing. Rays after the first are traced plainly — no over-relaxation
and no volumetric glow, both of which the pixel's first ray has already paid
for — and they share out half the ray-step budget rather than each taking it
whole.

Shadows are unaffected: a shadow ray still treats everything as solid, so a
see-through object throws the shadow of a solid one. The lit-versus-unlit view
(§15) ignores transparency too, because what it is drawing is the shadow term
and not the object.

## 22. Arranging the console

**Arrange** (in the UI group) takes hold of the console. While it is on, every
module shows a grip and every group header and module name grows a pencil.

- **A grip carries its module.** Drop it anywhere in any group — including onto
  a folded group's header, which puts it at the end of that group.
- **A header carries the whole group.** Groups reorder among themselves and
  nothing else: a group is always beside another group, never inside one. There
  are no sub-levels and none can be made.
- **The pencil renames** the group or the module it sits on. `Enter` or `Esc`
  finishes; the page's own keys stand down while a name is being typed.

Moving is done with the pointer rather than with the browser's drag and drop: a
grip is a small target, the console scrolls (and scrolls itself when a drag
reaches either end of it), and a touch screen has no drag gesture at all. A
header that has just been dragged does not also fold.

**Nothing is saved by rearranging.** The moment anything moves or is renamed, an
amber strip appears at the top of the console — stuck there, so it cannot be
scrolled away from — saying the layout has changed and naming the two things
that would keep it: **Save as default**, or **Save as** a template. It goes away
the moment either happens.

The arrangement is three small things — the order of the groups, which modules
each one holds, and anything renamed — and none of it is about what the page
draws, so it rides in the saved settings like any other preference and travels
inside a template like one too. A module that a saved layout has never heard of,
because it was added afterwards, goes back to the group it was built in rather
than disappearing.

## 23. Templates

A template is one whole snapshot of the settings under a name of your choosing.
Ten of them.

| Control | |
| --- | --- |
| **Stored** | the templates there are. Clicking one applies it; **shift**-clicking only points at it |
| **Fade to next** | 0-1, cross-fade from here to the next one along |
| **Save as** | name this moment and keep it |
| **Update** | write over the one showing as chosen, keeping its name and number |
| **Earlier / Later** | move the chosen one a place along the list |
| **Rename** | give the one showing as chosen a different name; nothing it holds changes |
| **Delete** | remove the one showing as chosen |

`alt`+`1`–`0` recalls the template in that slot, counting the list from the top;
`alt+0` is the tenth. Applying one brings its arrangement (§22) with it.

The number a template answers to is simply **its place in the list**, so
**Earlier** and **Later** move its `alt` shortcut with it. The order is kept in
this browser and in the cloud, so the list is the same on another machine.

**Update**, **Rename**, **Earlier**, **Later** and **Delete** all act on the
template showing as chosen.
A plain click on one both chooses it and loads it; **shift** and a click only
chooses it — so the one to be renamed or deleted need not be the one you are
sitting on, and nothing you have set up is lost to get at it.

A template holds **when** as well as **what**. The three clocks — the flight
path's, the field's and the impulses' — are saved with everything else, and this
field has no random number in it that is not a hash of a position, so where the
clocks stand *is* the state of the animation. Recalling a template therefore
lands on the same frame, not merely on the same settings. The auto cam's
steering goes with them for the same reason. **Save as default** keeps them too,
so **Reset** rewinds to the moment the default was saved.

**Fade to next** crosses from wherever you are now to the next template in the
list, wrapping round at the end. Numbers and colours cross over; anything that
is a choice rather than an amount — which bound, which build steps, whether a
thing is on — flips at the half-way mark, because there is no half of a choice.
Let go anywhere in between and you stay there; take it back to nothing and you
are exactly where you started. Take it all the way and that template becomes the
one you are on, the fader returns to nothing, and the next one along is the one
after it — so a chain of templates can be walked through one fade at a time.
Headings cross **the short way round**. An angle has no size, only a direction:
7.5 radians and 1.2 are the same way round, and a heading left to accumulate
picks up whole turns nothing can see — until something interpolates it, and they
all come out at once as a spin. So headings are kept inside half a turn either
side of nothing, and every difference between two of them is taken the short
way; the half-way point of a cross-fade is never more than a quarter turn from
either end.

**Which object is on screen never changes**, at any point of the fade or at the
end of it: a fader is for crossing between looks, and a cut to another object is
not one. The clocks do not cross either — the animation keeps running through
the fade and lands on the template's own moment at the end.

They are held twice, exactly as the spheres in §16 are: in this browser so they
survive a reload with no network at all, and one small cloud document each so
they follow the page to another computer. Ten copies of the settings will not
fit inside the settings, which is why they are not a field in it. **Reset never
touches them.**

A template carries every setting, including the ones that leave nothing to look
at, and it is recalled without reading what is in it. One of those reads as a
broken page rather than as a choice: **Transparent** on with **Opacity** at
nothing and **Edge** at nothing keeps no part of the surface at any angle (§21),
so the frame is only whatever shows through it. It is a legitimate thing to ask
for, so it is never undone — but recalling such a template **says so**, and
names the switch, because otherwise finding it means knowing in advance which of
a hundred settings to look at. The notice is appended to the template's name in
the hint line and in the console log (§33).

---

## 24. Depth map

An image thrown at the object from a card standing in the world, the way a decal
projector throws one. Where the picture is bright the surface is pushed out
along its own normal; where it is dark the surface is left where it was. It is
real geometry and not a shading trick — the silhouette moves, and the sun, the
shadows, the matcap and the glass all see the relief and agree about it.

| Control | Range | Default |
| --- | --- | --- |
| **Depth map** | off / on | off |
| **Map** | which image, from the one shared list of §16 | Chrome |
| **Add / Keep / Delete** | as §16, but on the chosen decal | |
| **Target** | which part of this object the beam may touch | Everything |
| **Intensity** | -1.00 – 1.00 | 0.12 |
| **Distance** | 0.05 – 4.00 | 1.20 |
| **Edit** | show the gizmo, or put it away | off |
| **Place** | stand the card in front of the view | |

Off is exact: not a texel is read and the frame is the one it always was. So is
**Intensity at nothing** — the beam's own bounding box is skipped with it, and a
box that only ever shortened a step would still move where a ray landed. A card
standing clear of the object is exact too: the shape it beams at is the shape it
would have had.

**Target.** One decal, aimed at one part, and every object is made of different
parts:

| Object | Parts |
| --- | --- |
| **Brain** | Shell, Cells |
| **Neuron** | Soma, Dendrites, Axon |
| **Chrome** | Slab, Cube |

The chosen part takes the picture **before** it is combined with the rest, so a
soma that has been pushed out still blends into its dendrites the way it always
did, rather than being displaced after the fact and cutting across them. Aimed
at *Everything* the whole field takes it at once. The choice is remembered per
object, because a part of the brain means nothing while the neuron is on screen.

**Reach.** The beam is the card's rectangle swept along its own back axis for
**Distance**. The picture is at full strength for the first part of that and
fades out over the rest, and it fades at the rim of the card as well. Both fades
are what keeps the field continuous: relief that switched on at an edge would be
a wall with no distance to it, and the march would step through the side of its
own decal. The far fade is also what stops a beam longer than the body embossing
the **back** of it, which a distance field cannot tell from the front.

**Intensity below zero carves in** instead of pushing out, and is otherwise the
same thing.

**Cost.** Inside the beam the march is divided down by how steeply the picture
can tilt the field — a deeper bite over a smaller card is a steeper slope — and
outside it nothing is paid. The step budget on Auto grows to cover part of it,
the way it does for a warp, and a benchmark run says which map was used, what it
was aimed at, and what the beam cost the step.

## 25. The gizmo

`G`, the **Edit** button, or **Place** puts a gizmo on the card, drawn over the
picture rather than in it: there are no polygons in this scene to pick against,
so a handle is a shape at a projected point and a drag on it is arithmetic in
the camera's own basis. It is Blender's gizmo, and it works like Blender's.

| Handle | |
| --- | --- |
| **Arrow**, one per axis | move the card along that axis |
| **Ring**, one per axis | turn it about that axis |
| **Square** on the right and top edges | make the card wider or taller |
| **Square** at the end of the beam | set how far the beam reaches |
| **Circle** in the middle | move the card across the view |

The pivot is the centre of the picture, and everything turns and scales about
it. While the gizmo is up the card itself is drawn as a semi-transparent
rectangle with its beam sketched out in front of it, so what is about to be hit
is visible before it is hit. The gizmo is screen-sized rather than world-sized,
so it stays the same size to the hand whether the card is across the room or in
your face; a drag is worked out from where it started rather than frame by
frame, so a slow hand and a fast one land in the same place. `Esc` puts it away,
and so does switching the decal off.

Where the card stands, which way it faces, how big it is and how far it reaches
are all part of the settings: they are kept by **Save as default**, they travel
in a template, and **Reset** puts them back.

---

## 26. What is in the shader

A switch that is off is **not in the program**, rather than a branch the program
never takes. The difference matters: code inside an untaken branch still
reserves the registers it would need, for every pixel, and fewer registers to
go round means fewer pixels in flight at once. That is why switching a feature
off used to buy so little, and why the transparency **Depth** appeared to cost
something with transparency itself switched off.

So the fragment shader is assembled per draw from what is actually on:

| Left out when | |
| --- | --- |
| **Shadows** off | the second march, the budget loop, the painted sphere, the shaping |
| **MatCap** off | the sphere lookup |
| **Normal map** off | the three projections and their blend |
| **Transparency** off | the refraction loop, and the two marches it runs inside and outside the body |
| **Depth map** off | the beam, its bounding box, and every hook it has in an object's field |
| **Test cube** off | the cube, and the bound it widens |
| **Walking** off | the ground probe and the ray it marches (§30) |

Objects were already separate this way — each program carries **one** object's
field, so nothing about the brain is compiled while the neuron is on screen.

Programs are built the first time a combination is asked for and kept for the
session, so toggling back and forth costs one build each way and nothing after
that. Switched off is exact: every "off is byte-identical to the baseline"
claim in this document is now a claim about a **different, smaller program**
producing the same frame to the byte.

Two things stayed runtime checks because leaving them out would not pay for a
variant of its own: **Noise** at zero skips its hash, and **Haze** and **Glow**
are one multiply each. The post pass is one program either way, and reads
**Depth of field** as a switch: off, it does not gather.

---

## 27. Mirror

Blender's mirror modifier, the way a distance field does it. Two switches:

| Control | |
| --- | --- |
| **Mirror X** | fold about the up-down plane: one side is drawn on both |
| **Mirror Y** | fold about the horizontal plane: the top half is the bottom half too |

Folding the space in half is one instruction — take the absolute value of that
coordinate before the object is looked up — and a fold is an isometry on each
side of it, so the field stays a valid lower bound on the distance and the march
pays nothing for it. It is **real geometry**: the silhouette, the shadows, the
shading, the glow and anything marching through it all follow.

Both together fold the object into a quarter and draw that quarter four times.
Neither is the object as it was, to the byte.

The fold is applied **after** the depth decal has been read, so the decal's card
stays where it was put and its beam falls on the mirrored half like anything
else standing in the world.

---

## 28. Overlay

A sprite standing over the render, walking a sheet of frames on a loop.

| Control | Range | Default |
| --- | --- | --- |
| **Overlay** | off / on | off |
| **Sprite** | which image, from the one shared list of §16 | Chrome |
| **Add / Keep / Delete** | as §16, on the chosen **sprite** | |
| **Size** | 3-80% of the window's height | 20% |
| **Frames** | 1-16 laid side by side in the sheet | 4 |
| **Speed** | 0-24 frames a second | 6.0 |

He stands **centred across** the window and keeps **a twentieth of its height
clear beneath him** — so at the size he comes at, three quarters of the height
is open above his head. Both follow the window as it is resized, since both are
measured in it rather than in pixels.

It is a plain element laid over the canvas, not anything in the shader: it costs
the render nothing and cannot disturb a pixel of it. Hiding the panels leaves
him standing there — he is part of the shot, not part of the console.

**Add** takes a sprite sheet: every frame side by side in one strip, any size,
**transparency and all** — a cut-out stays a cut-out however big the sheet is
(see §16), and a sheet that has no transparency in it to begin with says so when
it is loaded and is marked in the list, rather than quietly standing in a box. It goes into the same list §16 keeps, at a larger size than a sphere
is kept at because a strip of frames needs the room, and **Keep** holds on to it
in this browser and in the cloud like any other image. A template remembers the sheet,
the size, the frame count and the speed — but not which frame he is on, which
follows the clock on the wall rather than the field's.

---

## 29. Blend to sphere

| Control | Range | Default |
| --- | --- | --- |
| **Blend to sphere** | 0-1 | 0.00 |

Travels from whatever the object builds toward a plain ball, and back. It is
real geometry the whole way: the silhouette moves, and the shadows, the glow
and anything marching through it follow. At **nothing** the object is untouched
to the byte.

Two fields that are each a lower bound on a distance blend into one that still
is, so the march needs no help for this and pays nothing extra for it — unlike
a warp, which has to be divided down. The ball is read in world coordinates
before any warp is applied, so it stays a ball however bent the space around it
is, and a fold about a plane through its centre leaves it alone.

The ball is sized to whichever object it started from, so each one arrives
somewhere different. The bounding volume grows to hold it when the slider is
off nothing — the chrome slab's own box is a fraction of a unit thick, and
anything outside the bounds is never marched at all, so without that the ball
would come out sliced flat.

---

## 30. Walking

Fly mode can put its feet on the ground. `Tab` while flying switches walking on
and off — the way Blender does — and so does the **Walking** button in the
Camera group.

Walking takes over the up-and-down axis and nothing else. Looking, the two
smoothing sliders, `R`, the saved viewpoints and the templates all behave
exactly as they do in flight.

- `W A S D` move over the ground. Forward is flattened to the horizon, so
  looking up does not climb.
- `Q` and `E` no longer lift or lower. Gravity owns the height.
- `shift` breaks into a run — about two and a half times the walk — while
  **Run** is on.
- `space` jumps, while **Jump** is on and both feet are down.
- Leaving walking hands the camera straight back to free flight, wherever it is.

**The ground comes from the shader.** The field is only known inside the
program that draws it, so the walker asks the question the only way there is to
ask it: one ray, straight down from the eye, rendered into a framebuffer one
pixel wide with **the same program and the same uniforms the frame was just
drawn with**, and read back. What the walker stands on is therefore exactly the
surface that is on screen — warps, folds, blend-to-sphere, mirror and depth
decal included. The answer comes back as a distance in two bytes rather than
one: quantised to 1/255 of the scene, a walker standing still visibly shivers.
**Standing still is exactly still**, to the last decimal — and that takes more
than rounding off the graininess. Chasing the answer does not converge: the ray
starts at the eye, so the distance it returns depends on where the eye is, and
correcting the eye by that distance moves the start of the next ray, which
returns a slightly different distance, which moves it back. Left alone it sits
in a two-frame cycle a millimetre or two wide, for ever. So a walker that is
already standing, not moving and not rising holds its height unless the floor
has moved by more than **a few percent of the walker's own height** — far below
anything an eye can see, and it scales when the walker does. A walk *over* the
ground keeps the tight tolerance instead, or it would climb a slope in steps.

A tolerance alone is not enough, because how big the march's own error is
depends on the relief and the warp and can be wider than any fixed number. So
there is a second rule, and it does not depend on the size of anything: **while
standing, only act on a reading the frame before it already agreed with.** A
cycle cannot do that — the act of correcting the eye is what changes the next
answer, so two readings in a row never match. A floor that is really there, or
one that is moving because the field is animating, does match, and is followed
as it moves.

**Where there is nothing below, there is nothing to stand on.** The chrome
object in its default *intersect* mode is islands with real gaps between them —
the two slabs only overlap where the height field is shallow — so a walker can
step off the edge of the world. It falls, and once it is further below the
object than the object is wide it is put back above it rather than falling for
ever.

**The Camera group gains:**

| Control | Range | Default |
| --- | --- | --- |
| **Walking** | on / off | off |
| **Jump** | on / off | on |
| **Run** | on / off | on |
| **Steps** | on / off | off |
| **Add** (step sample) | a file from this computer | none |
| **Controller size** | 0.01–4.00, or typed | 1.00 |
| **Walk speed** | 0.03-4.00, or typed | 0.45 |
| **Jump height** | 0.01-2.00, or typed | 0.22 |

**Controller size is one knob for the whole body.** How tall the walker stands,
how long its stride is, how fast it walks and how high it jumps all scale with
it together — which is what stops a slider that shrinks the walker to an ant
from also turning it into a rocket. Turn it right down to walk over the fine
detail of an object rather than striding across it; the ground it finds is the
same ground either way, it is simply standing closer to it.

**Jump height is the height**, not an impulse: gravity is worked back out of the
number so that a jump of 0.22 rises 0.22 and lands again, whatever else is set.

**Steps** plays a sound each time a foot lands. The interval is a distance
walked, not a clock, so running steps come faster on their own and standing
still makes no sound at all; landing from a jump makes one. **Add** loads a
sample from the computer and it is used until the page is closed — it is not
saved into a template or the default. Without one the page synthesises a plain
low thud.

---

## 31. The lens, and depth of field

**Field of view** is in the Camera group.

| Control | Range | Default |
| --- | --- | --- |
| **Field of view** | 100°-8°, or typed in degrees | 42° |

The reading is the vertical angle the frame takes in, and a number typed into it
is read as degrees. The slider runs the other way round — wide angle on the
left, long lens on the right — because that is the direction the picture opens
out in. A narrow window still picks a wider lens of its own on first load; the
control simply starts wherever that left it.

**Depth of field** is the rest of the group.

| Control | Range | Default |
| --- | --- | --- |
| **Depth of field** | on / off | off |
| **Focus distance** | 0.10-12.00, or typed | 3.00 |
| **Focus depth** | 0.05-6.00, or typed | 0.80 |
| **Blur** | 0.0-40.0 pixels | 4.0 |

Three controls, in the shape that was asked for and that a camera actually has:
a surface that is sharp, a band around it that is also sharp, and how soft
everything beyond that band gets. **Focus depth** is the half-width of the band
— the same amount in front of the focus surface and behind it — and the blur
then comes on over a second band of the same width, so nothing snaps from sharp
to soft at an edge. Past that it is fully blurred and gets no worse; a real lens
keeps going, but a circle of confusion that grows without limit costs more every
frame to draw the further away the background is.

**How it is done.** The march already knows how far it went, so each pixel
writes its own circle of confusion into the alpha channel of the frame — the
drawing buffer has no alpha of its own, so nothing else was ever going to read
it — and the post pass gathers thirteen taps on a golden-angle spiral, weighted
by that same number so a sharp foreground cannot bleed outward over a blurred
background. Background pixels, which never hit anything, take the value the
distance fade ends at.

**Off is exact.** With **Depth of field** off, the shader writes 1.0 and the
post pass does not gather at all: the frame is byte-identical to the frame
before this existed. **Blur** at zero is the same frame again, with the switch
on.

---

## 32. Seed

| Control | Range | Default |
| --- | --- | --- |
| **Seed** | 0-999 on the slider, any number typed | 0 |
| **Roll** | takes one at random | |

One number decides **which roll of everything generated is on screen**: where
the brain's cells sit, which way the neuron's dendrites leave the soma, what the
chrome landscape looks like, and how the domain warp falls. Nothing about the
*kind* of object changes — a brain is still a brain, with the same cell pitch
and the same shell — only which particular one.

**Zero is the page's own roll**, so it is what every stored baseline, the native
port and every template written before this existed all mean, and the frame at
seed zero is byte-identical to the frame before the control existed.

It is one place to look in the noise rather than a number fed into it: the seed
becomes an offset, three strides that share no common measure, added to the
coordinate each noise lattice and each cell id is read at. That costs one
addition per sample instead of one per hash — a noise lookup takes eight hashes
— and it means two seeds never land on the same lattice. The seed is saved with
a template, and crossing between two templates **flips** it at the half-way
mark rather than sliding through every roll in between.

---

## 33. The console

A window that comes **down from the top** on `` ` `` and goes back up on `` ` ``
or `Esc`. It runs from the left edge of the window **across to the menu's edge,
exactly** — and with the panels hidden (`H`), across the lot. It is not part of
the HUD: hiding the panels leaves it wherever it was.

`shift+`` ` `` is still Fly mode. The plain key is the console.

**It writes down everything the page says out loud.** Every notice that flashes
under the menu and is gone in a second and a half is kept here with the second
it happened at, along with any fault the page throws. That is the point of it:
a notice you missed is not a notice.

**It takes commands.** The field it opens with the focus in takes:

| | |
| --- | --- |
| `help` | the list |
| `get <name>` | what a setting is now |
| `set <name> <value>` | change one |
| `find <text>` | every setting whose name has that in it |
| `keys` | what the keyboard does |
| `dump` | every setting as JSON, and on the clipboard |
| `dump <name or number>` | one template the same way |
| `clear` | empty the log |

A name is any setting that a saved default holds — the same list a template is
made of — so anything in the menu can be reached by name, and so can a few
things that are not in it. A value is a number, `on` / `off`, a colour, or a
JSON array for the ones that are lists (`set warpOn [1,0,1]`). A value typed
here goes in **through the same controls a hand would use**, so the menu, the
readings and the pressed states all follow. A name that does not exist comes
back with the closest ones that do.

`dump` is how a setting or a template leaves the machine it was made on. It
writes the JSON into the log and puts it on the clipboard, so a template that
only exists in one browser can be handed to someone else — which matters
because the cloud copy of a template can be written but, under the rules the
project has, not listed back.

The up and down arrows walk back through what has been typed. The log keeps the
last 400 lines.

---

## 34. Pictures travel by name

Every image slot — the MatCap sphere, the shadow sphere, the normal map, the
depth-map card and the overlay sprite — is stored as a **position** in this
browser's own list of images. That list is the built-in ones plus whatever this
browser has loaded, so the same number on another machine is a different
picture, or no picture at all.

So a saved setting and a template also write down **what each slot was pointing
at, by name**. On arrival the page finds it again by name: if the picture is
here under a different number, the slot is corrected; if it is not here at all,
the page **says so** rather than quietly wearing the wrong one.

The images themselves do not travel. They are kept in the cloud under
`matcaps/{id}` and are subject to the same `list` restriction as the templates
(§9), so an image loaded on one machine stays on it.

---

## 35. Octaves

Two controls, one number each: how many layers of noise a thing is built from.

| Control | Where | Range | Default |
| --- | --- | --- | --- |
| **Warp octaves** | every object | 1–4 | 1 |
| **Relief octaves** | chrome only | 1–4 | 2 |

Each layer is **half the amplitude and 2.03 times the rate** of the one before
it, offset so the layers do not line up and — for the relief, which drifts —
climbing a little faster in time.

**More octaves is more detail and never more size.** The sum is normalised back
to what the shipped count summed to, so the height **Relief** asks for and the
strength **Warp strength** asks for are the height and the strength you get at
any octave count. Only the fineness changes.

**The shipped counts are exact.** Relief at 2 and warp at 1 draw what the page
has always drawn, to the byte — which is what every stored baseline in
`tools/shadows.mjs` and the native port's parity depend on. That is not free to
arrange: the first two relief terms are written out literally rather than
generated by a loop, because `0.06 + 0.03` is not the float `0.09` and a loop
that computed the same arguments would not compute the same numbers. For the
same reason the relief's Lipschitz scaling is skipped outright at two octaves
instead of being multiplied by a factor that is only nearly one.

**The march has to be told.** A finer octave is a steeper one, and the step size
is derived from an upper bound on the field's slope: leave that bound at its
one-octave value and the march walks straight through the creases the extra
octaves just carved, which shows up as holes in the object. So both the warp's
`lip` and the chrome's `sceneLip()` scale with the octaves' combined slope over
their combined amplitude.

**It is not free.** Each octave is another noise lookup at every march step —
eight hash evaluations — and the shorter steps its steepness forces cost more
again. Four octaves of warp is a noticeably heavier frame than one.
