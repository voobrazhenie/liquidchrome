//! The deterministic benchmark, per REQUIREMENTS.md §11.
//!
//! A run has to mean the same thing on every machine, so nothing about it is
//! allowed to depend on how fast the machine is:
//!   * the camera follows a fixed path, never the user's view or saved slots
//!   * the animation clocks step by a fixed 1/60 s PER FRAME, not by elapsed time,
//!     so frame N shows the same geometry everywhere
//!   * the march budget is frozen at what the settings ask for, and the adaptive
//!     resolution and step controllers are switched off for the run
//! The only thing measured is how long each of those identical frames took. Same
//! four shots, same seeds and same frame counts as the page, so a native number is
//! comparable with a browser one.

use crate::camera::Cam;
use crate::state::{Backend, Present, State};
use crate::state::{AA_NAME, BND_NAME, OBJ_NAME, RES_NAME, SCENE_R, WARP_NAME};
use std::time::Instant;

pub const VERSION: u32 = 1;
pub const DT: f32 = 1.0 / 60.0; // clock step per frame
pub const WARM_MS: f32 = 1500.0; // discarded: shader caches, GPU clocks ramping
pub const FRAMES: usize = 150; // per shot
pub const CAP_MS: f32 = 45000.0; // a machine too slow to finish reports partial
pub const C0: f32 = 12.0;
pub const M0: f32 = 40.0;
pub const P0: f32 = 6.0;

pub enum Path {
    Line { a: [f32; 3], b: [f32; 3] },
    Orbit { r: f32, y: f32, a0: f32, a1: f32 },
}

pub struct Shot {
    pub name: &'static str,
    pub path: Path,
    pub t: [f32; 3],
}

/// Every position is in units of the object's own radius, and every shot passes
/// above it looking down at the middle.
pub const SHOTS: [Shot; 4] = [
    Shot {
        name: "Pass",
        path: Path::Line { a: [-1.65, 0.55, 0.10], b: [1.65, 0.55, -0.10] },
        t: [0.0, 0.0, 0.0],
    },
    Shot {
        name: "Diagonal",
        path: Path::Line { a: [-1.30, 0.95, -1.30], b: [1.30, 0.35, 1.30] },
        t: [0.0, 0.05, 0.0],
    },
    Shot {
        name: "Orbit",
        path: Path::Orbit { r: 1.05, y: 0.45, a0: 0.60, a1: 2.70 },
        t: [0.0, 0.0, 0.0],
    },
    Shot {
        name: "Graze",
        path: Path::Line { a: [0.15, 0.30, -1.75], b: [-0.10, 0.22, 1.15] },
        t: [0.0, 0.05, 0.0],
    },
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Warm,
    Run,
}

pub struct Bench {
    pub on: bool,
    phase: Phase,
    idx: usize,
    prev_shot: usize,
    t0: Instant,
    ms: Vec<f32>,
    shot_ms: Vec<Vec<f32>>,
    /// whatever the settings asked for when the run started, held for the whole run
    pub steps: f32,
    save: (f32, f32, f32),
    /// the present mode in force before the run, so it can be put back
    pub present_was: Option<Present>,
}

impl Default for Bench {
    fn default() -> Self {
        Bench {
            on: false,
            phase: Phase::Warm,
            idx: 0,
            prev_shot: 0,
            t0: Instant::now(),
            ms: Vec::new(),
            shot_ms: Vec::new(),
            steps: 0.0,
            save: (0.0, 0.0, 0.0),
            present_was: None,
        }
    }
}

pub struct Report {
    pub text: String,
    pub avg: f32,
    pub complete: bool,
    pub enough: bool,
}

fn pose(sh: &Shot, u: f32, r: f32) -> Cam {
    let e = match sh.path {
        Path::Orbit { r: rr, y, a0, a1 } => {
            let a = a0 + (a1 - a0) * u;
            [a.sin() * rr, y, a.cos() * rr]
        }
        Path::Line { a, b } => {
            [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u, a[2] + (b[2] - a[2]) * u]
        }
    };
    Cam {
        eye: [e[0] * r, e[1] * r, e[2] * r],
        tgt: [sh.t[0] * r, sh.t[1] * r, sh.t[2] * r],
        roll: 0.0,
    }
}

