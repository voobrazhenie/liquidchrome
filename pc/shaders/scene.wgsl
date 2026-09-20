// Cortical Flythrough — scene shader.
//
// A line-by-line port of the WebGL fragment shader in index.html (<script id="fs">),
// kept in the same order and with the same names so the two can be diffed. Anything
// that had to change for WGSL is marked NATIVE.
//
// One object at a time: shaderpp.rs strips the //#ifdef blocks that do not belong to
// the object being built, so each pipeline carries only its own SDF — the same reason
// the page compiles one program per object.

struct Uniforms {
  res: vec2f,
  time: f32,
  roll: f32,
  eye: vec4f,        // xyz used
  tgt: vec4f,        // xyz used
  colA: vec4f,       // structure colour of the active object
  colB: vec4f,       // accent colour
  warpOn: vec4f,     // per-deformation switches: x noise, y twist, z bend
  lcOn: vec4f,       // LiquidChrome steps 1-4: slab, relief, copy, intersect
  scale: f32,        // cell density
  spike: f32,
  steps: f32,
  fov: f32,
  shift: f32,
  morph: f32,
  pulse: f32,
  scene: f32,
  thick: f32,
  warpAmt: f32,      // global warp strength
  warpFreq: f32,     // noise warp frequency
  lcTwirl: f32,      // step 5, radians at the centre; 0 when the step is off
  lcRelief: f32,
  lcFreq: f32,
  lcThick: f32,      // half-thickness of the slab
  eps: f32,          // how close to the surface a ray must get to count as a hit
  omega: f32,        // over-relaxation: how far past the safe radius a step reaches
  bound: f32,        // bounding volume: 0 sphere, 1 box, 2 pick per object
  boundPad: f32,     // slack added to it
  warpMode: f32,     // noise warp: 0 bends the space, 1 pushes the surface
  // NATIVE: the surface, and the passes after it. Every one of these was a
  // constant in the page's shader; the defaults below reproduce it exactly.
  matSmooth: f32,    // 0 rough .. 1 mirror. 0.5 is the page's highlight
  matMetal: f32,     // 0 dielectric .. 1 metal: the highlight takes the body colour
  postExposure: f32, // scales the image into the tone map
  postGlow: f32,     // the haze the march accumulates around the surface
  postFog: f32,      // how far the background reaches into the object
  postVignette: f32, // corner falloff
  postGrain: f32,    // the dither that keeps the gradients from banding
  postPad: f32,
  // The background gradient, and the two tints the page hard-coded into the
  // lighting. Those two are why an object with both of its colours set to black
  // was still not black: the rim and the back light carried their own colour.
  bgLow: vec4f,      // looking down
  bgHigh: vec4f,     // looking up
  rimCol: vec4f,     // fresnel rim and the specular highlight
  spikeCol: vec4f,   // back light, travelling impulses, and half of the glow
};

@group(0) @binding(0) var<uniform> U: Uniforms;

// NATIVE: the web build bakes the march length into the shader as a constant,
// because ANGLE lays the loop out at link time and needs to know how long it is.
// Here the budget comes from the uniform and nothing about the trip count is known
// when the shader is compiled, so no driver can unroll it — which is what the page's
// "retry with a shorter march" workaround exists to survive. The clamp is the only
// bound: a budget of 0, or a NaN, still leaves the loop finite.


fn gy(p: vec3f) -> f32 { return dot(sin(p), cos(p.yzx)); }

fn hash33(p0: vec3f) -> vec3f {
  var p = fract(p0 * vec3f(0.1031, 0.1030, 0.0973));
  p = p + vec3f(dot(p, p.yxz + vec3f(33.33)));
  return fract((p.xxy + p.yxx) * p.zyx);
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
  let h = clamp(0.5 + 0.5*(b-a)/k, 0.0, 1.0);
  return mix(b, a, h) - k*h*(1.0-h);
}
fn smax(a: f32, b: f32, k: f32) -> f32 {
  let h = clamp(0.5 - 0.5*(b-a)/k, 0.0, 1.0);
  return mix(b, a, h) + k*h*(1.0-h);
}

fn sdEll(p: vec3f, r: vec3f) -> f32 {
  let k0 = length(p/r);
  let k1 = length(p/(r*r));
  return k0*(k0-1.0)/max(k1, 1e-5);
}

