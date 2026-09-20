//! The camera: three automatic paths, free flight, and the six saved viewpoints.
//! Ported from index.html — same paths, same constants, same steering.

use crate::state::{Fav, State, SCENE_R};

#[derive(Clone, Copy, Debug)]
pub struct Cam {
    pub eye: [f32; 3],
    pub tgt: [f32; 3],
    pub roll: f32,
}

impl Default for Cam {
    fn default() -> Self {
        Cam { eye: [0.0, 0.0, 3.0], tgt: [0.0; 3], roll: 0.0 }
    }
}

/// Which movement keys are down. Held state, integrated once per frame.
#[derive(Clone, Copy, Default, Debug)]
pub struct Held {
    pub w: bool,
    pub a: bool,
    pub s: bool,
    pub d: bool,
    pub q: bool,
    pub e: bool,
    pub shift: bool,
}

impl Held {
    pub fn clear(&mut self) {
        *self = Held::default();
    }
}

/// xorshift, so a new viewpoint does not need a dependency
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x >> 40) as f32) / (1u32 << 24) as f32
    }
}

pub fn path_point(u: f32) -> [f32; 3] {
    [1.62 * u.sin(), 0.52 * (u * 0.73 + 1.1).sin(), 1.72 * (u * 0.61).cos()]
}

fn rotate_dir(d: [f32; 3], az: f32, el: f32) -> [f32; 3] {
    let (ca, sa) = (az.cos(), az.sin());
    let x = d[0] * ca + d[2] * sa;
    let z = -d[0] * sa + d[2] * ca;
    let len = (x * x + z * z).sqrt().max(1e-5);
    let (ce, se) = (el.cos(), el.sin());
    let y = d[1] * ce + len * se;
    let k = (len * ce - d[1] * se) / len;
    [x * k, y, z * k]
}

/// Yaw 0 looks down +Z; pitch stops just shy of the poles so the horizontal basis
/// never collapses.
pub fn fly_fwd(st: &State) -> [f32; 3] {
    let cp = st.fly_pitch.cos();
    [cp * st.fly_yaw.sin(), st.fly_pitch.sin(), cp * st.fly_yaw.cos()]
}

/// screen-right, matching the shader's `uu = normalize(cross(ww, worldUp))`,
/// flattened to the horizon so strafing never climbs
fn fly_right(st: &State) -> [f32; 3] {
    [-st.fly_yaw.cos(), 0.0, st.fly_yaw.sin()]
}

pub fn fly_look(st: &mut State, dx: f32, dy: f32) {
    st.fly_yaw -= dx * 0.0040;
    st.fly_pitch = (st.fly_pitch - dy * 0.0040).clamp(-1.553, 1.553);
}

pub fn set_fly_speed(st: &mut State, v: f32) {
    st.fly_speed = v.clamp(0.03, 12.0);
}

pub fn set_zoom(st: &mut State, z: f32) {
    st.zoom = z.clamp(0.42, 1.8);
}