pub struct RunInfo<'a> {
    pub rw: u32,
    pub rh: u32,
    /// what the march was rebuilt to: the resolution the console asked for
    pub bw: u32,
    pub bh: u32,
    pub win_w: u32,
    pub win_h: u32,
    pub scale_factor: f64,
    pub adapter: &'a str,
    pub driver: &'a str,
    pub backend: Backend,
    pub present: Present,
    pub post: &'a str,
}

impl Bench {
    pub fn start(&mut self, st: &State, steps: f32) {
        if self.on {
            return;
        }
        self.on = true;
        self.phase = Phase::Warm;
        self.idx = 0;
        self.prev_shot = 0;
        self.ms.clear();
        self.shot_ms = SHOTS.iter().map(|_| Vec::new()).collect();
        self.steps = steps;
        self.save = (st.clock, st.m_clock, st.p_clock);
        self.t0 = Instant::now();
    }

    fn clocks(&self, st: &mut State, i: usize) {
        let g = i as f32 * DT;
        st.clock = C0 + g;
        st.m_clock = M0 + g;
        st.p_clock = P0 + g;
    }

    /// One frame of the run. `raw_ms` is the cost of the frame drawn last, so it
    /// belongs to the pose that was set last. Returns true when the run just ended.
    pub fn step(&mut self, now: Instant, raw_ms: f32, st: &mut State, cam: &mut Cam) -> bool {
        let r = SCENE_R[st.scene];
        if self.phase == Phase::Warm {
            *cam = pose(&SHOTS[0], 0.0, r);
            self.clocks(st, 0);
            if (now - self.t0).as_secs_f32() * 1000.0 >= WARM_MS {
                self.phase = Phase::Run;
                self.t0 = now;
                self.idx = 0;
            }
            return false;
        }
        if self.idx > 0 {
            self.ms.push(raw_ms);
            self.shot_ms[self.prev_shot].push(raw_ms);
        }
        let total = SHOTS.len() * FRAMES;
        if self.idx >= total || (now - self.t0).as_secs_f32() * 1000.0 > CAP_MS {
            self.on = false;
            st.clock = self.save.0;
            st.m_clock = self.save.1;
            st.p_clock = self.save.2;
            return true;
        }
        let shot = self.idx / FRAMES;
        *cam = pose(&SHOTS[shot], (self.idx % FRAMES) as f32 / (FRAMES - 1) as f32, r);
        self.clocks(st, self.idx);
        self.prev_shot = shot;
        self.idx += 1;
        false
    }

    /// Stop where it stands; whatever has been measured is still reported.
    pub fn stop(&mut self, st: &mut State) {
        if !self.on {
            return;
        }
        self.on = false;
        st.clock = self.save.0;
        st.m_clock = self.save.1;
        st.p_clock = self.save.2;
    }

    pub fn complete(&self) -> bool {
        self.idx >= SHOTS.len() * FRAMES
    }

    pub fn percent(&self) -> u32 {
        let total = (SHOTS.len() * FRAMES) as f32;
        ((100.0 * self.idx as f32 / total) as u32).min(100)
    }

    pub fn progress_text(&self) -> String {
        format!(
            "Shot {} of {}  \u{b7}  {}\n{} of {} frames\n\nHold still \u{2014} anything that steals the GPU\nwhile this runs lands in the numbers.",
            self.prev_shot + 1,
            SHOTS.len(),
            SHOTS[self.prev_shot].name,
            self.idx,
            SHOTS.len() * FRAMES
        )
    }