//#ifdef SCENE_BRAIN
fn shell(p: vec3f) -> f32 {
  var q = p; q.x = abs(q.x) - 0.50;
  var d = sdEll(q, vec3f(0.56 * (1.0 - 0.13*q.z), 0.63, 1.12));
  var c = p - vec3f(0.0, -0.50, -0.80); c.x = abs(c.x) - 0.22;
  d = smin(d, sdEll(c, vec3f(0.30, 0.24, 0.34)), 0.13);
  d = smin(d, sdEll(p - vec3f(0.0, -0.78, -0.34), vec3f(0.15, 0.34, 0.19)), 0.14);
  d = d - 0.030 * sin(p.x*5.2 + U.morph*0.37) * sin(p.y*4.4 + 1.3 - U.morph*0.29) * sin(p.z*4.8 + 2.1 + U.morph*0.31);
  return d;
}
//#endif

fn spikeAt(p: vec3f) -> f32 {
  if (U.spike <= 0.0) { return 0.0; }   // impulses off: no travelling light anywhere
  let w = sin(dot(p, vec3f( 1.4, 0.7, 1.9)) * 3.2 - U.pulse*2.3)
        + sin(dot(p, vec3f(-1.7, 1.2, 0.6)) * 4.1 - U.pulse*3.0)
        + 0.7 * sin(length(p) * 7.0 - U.pulse*4.2);
  return smoothstep(1.25, 2.35, w);
}

//#ifdef SCENE_NEURON
fn dot2(v: vec3f) -> f32 { return dot(v, v); }

// 3D value noise — two octaves of it perturb the membrane
fn vn(p: vec3f) -> f32 {
  let i = floor(p);
  var f = fract(p);
  f = f*f*(3.0 - 2.0*f);
  let e = vec2f(0.0, 1.0);
  let a = hash33(i + e.xxx).x; let b = hash33(i + e.yxx).x;
  let c = hash33(i + e.xyx).x; let d = hash33(i + e.yyx).x;
  let g = hash33(i + e.xxy).x; let h = hash33(i + e.yxy).x;
  let k = hash33(i + e.xyy).x; let l = hash33(i + e.yyy).x;
  return mix(mix(mix(a, b, f.x), mix(c, d, f.x), f.y),
             mix(mix(g, h, f.x), mix(k, l, f.x), f.y), f.z) - 0.5;
}

// exact distance to a quadratic Bezier (Quilez), plus the curve parameter so
// the tube can taper along its length
fn sdBez(p: vec3f, A: vec3f, B: vec3f, C: vec3f, tOut: ptr<function, f32>) -> f32 {
  let a = B - A;
  let b = A - 2.0*B + C + vec3f(1e-5);
  let c = a * 2.0;
  let d = A - p;
  let kk = 1.0 / dot(b, b);
  let kx = kk * dot(a, b);
  let ky = kk * (2.0*dot(a,a) + dot(d,b)) / 3.0;
  let kz = kk * dot(d, a);
  var res = 0.0;
  let p1 = ky - kx*kx;
  let q  = kx*(2.0*kx*kx - 3.0*ky) + kz;
  var h  = q*q + 4.0*p1*p1*p1;
  if (h >= 0.0) {
    h = sqrt(h);
    let x = (vec2f(h, -h) - vec2f(q)) * 0.5;
    let uv = sign(x) * pow(abs(x), vec2f(1.0/3.0));
    let t = clamp(uv.x + uv.y - kx, 0.0, 1.0);
    *tOut = t;
    res = dot2(d + (c + b*t)*t);
  } else {
    let z = sqrt(-p1);
    let v = acos(clamp(q / (p1*z*2.0), -1.0, 1.0)) / 3.0;
    let m = cos(v); let nn = sin(v) * 1.732050808;
    let t = clamp(vec3f(m+m, -nn-m, nn-m)*z - vec3f(kx), vec3f(0.0), vec3f(1.0));
    let r1 = dot2(d + (c + b*t.x)*t.x);
    let r2 = dot2(d + (c + b*t.y)*t.y);
    *tOut = select(t.y, t.x, r1 < r2);
    res = min(r1, r2);
  }
  return sqrt(res);
}