/// put the camera at `pos` with `target` dead ahead
pub fn aim_at(st: &mut State, pos: [f32; 3], target: [f32; 3]) {
    let d = [target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
    st.fly_pos = pos;
    st.fly_pitch = (d[1] / l).clamp(-1.0, 1.0).asin();
    st.fly_yaw = (d[0] / l).atan2(d[2] / l);
}

pub fn fly_step(st: &mut State, dt: f32, held: &Held) {
    let mx = (held.d as i32 - held.a as i32) as f32; // strafe
    let mz = (held.w as i32 - held.s as i32) as f32; // forward
    let my = (held.e as i32 - held.q as i32) as f32; // world up / down
    if mx == 0.0 && mz == 0.0 && my == 0.0 {
        return;
    }
    let f = fly_fwd(st);
    let r = fly_right(st);
    let v = st.fly_speed * if held.shift { 3.5 } else { 1.0 } * dt;
    st.fly_pos[0] += (f[0] * mz + r[0] * mx) * v;
    st.fly_pos[1] += (f[1] * mz + my) * v;
    st.fly_pos[2] += (f[2] * mz + r[2] * mx) * v;
}

pub fn camera(st: &State, t: f32) -> Cam {
    if st.fly {
        let eye = st.fly_pos;
        let f = fly_fwd(st);
        return Cam { eye, tgt: [eye[0] + f[0], eye[1] + f[1], eye[2] + f[2]], roll: 0.0 };
    }
    if st.mode == 1 {
        let u = t * 0.085;
        let eye = path_point(u);
        let ah = path_point(u + 0.07);
        let d =
            rotate_dir([ah[0] - eye[0], ah[1] - eye[1], ah[2] - eye[2]], st.drag_az, st.drag_el);
        return Cam {
            eye,
            tgt: [eye[0] + d[0], eye[1] + d[1], eye[2] + d[2]],
            roll: 0.10 * (t * 0.21).sin(),
        };
    }
    let (az, el, r, tgt, roll);
    if st.mode == 2 {
        az = 0.9 + t * 0.035 + st.drag_az;
        el = -0.10 + 0.09 * (t * 0.09).sin() + st.drag_el;
        r = (2.02 + 0.14 * (t * 0.05).sin()) * st.zoom * st.fit;
        tgt = [0.22 * (t * 0.06).sin(), 0.10 * (t * 0.043).sin() - 0.05, 0.18 * (t * 0.05).cos()];
        roll = 0.03 * (t * 0.07).sin();
    } else {
        az = t * 0.10 + st.drag_az;
        el = 0.20 + 0.16 * (t * 0.13).sin() + st.drag_el;
        r = (3.05 + 0.30 * (t * 0.071).sin()) * st.zoom * st.fit;
        tgt = [0.0, 0.03 * (t * 0.2).sin(), 0.0];
        roll = 0.04 * (t * 0.11).sin();
    }
    let el = el.clamp(-1.35, 1.35);
    Cam {
        eye: [
            tgt[0] + r * el.cos() * az.sin(),
            tgt[1] + r * el.sin(),
            tgt[2] + r * el.cos() * az.cos(),
        ],
        tgt,
        roll,
    }
}

/// `R`. Always frames the object rather than empty space, and stays close enough
/// that the object overruns the frame.
pub fn random_view(st: &mut State, cam: &Cam, rng: &mut Rng) {
    let r = SCENE_R[st.scene];
    if st.fly {
        // pick a point near the object's middle, then step back along the line of
        // sight to it, so the camera is always facing the object and never space
        let tp = [
            (rng.next_f32() * 2.0 - 1.0) * r * 0.28,
            (rng.next_f32() * 2.0 - 1.0) * r * 0.16,
            (rng.next_f32() * 2.0 - 1.0) * r * 0.28,
        ];
        let yaw = rng.next_f32() * 6.2832;
        let el = (rng.next_f32() * 2.0 - 1.0) * 0.6;
        let d = r * (0.50 + rng.next_f32() * 0.55);
        let ce = el.cos();
        let mut pos =
            [tp[0] - d * ce * yaw.sin(), tp[1] - d * el.sin(), tp[2] - d * ce * yaw.cos()];
        // the chrome slab is thin, so a low camera lands inside its relief
        if st.scene == 2 && pos[1].abs() < 0.45 {
            pos[1] = if pos[1] < 0.0 { -0.45 } else { 0.45 };
        }
        aim_at(st, pos, tp);
    } else if st.mode == 1 {
        // the path decides where the camera is, so only the aim is left: the
        // steering offsets are the yaw and pitch that swing the path's own tangent
        // onto a point near the middle of the object
        st.clock = rng.next_f32() * 400.0;
        let u = st.clock * 0.085;
        let e0 = path_point(u);
        let a0 = path_point(u + 0.07);
        let tv = [a0[0] - e0[0], a0[1] - e0[1], a0[2] - e0[2]];
        let tp1 = [
            (rng.next_f32() * 2.0 - 1.0) * r * 0.25,
            (rng.next_f32() * 2.0 - 1.0) * r * 0.15,
            (rng.next_f32() * 2.0 - 1.0) * r * 0.25,
        ];
        let wv = [tp1[0] - e0[0], tp1[1] - e0[1], tp1[2] - e0[2]];
        st.drag_az = wv[0].atan2(wv[2]) - tv[0].atan2(tv[2]);
        st.drag_el = (wv[1].atan2((wv[0] * wv[0] + wv[2] * wv[2]).sqrt())
            - tv[1].atan2((tv[0] * tv[0] + tv[2] * tv[2]).sqrt()))
        .clamp(-0.9, 0.9);
    } else {
        // orbit and drift are aimed at the centre by construction, so only the
        // angle and the range are left to pick
        st.drag_az = rng.next_f32() * 6.2832;
        st.drag_el = (rng.next_f32() - 0.5) * 1.6;
        st.zoom = 0.42 + rng.next_f32() * 0.30;
    }
    let _ = cam;
    st.vel_az = 0.0;
    st.vel_el = 0.0;
}

/// hand the camera over exactly where the auto path left it
pub fn set_fly(st: &mut State, on: bool, cam: &Cam, held: &mut Held) {
    if on && !st.fly {
        aim_at(st, cam.eye, cam.tgt);
    }
    st.fly = on;
    held.clear();
    st.vel_az = 0.0;
    st.vel_el = 0.0;
}

pub fn fav_store(st: &mut State, i: usize) {
    st.favs[i] = Some(Fav {
        fly: st.fly,
        pos: st.fly_pos,
        yaw: st.fly_yaw,
        pitch: st.fly_pitch,
        mode: st.mode,
        clock: st.clock,
        az: st.drag_az,
        el: st.drag_el,
        zoom: st.zoom,
    });
}

/// Returns false when the slot is empty.
pub fn fav_recall(st: &mut State, i: usize, cam: &Cam, held: &mut Held) -> bool {
    let Some(f) = st.favs[i] else { return false };
    // switch modes first: turning flight on aims the camera where the auto path
    // left it, which would overwrite the position being restored
    if f.fly != st.fly {
        set_fly(st, f.fly, cam, held);
    }
    st.fly_pos = f.pos;
    st.fly_yaw = f.yaw;
    st.fly_pitch = f.pitch;
    st.clock = f.clock;
    st.zoom = f.zoom;
    st.drag_az = f.az;
    st.drag_el = f.el;
    st.vel_az = 0.0;
    st.vel_el = 0.0;
    st.mode = f.mode;
    true
}