    pub fn report(&self, st: &State, info: &RunInfo) -> Report {
        let ms = &self.ms;
        if ms.len() < 20 {
            return Report {
                text: "Stopped before there was anything worth reporting.".into(),
                avg: 0.0,
                complete: false,
                enough: false,
            };
        }
        let sum: f32 = ms.iter().sum();
        let mut sorted = ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f32| -> f32 {
            let i = (p * (sorted.len() - 1) as f32).round() as usize;
            sorted[i.min(sorted.len() - 1)]
        };
        let fps = |m: f32| if m > 0.0 { 1000.0 / m } else { 0.0 };
        let avg = fps(sum / ms.len() as f32);
        let med = fps(pct(0.50));
        let low = fps(pct(0.99)); // the slowest 1% of frames
        let best = fps(pct(0.01));
        let complete = self.complete();

        let shots: Vec<String> = self
            .shot_ms
            .iter()
            .enumerate()
            .map(|(k, a)| {
                let t: f32 = a.iter().sum();
                format!(
                    "{} {}",
                    SHOTS[k].name,
                    if a.is_empty() {
                        "\u{2014}".to_string()
                    } else {
                        format!("{:.1}", fps(t / a.len() as f32))
                    }
                )
            })
            .collect();

        // A display that caps the frame rate measures the display, not the render.
        // Steady frames sitting on a common refresh rate is what that looks like.
        let caps = [30.0, 50.0, 60.0, 75.0, 90.0, 100.0, 120.0, 144.0, 165.0, 240.0];
        let vsync = caps.iter().any(|c: &f32| (med - c).abs() / c < 0.03 && low > avg * 0.85);

        let warps: Vec<&str> = (0..3).filter(|w| st.warp_on[*w]).map(|w| WARP_NAME[w]).collect();

        let mut l: Vec<String> = Vec::new();
        l.push(format!("Cortical Flythrough \u{b7} native \u{b7} benchmark v{VERSION}"));
        l.push(String::new());
        l.push(format!("Backend    {} \u{b7} {}", info.backend.label(), info.present.label()));
        l.push(format!("Object     {}", OBJ_NAME[st.scene]));
        // marched, then what it was rebuilt to when the upscaler is in the way —
        // two numbers matter now that Upscale divides the chosen resolution
        let size = if (info.rw, info.rh) == (info.bw, info.bh) {
            format!("{}\u{d7}{}", info.rw, info.rh)
        } else {
            format!("{}\u{d7}{} \u{2192} {}\u{d7}{}", info.rw, info.rh, info.bw, info.bh)
        };
        l.push(format!(
            "Render     {} \u{b7} {} \u{b7} AA {} \u{b7} post {}",
            RES_NAME[st.res_pin], size, AA_NAME[st.aa], info.post
        ));
        l.push(format!(
            "March      {} steps \u{b7} precision {:.1} \u{b7} relax {:.2}",
            self.steps as u32, st.eps, st.omega
        ));
        l.push(format!("Bounds     {} + {:.2}", BND_NAME[st.bound], st.bound_pad));
        l.push(format!(
            "Warp       {} \u{b7} {} \u{b7} {:.2} @ {:.2}",
            if warps.is_empty() { "none".to_string() } else { warps.join("+") },
            if st.warp_mode == 1 { "object" } else { "space" },
            st.warp_amt,
            st.warp_freq
        ));
        match st.scene {
            2 => l.push(format!(
                "Chrome     relief {:.2} \u{b7} scale {:.2} \u{b7} thick {:.3} \u{b7} twirl {:.2}",
                st.lc_relief, st.lc_freq, st.lc_thick, st.lc_twirl
            )),
            0 => l.push(format!("Brain      density {:.1}", st.density)),
            _ => l.push(format!("Neuron     line width {:.3}", st.thick)),
        }
        l.push(String::new());
        l.push(format!("avg        {:.1} fps   ({:.2} ms)", avg, sum / ms.len() as f32));
        l.push(format!("median     {med:.1} fps"));
        l.push(format!("1% low     {low:.1} fps"));
        l.push(format!("best 1%    {best:.1} fps"));
        l.push(format!(
            "frames     {} in {:.2} s{}",
            ms.len(),
            sum / 1000.0,
            if complete { "" } else { "  \u{2014} PARTIAL" }
        ));
        if !complete {
            l.push(format!(
                "           cut off at the {} s limit, so it covers only the start of the path. Not comparable with a complete run.",
                CAP_MS / 1000.0
            ));
        }
        l.push(format!("shots      {}", shots.join(" \u{b7} ")));
        if vsync {
            l.push(format!(
                "note       steady at {med:.0} fps \u{2014} the display is the limit here, not the render. Set the present mode to Uncapped, or raise the resolution."
            ));
        }
        l.push(String::new());
        l.push(format!("Adapter    {}", info.adapter));
        l.push(format!("Driver     {}", info.driver));
        l.push(format!(
            "Window     {}\u{d7}{} @ scale {:.2}{}",
            info.win_w,
            info.win_h,
            info.scale_factor,
            if st.fullscreen { " \u{b7} full screen" } else { "" }
        ));
        l.push(format!(
            "App        cortical-flythrough {} \u{b7} {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        ));

        Report { text: l.join("\n"), avg, complete, enough: true }
    }
}