// a Bezier segment swept by a radius that tapers from r0 to r1
fn tube(p: vec3f, A: vec3f, B: vec3f, C: vec3f, r0: f32, r1: f32) -> f32 {
  var t = 0.0;
  let d = sdBez(p, A, B, C, &t);
  return d - mix(r0, r1, t);
}

// one cell: soma, five unequal dendrites, a Y fork on each, a thick axon
fn neuron(p: vec3f, sd: ptr<function, f32>) -> f32 {
  *sd = length(p);
  let m = U.morph * 0.5;
  let r = U.thick;
  var d = (*sd) - (0.165 + r*0.8);

  for (var i = 0; i < 5; i++) {
    let fi = f32(i);
    let hh = hash33(vec3f(fi*1.7 + 0.3, 3.1, 7.3));
    let ang = fi * 2.39996 + hh.x*0.7;
    let z = 1.0 - (2.0*fi + 1.0) / 5.0;
    let rr = sqrt(max(0.0, 1.0 - z*z));
    let dir = normalize(vec3f(rr*cos(ang), z*0.92 + hh.y*0.30 - 0.15, rr*sin(ang)) + vec3f(1e-4));
    let len = 0.62 + 0.30*hh.z;

    // whole branch, children included, sits inside this sphere — skip it early
    if (length(p - dir*len*0.9) - (len*1.05 + 0.22) > d) { continue; }

    let sv = normalize(cross(dir, vec3f(0.31, 0.79, 0.53)) + vec3f(1e-4));
    let up = normalize(cross(dir, sv));
    let w1 = sin(m*0.61 + fi*1.7); let w2 = sin(m*0.47 + fi*2.9);

    let A = dir * 0.07;
    let B = dir * len * 0.55 + sv * (0.20 + 0.10*w1) * len;
    let C = dir * len + sv * (0.06 + 0.18*w2) * len + up * 0.06 * w1;
    let rb = r * (1.60 + 0.55*hh.x);   // trunk, thick where it leaves the soma
    let rm = r * (0.78 + 0.30*hh.y);   // waist at the first bifurcation
    d = smin(d, tube(p, A, B, C, rb, rm), 0.09);

    let fwd = normalize(C - B + vec3f(1e-5));
    for (var j = 0; j < 2; j++) {
      let sgn = select(-1.0, 1.0, j == 0);
      let off = normalize(sv*sgn*0.85 + up*(0.30*sgn + 0.22*w2) + fwd*0.25);
      let B2 = C + fwd*0.16*len + off*0.16*len;
      let C2 = C + fwd*0.30*len + off*0.46*len + up*0.06*w1;
      d = smin(d, tube(p, C, B2, C2, rm*0.92, rm*(0.34 + 0.22*hh.x)), 0.035);
    }
  }

  // axon: thicker, straighter, out of the far side
  d = smin(d, tube(p, vec3f(0.0, -0.10, 0.0),
                      vec3f(0.26*sin(m*0.40), -0.70, 0.20),
                      vec3f(-0.32, -1.24, -0.18 + 0.22*sin(m*0.33)),
                   r*1.5, r*1.05), 0.06);

  // organic membrane: coarse lobes for the silhouette, fine grain for skin
  let w = 1.0 - clamp(d / 0.085, 0.0, 1.0);
  if (w > 0.0) {
    let lobe = vn(p * 3.0 + vec3f(0.0, m*0.09, 0.0));
    let skin = vn(p * 24.0);
    d = d - w * ((0.60*r + 0.023) * lobe + (0.14*r + 0.007) * skin);
  }
  return d;
}
//#endif

fn sceneLip() -> f32 {
//#ifdef SCENE_CHROME
  if (U.lcOn.y > 0.5) {
    // the height field turns the box map into I - y (x) grad h, whose operator
    // norm is sqrt(1 + |grad h|^2) — tighter than 1 + |grad h|, so bigger steps
    let g = U.lcRelief * U.lcFreq * 3.0;
    return sqrt(1.0 + g*g);
  }
//#endif
  return 1.0;
}

//#ifdef SCENE_CHROME
fn sdBox(p: vec3f, b: vec3f) -> f32 {
  let q = abs(p) - b;
  return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// classic 3D gradient noise (Perlin): a random unit gradient per lattice corner,
// dotted with the offset and blended with the quintic fade, so it is C2
fn pgd(i: vec3f, f: vec3f) -> f32 { return dot(normalize(hash33(i) - vec3f(0.5)), f); }
fn perlin(p: vec3f) -> f32 {
  let i = floor(p); let f = fract(p);
  let u = f*f*f*(f*(f*6.0 - vec3f(15.0)) + vec3f(10.0));
  let a = pgd(i + vec3f(0.0,0.0,0.0), f - vec3f(0.0,0.0,0.0));
  let b = pgd(i + vec3f(1.0,0.0,0.0), f - vec3f(1.0,0.0,0.0));
  let c = pgd(i + vec3f(0.0,1.0,0.0), f - vec3f(0.0,1.0,0.0));
  let d = pgd(i + vec3f(1.0,1.0,0.0), f - vec3f(1.0,1.0,0.0));
  let e = pgd(i + vec3f(0.0,0.0,1.0), f - vec3f(0.0,0.0,1.0));
  let g = pgd(i + vec3f(1.0,0.0,1.0), f - vec3f(1.0,0.0,1.0));
  let h = pgd(i + vec3f(0.0,1.0,1.0), f - vec3f(0.0,1.0,1.0));
  let k = pgd(i + vec3f(1.0,1.0,1.0), f - vec3f(1.0,1.0,1.0));
  return 1.40 * mix(mix(mix(a,b,u.x), mix(c,d,u.x), u.y),
                    mix(mix(e,g,u.x), mix(h,k,u.x), u.y), u.z);
}

// LiquidChrome, built exactly as the five steps read:
//   1  a 20 x 20 x 0.5 slab (normalised to half-extents 1, uLcThick, 1)
//   2  displaced along its thin axis by two octaves of Perlin -> a landscape
//   3  an undeformed copy of the same slab, in the same place
//   4  kept only where the two overlap  (max of the two distances)
//   5  the result twirled about the vertical axis
fn liquid(p: vec3f, sd: ptr<function, f32>) -> f32 {
  *sd = 1e5;
  var q = p;

  // 5 - twirl. Applied to the coordinates ahead of both slabs, which twirls
  // the intersection itself. The angle falls off with radius, so the shear it
  // adds is bounded by the twirl amount.
  if (U.lcTwirl != 0.0) {
    let r = length(q.xz);
    let a = U.lcTwirl * max(0.0, 1.0 - r / 1.45);
    let c = cos(a); let sn = sin(a);
    q = vec3f(c*q.x - sn*q.z, q.y, sn*q.x + c*q.z);
  }

  let he = vec3f(1.0, U.lcThick, 1.0);

  // 2 - the height field, two octaves, drifting when the field is animated
  var h = 0.0; var lip = 1.0;
  if (U.lcOn.y > 0.5) {
    let f = U.lcFreq;
    h = perlin(vec3f(q.x*f, U.morph*0.06, q.z*f))
      + 0.5 * perlin(vec3f(q.x*f*2.03 + 5.2, U.morph*0.09 + 1.7, q.z*f*2.03 - 3.1));
    h = h * 0.667 * U.lcRelief;
    // a displaced box is no longer 1-Lipschitz; the slope of the height field
    // adds to the gradient, so divide by an upper bound on it
    lip = sceneLip();
  }

  let dA = sdBox(q - vec3f(0.0, h, 0.0), he) / lip;   // 1 + 2
  let dB = sdBox(q, he);                              // 3

  var d = 1e5;
  if (U.lcOn.x > 0.5 && U.lcOn.z > 0.5) {
    d = select(min(dA, dB), max(dA, dB), U.lcOn.w > 0.5);   // 4 - intersect, else union
  } else if (U.lcOn.x > 0.5) {
    d = dA;
  } else if (U.lcOn.z > 0.5) {
    d = dB;
  }

  // tint by altitude: the crests take the accent colour
  *sd = (0.5 - 0.5 * clamp(h / max(U.lcRelief, 1e-3), -1.0, 1.0)) * 0.42;
  return d;
}
//#endif

// three channels of value noise out of one set of eight corner hashes
fn vn3(p: vec3f) -> vec3f {
  let i = floor(p);
  var f = fract(p);
  f = f*f*(3.0 - 2.0*f);
  let e = vec2f(0.0, 1.0);
  let a = hash33(i + e.xxx); let b = hash33(i + e.yxx);
  let c = hash33(i + e.xyx); let d = hash33(i + e.yyx);
  let g = hash33(i + e.xxy); let h = hash33(i + e.yxy);
  let k = hash33(i + e.xyy); let l = hash33(i + e.yyy);
  return mix(mix(mix(a, b, f.x), mix(c, d, f.x), f.y),
             mix(mix(g, h, f.x), mix(k, l, f.x), f.y), f.z) - vec3f(0.5);
}

// Deform the coordinate space itself, before any object is looked up.
// Composing warps multiplies their Lipschitz constants; `lip` accumulates an
// upper bound on that product. Dividing the scene distance by it turns the
// warped field back into a valid lower bound on the true distance, which is
// exactly what sphere tracing needs in order not to step through a surface.
fn domainWarp(p0: vec3f, lip: ptr<function, f32>) -> vec3f {
  var p = p0;
  *lip = 1.0;
  let amt = U.warpAmt;

  // bend — rotate about Z by an angle that grows along X.
  // The rotation is an isometry; the angle varying with x adds k*r.
  if (U.warpOn.z > 0.5) {
    let k = 1.60 * amt;
    let a = k * p.x;
    let c = cos(a); let sn = sin(a);
    p = vec3f(c*p.x - sn*p.y, sn*p.x + c*p.y, p.z);
    *lip = (*lip) * sqrt(1.0 + k*k*dot(p.xy, p.xy));
  }

  // twist — rotate about Y by an angle that grows along Y (a whirl).
  if (U.warpOn.y > 0.5) {
    let k = 1.70 * amt;
    let a = k * p.y;
    let c = cos(a); let sn = sin(a);
    p = vec3f(c*p.x - sn*p.z, p.y, sn*p.x + c*p.z);
    *lip = (*lip) * sqrt(1.0 + k*k*dot(p.xz, p.xz));
  }

  // noise displacement — the domain warp proper. Smoothstep value noise is
  // C1, so the displaced space stays continuous; its per-axis gradient peaks
  // at 1.5 per unit cell, and 2.2 covers the 3-channel worst case.
  // In object mode this block is skipped and the same noise is applied to the
  // surface instead (see mapLip). Twist and bend stay space warps either way.
  if (U.warpOn.x > 0.5 && U.warpMode < 0.5) {
    let f = U.warpFreq;
    let A = 0.55 * amt;
    p = p + A * vn3(p*f + vec3f(0.0, U.morph*0.045, 0.0));
    *lip = (*lip) * (1.0 + A * f * 2.2);
  }
  return p;
}

// the undistorted scene: the active object, evaluated in whatever space it is given
fn sceneMap(p: vec3f, sd: ptr<function, f32>) -> f32 {
//#ifdef SCENE_CHROME
  return liquid(p, sd);
//#endif
//#ifdef SCENE_NEURON
  return neuron(p, sd);
//#endif
//#ifdef SCENE_BRAIN
  *sd = 1e5;
  let sh = shell(p);
  if (sh > 0.09) { return sh - 0.02; }

  let s = U.scale;
  let C = 1.45 / s;

  // every soma drifts along its own short line as the field morphs
  let wob = vec3f(sin(U.morph*0.31), sin(U.morph*0.27 + 1.7), sin(U.morph*0.23 + 3.1));

  let id = floor(p/C - vec3f(0.5));
  for (var i = 0; i < 2; i++) {
    for (var j = 0; j < 2; j++) {
      for (var k = 0; k < 2; k++) {
        let cid = id + vec3f(f32(i), f32(j), f32(k));
        let h = hash33(cid);
        var cen = (cid + vec3f(0.5) + (h - vec3f(0.5)) * 0.55) * C;
        cen = cen + C * 0.40 * (wob * (h - vec3f(0.5)) + wob.zxy * (h.yzx - vec3f(0.5)));
        *sd = min(*sd, length(p - cen) / (0.70 + 0.50*h.z));
      }
    }
  }
  let u = (*sd) / C;
  let bead = 0.55 * exp(-3.2 * u * u);

  // the neurite lattice flows and slowly counter-rotates against itself
  let q = p * s + vec3f(sin(U.morph*0.21), cos(U.morph*0.17), sin(U.morph*0.13)) * 2.6;
  let ca = cos(U.morph*0.055); let sa = sin(U.morph*0.055);
  let q2 = vec3f(q.x*ca - q.z*sa, q.y, q.x*sa + q.z*ca) * 0.61 + vec3f(2.7, 1.3, 4.1);
  let w1 = abs(gy(q))  - (0.27 + bead);
  let w2 = abs(gy(q2)) - (0.32 + bead);
  let d = smax(w1, w2, 0.22) / (2.1 * s);
  return smax(d, sh, 0.05);
//#endif
}

// Object warp: rather than bending the space the object sits in, push the
// object's own surface in and out along its normal by the same noise field.
// It is real geometry — the silhouette moves and the normals follow — but the
// step penalty is only paid where a lobe can actually reach. Further out the
// ray subtracts the deepest a lobe could ever pull the surface toward it and
// keeps its full-length step, which the space warp can never do because the
// whole domain is distorted everywhere at once.
fn mapLip(p: vec3f, sd: ptr<function, f32>, lip: ptr<function, f32>) -> f32 {
  var l = 1.0;
  let q = domainWarp(p, &l);
  var d = sceneMap(q, sd);
  if (U.warpMode > 0.5 && U.warpOn.x > 0.5 && U.warpAmt > 0.0) {
    // The height is a fraction of one lobe, not a fixed length: a bump as deep
    // as the features it sits on stops being relief and just inflates them
    // into each other. Tying it to the scale keeps the slope — and so the
    // cost — constant however fine the noise is set.
    let A = 0.45 * U.warpAmt / max(U.warpFreq, 1.0);
    let reach = 0.5 * A;                   // vn3 spans -0.5 .. 0.5
    if (d > 2.0 * reach) {
      // Out of every lobe's reach, so subtract the deepest one could pull the
      // surface this way and keep the full-length step.
      d = d - reach;
    } else {
      d = d - A * vn3(q*U.warpFreq + vec3f(0.0, U.morph*0.045, 0.0)).x;
      l = l * (1.0 + A * U.warpFreq * 1.5);   // smoothstep noise peaks at 1.5/cell
    }
  }
  *lip = l;
  return d / l;
}

fn map(p: vec3f, sd: ptr<function, f32>) -> f32 {
  var lip = 1.0;
  return mapLip(p, sd, &lip);
}

fn calcNormal(p: vec3f) -> vec3f {
  var t = 0.0;
  let e = vec2f(1.0, -1.0) * 0.0016;
  return normalize(
    e.xyy * map(p + e.xyy, &t) + e.yyx * map(p + e.yyx, &t) +
    e.yxy * map(p + e.yxy, &t) + e.xxx * map(p + e.xxx, &t));
}

fn bgCol(rd: vec3f) -> vec3f {
  return mix(U.bgLow.xyz, U.bgHigh.xyz, rd.y*0.5 + 0.5);
}

// ---- bounding volume -------------------------------------------------
// The march only runs where the object can be. A sphere fits the brain and the
// neuron; the chrome slab is 2 x 2 across and a fraction of that thick, so a
// box skips the empty air above and below that a sphere still marches through.
fn boundHalf() -> vec3f {
//#ifdef SCENE_CHROME
  // the twirl sweeps the slab's corners, so across x and z the reach is the
  // corner radius whatever the angle — and a twist warp is bounded by it too.
  // y is where the tightness actually is.
  let relief = select(0.0, U.lcRelief, U.lcOn.y > 0.5);
  // intersecting with the undeformed copy clips the relief straight back off
  let isect = U.lcOn.x > 0.5 && U.lcOn.z > 0.5 && U.lcOn.w > 0.5;
  var ry = U.lcThick + select(relief, 0.0, isect);
  // bend tips space about Z, which lifts the slab out of an axis-aligned box
  if (U.warpOn.z > 0.5) { ry = 1.74; }
  return vec3f(1.4143, ry, 1.4143);
//#endif
//#ifdef SCENE_BRAIN
  return vec3f(1.74);
//#endif
//#ifdef SCENE_NEURON
  return vec3f(1.74);
//#endif
}

fn autoBox() -> bool {
//#ifdef SCENE_CHROME
  return true;
//#endif
//#ifdef SCENE_BRAIN
  return false;
//#endif
//#ifdef SCENE_NEURON
  return false;
//#endif
}

fn render(fc: vec2f) -> vec3f {
  var uv = (fc - 0.5*U.res) / min(U.res.x, U.res.y);
  uv.y = uv.y - U.shift;
  let cr = cos(U.roll); let sr = sin(U.roll);
  uv = vec2f(cr*uv.x + sr*uv.y, -sr*uv.x + cr*uv.y);   // mat2(cr,-sr,sr,cr) * uv

  let ro = U.eye.xyz;
  let ww = normalize(U.tgt.xyz - ro);
  let uu = normalize(cross(ww, vec3f(0.0, 1.0, 0.0)));
  let vv = cross(uu, ww);
  let rd = normalize(uv.x*uu + uv.y*vv + U.fov*ww);

  var col = bgCol(rd);
  var glow = 0.0;

  // rotations preserve radius; only the noise term can push the surface out
  let pad = select(0.0, 0.55 * U.warpAmt, U.warpOn.x > 0.5) + U.boundPad;
  let useBox = select(U.bound > 0.5, autoBox(), U.bound > 1.5);

  var tNear = 0.0; var tFar = -1.0;
  if (useBox) {
    let he = boundHalf() + vec3f(pad);
    let rs = rd + step(abs(rd), vec3f(1e-6)) * 1e-6;   // never divide by zero
    let inv = vec3f(1.0) / rs;
    let t0 = (-he - ro) * inv; let t1 = (he - ro) * inv;
    let lo = min(t0, t1); let hi = max(t0, t1);
    tNear = max(max(max(lo.x, lo.y), lo.z), 0.0);
    tFar  = min(min(hi.x, hi.y), hi.z);
  } else {
    let bR = 1.74 + pad;
    let b = dot(ro, rd);
    let cc = dot(ro, ro) - bR*bR;
    let disc = b*b - cc;
    if (disc > 0.0) {
      let hs = sqrt(disc);
      tNear = max(-b - hs, 0.0);
      tFar  = -b + hs;
    }
  }

  if (tFar > tNear) {
    var t = tNear;
    var sd = 1e5; var sdHit = 1e5; var dmy = 0.0;
    var hit = 0.0; var d = 1e5; var lip = 1.0;
    // Over-relaxed sphere tracing: a plain march steps by the safe radius,
    // which is conservative on a surface it is running alongside. Stepping
    // omega times further converges in fewer iterations, and is still exact
    // because a step whose sphere fails to touch the previous one is walked
    // back to where the plain step would have landed.
    var omega = U.omega; var prevD = 0.0; var stepLen = 0.0;

    var i = 0u;
    let budget = u32(clamp(U.steps, 1.0, 4096.0));
    loop {
      if (i >= budget) { break; }
      i = i + 1u;
      let p = ro + rd*t;
      d = mapLip(p, &sd, &lip);
      // the warp shrinks reported distances; the glow wants the real one,
      // otherwise warped regions bloom into a milky haze
      let lipT = lip * sceneLip();          // reported distance -> true distance
      glow = glow + exp(-38.0 * max(d * lipT, 0.0)) * (0.45 + 1.05*spikeAt(p)) * select(0.017, 0.008, U.scene > 0.5);
      // the spheres no longer overlap, so the last step jumped a gap it had
      // no right to: undo the relaxed part of it and march plainly from here
      let sorFail = omega > 1.0 && (d + prevD) < stepLen;
      if (sorFail) {
        stepLen = stepLen - omega * stepLen;   // negative, so t moves back
        omega = 1.0;
      } else if (d * lipT < U.eps * (0.0009*t + 0.0005)) {
        t = t + d * lipT;                      // land on the surface, not on the sample
        hit = 1.0; sdHit = sd;
        break;
      } else {
        stepLen = d * omega * 0.98;
      }
      prevD = d;
      t = t + stepLen;
      if (t > tFar) { break; }
    }
    // A ray that spent its budget just short of a surface it was converging
    // on gets shaded AT that surface, not where it stopped: shading the
    // stopping point is what terraces a smooth surface into ledges, one per
    // iteration count. d is the compressed distance, so scale it back first.
    let trueD = d * lip * sceneLip();
    if (hit < 0.5 && t <= tFar && trueD < 0.05) {
      t = t + trueD;
      hit = 1.0; sdHit = sd;
    }

    if (hit > 0.5) {
      let p = ro + rd*t;
      let n = calcNormal(p);
      let C = select(1.45 / U.scale, 0.42, U.scene > 0.5);
      let somaMix = 1.0 - smoothstep(C*0.18, C*0.62, sdHit);

      let base = mix(U.colA.xyz, U.colB.xyz, somaMix);
      let key = normalize(-rd + vec3f(0.30, 0.62, 0.10));
      let dif = max(dot(n, key), 0.0);
      let bak = max(dot(n, -key), 0.0) * 0.28;
      let ndv = max(dot(n, -rd), 0.0);
      let fres = 0.035 + 0.965 * pow(1.0 - ndv, 5.0);
      let hv = normalize(key - rd);
      // Smoothness is the width of the highlight, on a log scale so the slider
      // feels even end to end. The constant is picked so that 0.5 lands on
      // exactly 48, the page's exponent: at the default this is a no-op.
      let gloss = exp2(1.0 + 9.169925 * U.matSmooth);
      let spec = pow(max(dot(n, hv), 0.0), gloss);
      // A metal has no diffuse of its own and tints what it reflects, so the
      // body colour moves out of the diffuse term and into the highlight.
      let specCol = mix(U.rimCol.xyz, base, U.matMetal);
      var aoLip = 1.0;
      let ao = clamp(mapLip(p + n*0.05, &dmy, &aoLip) * aoLip * sceneLip() / 0.05, 0.0, 1.0) * 0.72 + 0.28;
      let sp = spikeAt(p);

      let body = 1.0 - 0.85 * U.matMetal;   // a metal keeps almost none of its diffuse
      col = base * (0.05 + 0.62*dif) * ao * body;
      col = col + U.spikeCol.xyz * vec3f(0.88, 0.84, 1.0) * bak * 0.55 * body;
      col = col + U.spikeCol.xyz * sp * (0.70 + 2.2*somaMix) * U.spike;
      col = col + U.colB.xyz * somaMix * somaMix * 0.42 * body;
      col = col + U.colA.xyz * 0.06;
      col = col + specCol * fres * (0.62 + 0.80*ao) * (0.85 + 0.55*somaMix);
      col = col + specCol * spec * (0.45 + 3.2*fres) * (1.0 + 2.0*U.matMetal);

      let fog = 1.0 - exp(-select(0.52, 0.22, U.scene > 0.5) * max(t - tNear, 0.0));
      col = mix(col, bgCol(rd) * 1.15, fog * 0.78 * U.postFog);
    }
  }

  col = col + (U.colA.xyz*0.60 + U.spikeCol.xyz*0.40) * glow * (0.55 + 0.70*U.spike) * U.postGlow;

  // exposure, then the tone map that folds the highlights back into range, then
  // the gamma the display expects
  col = col * U.postExposure;
  col = col / (vec3f(1.0) + col);
  col = pow(max(col, vec3f(0.0)), vec3f(0.4545));

  let sv = fc/U.res - vec2f(0.5);
  col = col * (1.0 - U.postVignette * dot(sv, sv));
  col = col + vec3f((hash33(vec3f(fc, U.time)).x - 0.5) * U.postGrain);

  return col;
}

// the full-screen triangle the page keeps in a vertex buffer: (-1,-1) (3,-1) (-1,3)
@vertex
fn vsMain(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
  let x = select(-1.0, 3.0, vi == 1u);
  let y = select(-1.0, 3.0, vi == 2u);
  return vec4f(x, y, 0.0, 1.0);
}

@fragment
fn fsMain(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  // NATIVE: gl_FragCoord counts up from the bottom, @builtin(position) down from
  // the top. Without this flip the whole image is upside down.
  let fc = vec2f(pos.x, U.res.y - pos.y);
  return vec4f(render(fc), 1.0);
}
