//! Window, input and the frame loop.

use crate::bench::{Bench, RunInfo};
use crate::camera::{self, Cam, Held, Rng};
use crate::gfx::{EguiFrame, Frame, Gfx, PostMode};
use crate::midi::{Midi, Out};
use crate::state::{Backend, Present, Sizes, State, FHD_H, FHD_W, STEPS_MAX};
use crate::store::{self, Template};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

/// Every frame time of the last five seconds, in a ring so nothing is reallocated
/// or shifted per frame. 1024 slots covers five seconds at any rate a display can
/// actually show.
pub const HN: usize = 1024;

pub struct History {
    pub ms: [f32; HN],
    pub at: [f64; HN],
    pub head: usize,
    pub len: usize,
}

impl Default for History {
    fn default() -> Self {
        History { ms: [0.0; HN], at: [0.0; HN], head: 0, len: 0 }
    }
}

impl History {
    pub fn push(&mut self, now: f64, ms: f32) {
        self.ms[self.head] = ms;
        self.at[self.head] = now;
        self.head = (self.head + 1) % HN;
        if self.len < HN {
            self.len += 1;
        }
    }

    /// frames in the window over the time they took — a true mean rate, not the mean
    /// of per-frame rates, which a single stall would skew
    pub fn mean_fps(&self, now: f64, window_ms: f64) -> f32 {
        let (mut count, mut span) = (0.0f32, 0.0f32);
        for i in 1..=self.len {
            let k = (self.head + HN - i) % HN;
            if now - self.at[k] > window_ms {
                break;
            }
            span += self.ms[k];
            count += 1.0;
        }
        if span > 0.0 {
            count * 1000.0 / span
        } else {
            0.0
        }
    }
}

/// `--import <file>`: a settings export from the web page, in any of the shapes
/// the page saves it in.
fn import_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "--import")?;
    args.get(i + 1).cloned()
}

/// Bring a device up without letting a driver that dies on the way take the
/// session with it: a panic here costs the switch, not the app.
fn try_gfx(
    window: Arc<Window>,
    backend: Backend,
    present: crate::state::Present,
) -> Result<Gfx, String> {
    let attempt = std::panic::AssertUnwindSafe(|| Gfx::new(window, backend, present));
    match std::panic::catch_unwind(attempt) {
        Ok(r) => r,
        Err(_) => Err(format!("{} crashed while starting up", backend.label())),
    }
}

pub struct ReportView {
    pub title: String,
    pub big: String,
    pub unit: String,
    pub body: String,
    pub text: String,
    pub can_copy: bool,
}

pub struct App {
    pub(crate) window: Option<Arc<Window>>,
    pub(crate) gfx: Option<Gfx>,
    pub(crate) st: State,
    pub(crate) builtin: State,
    pub(crate) cam: Cam,
    pub(crate) held: Held,
    pub(crate) rng: Rng,

    pub(crate) egui_ctx: egui::Context,
    pub(crate) egui_state: Option<egui_winit::State>,

    pub(crate) start: Instant,
    pub(crate) last: Instant,
    pub(crate) ema: f32,
    pub(crate) frames: u32,
    pub(crate) hist: History,
    pub(crate) warm: i32,
    pub(crate) march_cap: f32,

    /// what the scene was marched at last frame
    pub(crate) rw: u32,
    pub(crate) rh: u32,
    /// and what that was reconstructed to: the resolution the console asked for
    pub(crate) bw: u32,
    pub(crate) bh: u32,

    pub(crate) hud: bool,
    pub(crate) toast: Option<(String, Instant)>,
    pub(crate) report: Option<ReportView>,
    pub(crate) bench: Bench,
    pub(crate) midi: Midi,
    pub(crate) templates: Vec<Template>,
    pub(crate) tpl_name: String,
    pub(crate) note: String,
    pub(crate) gfx_note: String,

    pub(crate) dragging: bool,
    pub(crate) looking: bool,
    pub(crate) mods: ModifiersState,
    pub(crate) want_backend: Option<Backend>,
    /// the ceiling the fps graph is currently drawn against
    pub(crate) graph_top: usize,
    pub(crate) quit: bool,
}

impl App {
    pub fn new(backend_arg: Option<Backend>) -> App {
        let builtin = State::default();
        let mut note;
        let mut st;
        if let Some(saved) = store::load_settings() {
            st = saved;
            note = format!(
                "Loaded {}",
                store::settings_path().map(|p| p.display().to_string()).unwrap_or_default()
            );
        } else {
            // Nothing saved here yet, so open on the page's own settings — same
            // object, same framing, same six views as the browser was left on.
            st = store::web_default();
            note = "Opened on the settings the web page was left on".to_string();
        }
        // `--import <file>` brings over a newer export from the page
        if let Some(path) = import_arg() {
            match store::import_web_file(&path) {
                Ok(imported) => {
                    st = imported;
                    note = format!("Imported {path}");
                }
                Err(e) => note = format!("Could not import {path}: {e}"),
            }
        }
        if let Some(b) = backend_arg {
            st.backend = b;
        }
        if !st.backend.available() {
            st.backend = crate::state::default_backend();
        }
        // the render scale is measured, not saved, so a restored Half or Native has
        // to be put back into it or the first frames come up at the wrong size
        st.scale_q = match st.res_pin {
            2 | 3 => 1.0,
            _ => 0.5,
        };
        let templates = store::load_templates();
        App {
            window: None,
            gfx: None,
            st,
            builtin,
            cam: Cam::default(),
            held: Held::default(),
            rng: Rng::new(0x2FD9C0 ^ store::now_ms()),
            egui_ctx: egui::Context::default(),
            egui_state: None,
            start: Instant::now(),
            last: Instant::now(),
            ema: 16.7,
            frames: 0,
            hist: History::default(),
            // The opening frames are capped hard. Nothing has been measured yet, and
            // the first frame also carries the shader build.
            warm: 5,
            march_cap: STEPS_MAX,
            rw: 0,
            rh: 0,
            bw: 0,
            bh: 0,
            hud: true,
            toast: None,
            report: None,
            bench: Bench::default(),
            midi: Midi::new(store::load_midi()),
            templates,
            tpl_name: String::new(),
            note,
            gfx_note: String::new(),
            dragging: false,
            looking: false,
            mods: ModifiersState::empty(),
            want_backend: None,
            graph_top: 0,
            quit: false,
        }
    }

    pub(crate) fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn make_window(
        &self,
        el: &ActiveEventLoop,
        size: Option<winit::dpi::PhysicalSize<u32>>,
    ) -> Option<Arc<Window>> {
        let mut attrs = Window::default_attributes().with_title("Cortical Flythrough");
        attrs = match size {
            Some(s) if s.width > 0 && s.height > 0 => attrs.with_inner_size(s),
            _ => attrs.with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0)),
        };
        match el.create_window(attrs) {
            Ok(w) => Some(Arc::new(w)),
            Err(e) => {
                log::error!("no window: {e}");
                None
            }
        }
    }

    /// Bring up a device on `backend`, and the console with it.
    ///
    /// **A window can only ever be OpenGL's once.** Windows lets a window's pixel
    /// format be set a single time, and OpenGL sets one; handing that same window
    /// to Vulkan or DirectX afterwards — or to a second OpenGL context — is what
    /// took the driver down. So any switch with OpenGL on either side gets a fresh
    /// window before the device is even asked for, and the old one is dropped.
    ///
    /// Creation is also caught: a driver that panics on the way up costs the
    /// switch, not the session.
    fn build_gfx(&mut self, el: &ActiveEventLoop, backend: Backend) {
        let Some(window) = self.window.clone() else { return };
        let size = window.inner_size();
        let was_fullscreen = self.st.fullscreen;
        let leaving = self.gfx.as_ref().map(|g| g.backend);
        self.gfx = None; // the old device must let go of the window first

        let mut window = window;
        let mut tried_fresh = false;
        if leaving.is_some() && (leaving == Some(Backend::Gl) || backend == Backend::Gl) {
            if let Some(fresh) = self.make_window(el, Some(size)) {
                self.window = Some(fresh.clone());
                window = fresh;
                tried_fresh = true;
            }
        }

        let mut err = match try_gfx(window.clone(), backend, self.st.present) {
            Ok(g) => {
                self.adopt(g, backend, window, was_fullscreen);
                return;
            }
            Err(e) => e,
        };

        // it may still be the window's fault — give it one clean one
        if !tried_fresh {
            if let Some(fresh) = self.make_window(el, Some(size)) {
                self.window = Some(fresh.clone());
                window = fresh;
                match try_gfx(window.clone(), backend, self.st.present) {
                    Ok(g) => {
                        self.adopt(g, backend, window, was_fullscreen);
                        return;
                    }
                    Err(e) => err = e,
                }
            }
        }

        // whatever is left that this machine will actually run
        for b in Backend::ALL {
            if b == backend || !b.available() {
                continue;
            }
            if let Ok(g) = try_gfx(window.clone(), b, self.st.present) {
                self.adopt(g, b, window.clone(), was_fullscreen);
                self.gfx_note = format!("{} \u{2014} using {} instead", err, b.label());
                self.toast(format!("{} is not available here", backend.label()));
                return;
            }
        }
        self.gfx_note = err.clone();
        self.toast(err);
    }

    /// Take on a freshly built device: rebuild the console against it, and let a
    /// software adapter bring the march budget down before the first frame.
    fn adopt(&mut self, g: Gfx, backend: Backend, window: Arc<Window>, fullscreen: bool) {
        self.adopt_inner(g, backend, window);
        // a new window opens windowed, whatever the old one was doing
        if fullscreen {
            self.set_fullscreen(true);
        }
    }

    fn adopt_inner(&mut self, mut g: Gfx, backend: Backend, window: Arc<Window>) {
        // The console's renderer lives inside Gfx, and egui hands over its font
        // atlas exactly once — so a new device needs a new context to be given one.
        self.egui_ctx = egui::Context::default();
        crate::ui::style(&self.egui_ctx);
        self.egui_state = Some(egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        ));
        self.gfx_note.clear();
        if !g.scene_pipe(self.st.scene) {
            self.gfx_note = format!("{} would not build this object", backend.label());
        }
        if g.is_software {
            // no GPU behind this adapter: the march has to come down or the first
            // frame takes minutes
            self.march_cap = 96.0;
            self.st.steps = self.st.steps.min(56.0);
            self.st.steps_pin = false;
            self.gfx_note =
                format!("Software renderer ({}) \u{2014} this will be slow", g.adapter_name);
        } else {
            self.march_cap = STEPS_MAX;
        }
        self.st.backend = backend;
        // the title says which API is driving, so a screenshot of a run carries it
        window.set_title(&format!("Cortical Flythrough \u{2014} {}", backend.label()));
        self.warm = 5;
        self.rw = 0;
        self.rh = 0;
        self.bw = 0;
        self.bh = 0;
        self.gfx = Some(g);
    }

    /// The sizes this frame is drawn at. The swapchain is always the window; the
    /// post pass fits base to it, which is what the browser's canvas scaling did on
    /// the page. The arithmetic itself lives in `state::frame_size` so it can be
    /// tested without a window.
    fn frame_sizes(&mut self, win_w: u32, win_h: u32) -> Sizes {
        let s = crate::state::frame_size((win_w, win_h), &self.st, self.warm > 0, self.march_cap);
        self.st.fit = s.fit;
        s
    }

    pub(crate) fn set_fullscreen(&mut self, on: bool) {
        let Some(win) = &self.window else { return };
        self.st.fullscreen = on;
        if !on {
            win.set_fullscreen(None);
            return;
        }
        let mode = if self.st.exclusive {
            win.current_monitor().and_then(|m| {
                m.video_modes()
                    .max_by_key(|v| (v.size().width * v.size().height, v.refresh_rate_millihertz()))
            })
        } else {
            None
        };
        match mode {
            Some(m) => win.set_fullscreen(Some(Fullscreen::Exclusive(m))),
            None => win.set_fullscreen(Some(Fullscreen::Borderless(None))),
        }
    }

    pub(crate) fn apply_present(&mut self, p: Present) {
        self.st.present = p;
        if let Some(g) = &mut self.gfx {
            g.set_present(p);
        }
    }

    pub(crate) fn start_bench(&mut self) {
        let steps = self.st.march_steps(self.march_cap);
        // uncapped for the run where the driver has it: a frame time sitting on the
        // refresh rate measures the display, not the render
        if self.st.present != Present::Immediate {
            if let Some(g) = &self.gfx {
                if g.present_honoured(Present::Immediate) {
                    self.bench.present_was = Some(self.st.present);
                    let p = Present::Immediate;
                    self.apply_present(p);
                }
            }
        }
        self.bench.start(&self.st, steps);
        self.report = Some(ReportView {
            title: "Benchmark \u{b7} running".into(),
            big: "0".into(),
            unit: "% done".into(),
            body: self.bench.progress_text(),
            text: String::new(),
            can_copy: false,
        });
    }

    pub(crate) fn stop_bench(&mut self) {
        if !self.bench.on {
            return;
        }
        self.bench.stop(&mut self.st);
        self.finish_bench();
    }

    fn finish_bench(&mut self) {
        // the run may have switched to uncapped presentation; the report has to name
        // what it ran with, not what it puts back
        let ran_with = self.st.present;
        if let Some(p) = self.bench.present_was.take() {
            self.apply_present(p);
        }
        let (adapter, driver, post) = match &self.gfx {
            Some(g) => (g.adapter_name.clone(), g.driver.clone(), g.post_path),
            None => (String::new(), String::new(), ""),
        };
        let (win_w, win_h, scale) = match &self.window {
            Some(w) => {
                let s = w.inner_size();
                (s.width, s.height, w.scale_factor())
            }
            None => (0, 0, 1.0),
        };
        let info = RunInfo {
            rw: self.rw,
            rh: self.rh,
            bw: self.bw,
            bh: self.bh,
            win_w,
            win_h,
            scale_factor: scale,
            adapter: &adapter,
            driver: &driver,
            backend: self.st.backend,
            present: ran_with,
            post,
        };
        let rep = self.bench.report(&self.st, &info);
        self.report = Some(if rep.enough {
            ReportView {
                title: format!(
                    "Benchmark \u{b7} {}",
                    if rep.complete { "done" } else { "partial" }
                ),
                big: format!("{:.1}", rep.avg),
                unit: "avg fps".into(),
                body: rep.text.clone(),
                text: rep.text,
                can_copy: true,
            }
        } else {
            ReportView {
                title: "Benchmark \u{b7} stopped".into(),
                big: "\u{2014}".into(),
                unit: String::new(),
                body: rep.text,
                text: String::new(),
                can_copy: false,
            }
        });
    }

    fn draw(&mut self) {
        let Some(window) = self.window.clone() else { return };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        let now = Instant::now();
        let raw = (now - self.last).as_secs_f32() * 1000.0; // true cost of the last frame
        let dt = (raw / 1000.0).min(0.05); // clamped, only for the clocks
        self.last = now;
        self.hist.push((now - self.start).as_secs_f64() * 1000.0, raw);

        // The controller has its say before the camera is built, so a knob on the
        // zoom or the steering is in this frame rather than the next one. During a
        // run it is drained and dropped: the same frames have to come out on every
        // machine, and a knob moved half way through would measure two pictures.
        let acts = self.midi.poll(dt, self.bench.on);
        for a in acts {
            self.midi_act(a);
        }
        if self.midi.dirty {
            self.midi.dirty = false;
            if let Err(e) = store::save_midi(&self.midi.maps) {
                self.note = format!("Could not save the mappings: {e}");
            }
        }

        // a run drives the camera and the clocks itself, and may hand control back
        // mid-frame when it finishes
        if self.bench.on {
            let done = self.bench.step(now, raw, &mut self.st, &mut self.cam);
            if done {
                self.finish_bench();
            } else if let Some(r) = &mut self.report {
                let pc = self.bench.percent().to_string();
                if r.big != pc {
                    r.big = pc;
                    r.body = self.bench.progress_text();
                }
            }
        }
        if !self.bench.on {
            if self.st.running {
                self.st.clock += dt;
            }
            if self.st.morph {
                self.st.m_clock += dt;
            }
            if self.st.running || self.st.morph {
                self.st.p_clock += dt;
            }
            self.st.drag_az += self.st.vel_az;
            self.st.drag_el += self.st.vel_el;
            self.st.vel_az *= 0.90;
            self.st.vel_el *= 0.90;
            self.st.drag_el = self.st.drag_el.clamp(-1.0, 1.0);
            // the clocks clamp dt hard to stay stable; flight tolerates a longer
            // frame than that before it starts under-travelling
            if self.st.fly {
                camera::fly_step(&mut self.st, (raw / 1000.0).min(0.10), &self.held);
            }
            self.cam = camera::camera(&self.st, self.st.clock);
        }

        let sizes = self.frame_sizes(size.width, size.height);
        let (rw, rh) = sizes.marched;
        self.rw = rw;
        self.rh = rh;
        self.bw = sizes.base.0;
        self.bh = sizes.base.1;

        // ---- the console ----
        let egui_ctx = self.egui_ctx.clone();
        let raw_input = match &mut self.egui_state {
            Some(s) => s.take_egui_input(&window),
            None => return,
        };
        let full = egui_ctx.run_ui(raw_input, |ui| {
            let ctx = ui.ctx().clone();
            self.ui(&ctx);
        });
        if let Some(s) = &mut self.egui_state {
            s.handle_platform_output(&window, full.platform_output);
        }
        let ppp = egui_ctx.pixels_per_point();
        let jobs = egui_ctx.tessellate(full.shapes, ppp);

        let steps =
            if self.bench.on { self.bench.steps } else { self.st.march_steps(self.march_cap) };
        let uniforms = crate::gfx::uniforms(&self.st, &self.cam, self.rw, self.rh, steps);

        if let Some(g) = &mut self.gfx {
            g.resize_surface(size.width, size.height);
            let scene = self.st.scene;
            g.scene_pipe(scene);
            // The upscaler takes the first pass when it is on; FXAA then runs on
            // what it reconstructed, rather than being switched off by it.
            let upscaling = sizes.upscaling;
            let fxaa = self.st.aa == 1;
            let post = if upscaling {
                PostMode::Upscale
            } else if fxaa {
                PostMode::Fxaa
            } else {
                PostMode::Resolve
            };
            let then_fxaa = fxaa && post != PostMode::Fxaa;
            let outcome = g.render(
                scene,
                &uniforms,
                rw,
                rh,
                sizes.base,
                post,
                then_fxaa,
                self.st.sharpen,
                EguiFrame { jobs, delta: full.textures_delta, pixels_per_point: ppp },
            );
            if outcome == Frame::Skipped {
                return;
            }
        }

        // snap up on a spike so one bad frame is enough to trigger a correction
        self.ema = if raw > self.ema * 2.0 { raw } else { self.ema * 0.90 + raw * 0.10 };
        self.frames += 1;
        // correct within a few frames when the frame time is over budget. Never
        // during a run: a controller that changes the resolution or the march budget
        // half way through would be measuring two different things.
        let trigger = if self.ema > 34.0 {
            3
        } else if self.ema < 13.0 {
            25
        } else {
            40
        };
        if !self.bench.on && self.frames >= trigger {
            if self.st.res_pin != 0 && self.ema < 250.0 {
                // a pinned resolution only gives way when the frame time gets untenable
            } else if self.ema > 22.0 && self.st.scale_q > 0.30 {
                // cost is quadratic in the scale, so aim straight at the budget
                let k = (17.0 / self.ema).sqrt();
                self.st.scale_q = (self.st.scale_q * k).min(self.st.scale_q - 0.04).max(0.30);
            } else if self.ema < 13.0 && self.st.scale_q < 1.0 {
                self.st.scale_q =
                    (self.st.scale_q * (15.0 / self.ema.max(1.0)).sqrt().min(1.25)).min(1.0);
            }
            if !self.st.steps_pin {
                if self.ema > 40.0 && self.st.steps > 44.0 {
                    self.st.steps -= 8.0;
                } else if self.ema < 12.0 && self.st.steps < 192.0 {
                    self.st.steps += 8.0;
                }
            }
            self.frames = 0;
        }
        if self.warm > 0 {
            self.warm -= 1;
        }
    }

    // ---- input ----------------------------------------------------------

    fn key(&mut self, code: KeyCode, pressed: bool, el: &ActiveEventLoop) {
        let shift = self.mods.shift_key();
        let ctrl = self.mods.control_key() || self.mods.super_key();

        // WASDQE belong to fly mode while it is on; A would otherwise cycle the AA
        if self.st.fly && !ctrl {
            let held = &mut self.held;
            let mut hit = true;
            match code {
                KeyCode::KeyW => held.w = pressed,
                KeyCode::KeyA => held.a = pressed,
                KeyCode::KeyS => held.s = pressed,
                KeyCode::KeyD => held.d = pressed,
                KeyCode::KeyQ => held.q = pressed,
                KeyCode::KeyE => held.e = pressed,
                _ => hit = false,
            }
            held.shift = shift;
            if hit {
                return;
            }
        }
        if !pressed {
            return;
        }

        // NATIVE: which graphics API is driving the frame
        if ctrl {
            match code {
                KeyCode::Digit1 | KeyCode::Numpad1 => self.want_backend = Some(Backend::Vulkan),
                KeyCode::Digit2 | KeyCode::Numpad2 => self.want_backend = Some(Backend::Dx12),
                KeyCode::Digit3 | KeyCode::Numpad3 => self.want_backend = Some(Backend::Gl),
                KeyCode::KeyQ => el.exit(),
                KeyCode::KeyS => self.save_default(),
                _ => {}
            }
            return;
        }

        match code {
            // space is the one "freeze everything" key: camera and field together
            KeyCode::Space => {
                let go = !self.st.running;
                self.st.running = go;
                self.st.morph = go;
            }
            KeyCode::KeyM => self.st.morph = !self.st.morph,
            KeyCode::KeyI => self.st.spike_on = !self.st.spike_on,
            KeyCode::KeyR => {
                let cam = self.cam;
                camera::random_view(&mut self.st, &cam, &mut self.rng);
            }
            KeyCode::Digit1 | KeyCode::Numpad1 => self.set_res(0),
            KeyCode::Digit2 | KeyCode::Numpad2 => self.set_res(1),
            KeyCode::Digit3 | KeyCode::Numpad3 => self.set_res(2),
            KeyCode::Digit4 | KeyCode::Numpad4 => self.set_res(3),
            KeyCode::KeyA => self.st.aa = (self.st.aa + 1) % 3,
            KeyCode::KeyF | KeyCode::F11 => {
                let on = !self.st.fullscreen;
                self.set_fullscreen(on);
            }
            KeyCode::KeyH | KeyCode::KeyU => self.hud = !self.hud,
            KeyCode::KeyV => {
                // NATIVE: vsync, mailbox, uncapped
                let i = Present::ALL.iter().position(|p| *p == self.st.present).unwrap_or(0);
                let p = Present::ALL[(i + 1) % Present::ALL.len()];
                self.apply_present(p);
                self.toast(format!("Present: {}", p.label()));
            }
            KeyCode::Escape if self.midi.mapping_mode() => {
                self.midi.learn = crate::midi::Learn::Off;
            }
            KeyCode::Escape => {
                if self.bench.on {
                    self.stop_bench();
                } else if self.report.is_some() {
                    self.report = None;
                } else if self.st.fullscreen {
                    self.set_fullscreen(false);
                }
            }
            // shift+` hands the camera over without reaching for the console
            KeyCode::Backquote if shift => {
                let (cam, on) = (self.cam, !self.st.fly);
                camera::set_fly(&mut self.st, on, &cam, &mut self.held);
            }
            KeyCode::Digit5 | KeyCode::Numpad5 => self.fav(0, shift),
            KeyCode::Digit6 | KeyCode::Numpad6 => self.fav(1, shift),
            KeyCode::Digit7 | KeyCode::Numpad7 => self.fav(2, shift),
            KeyCode::Digit8 | KeyCode::Numpad8 => self.fav(3, shift),
            KeyCode::Digit9 | KeyCode::Numpad9 => self.fav(4, shift),
            KeyCode::Digit0 | KeyCode::Numpad0 => self.fav(5, shift),
            _ => {}
        }
    }

    pub(crate) fn fav(&mut self, i: usize, store_it: bool) {
        const LBL: [&str; 6] = ["5", "6", "7", "8", "9", "0"];
        if store_it {
            camera::fav_store(&mut self.st, i);
            self.toast(format!("View {} stored", LBL[i]));
        } else {
            let cam = self.cam;
            let mut held = self.held;
            let ok = camera::fav_recall(&mut self.st, i, &cam, &mut held);
            self.held = held;
            if ok {
                self.cam = camera::camera(&self.st, self.st.clock);
                self.toast(format!("View {}", LBL[i]));
            } else {
                self.toast(format!("View {} is empty", LBL[i]));
            }
        }
    }

    /// One mapped message, applied. This is the only place a parameter id turns
    /// into a field or a call, so `midi::PARAMS` and this `match` are the two halves
    /// of the same list — an id in one and not the other simply does nothing.
    fn midi_act(&mut self, out: Out) {
        match out {
            Out::Value(id, v) => self.midi_set(id, v),
            Out::Set(id, on) => self.midi_flag(id, Some(on)),
            Out::Step(id) => self.midi_step(id),
            Out::Fire(id) => self.midi_fire(id),
        }
    }

    fn midi_set(&mut self, id: &str, v: f32) {
        let st = &mut self.st;
        match id {
            "zoom" => camera::set_zoom(st, v),
            "fly_speed" => camera::set_fly_speed(st, v),
            "fly_yaw" => st.fly_yaw = v,
            "fly_pitch" => st.fly_pitch = v.clamp(-1.553, 1.553),
            "drag_az" => st.drag_az = v,
            "drag_el" => st.drag_el = v.clamp(-1.0, 1.0),
            "lc_relief" => st.lc_relief = v,
            "lc_freq" => st.lc_freq = v,
            "lc_thick" => st.lc_thick = v,
            "lc_twirl" => st.lc_twirl = v,
            "steps" => {
                st.steps = v;
                st.steps_pin = true; // as the slider does: stop the controller trimming it
            }
            "eps" => st.eps = v,
            "omega" => st.omega = v,
            "bound_pad" => st.bound_pad = v,
            "density" => st.density = v,
            "warp_amt" => st.warp_amt = v,
            "warp_freq" => st.warp_freq = v,
            "thick" => st.thick = v,
            "spike" => st.spike = v,
            "mat_smooth" => st.mat_smooth = v,
            "mat_metal" => st.mat_metal = v,
            "sharpen" => st.sharpen = v,
            "post_exposure" => st.post_exposure = v,
            "post_glow" => st.post_glow = v,
            "post_fog" => st.post_fog = v,
            "post_vignette" => st.post_vignette = v,
            "post_grain" => st.post_grain = v,
            // a CC scanning across a segmented control lands on one of its choices
            "scene" => st.scene = (v as usize).min(2),
            "mode" => st.mode = (v as usize).min(2),
            "bound" => st.bound = (v as usize).min(2),
            "warp_mode" => st.warp_mode = (v as usize).min(1),
            "aa" => st.aa = (v as usize).min(2),
            "upscale" => st.upscale = (v as usize).min(3),
            "res_pin" => {
                let want = (v as usize).min(3);
                if want != st.res_pin {
                    self.set_res(want);
                }
            }
            "fly" => self.midi_flag("fly", Some(v >= 0.5)),
            _ => self.midi_flag(id, None),
        }
    }

    /// The switches. `None` flips whatever is there, which is what a pad does.
    fn midi_flag(&mut self, id: &str, on: Option<bool>) {
        let flip = |cur: bool| on.unwrap_or(!cur);
        match id {
            "running" => self.st.running = flip(self.st.running),
            "morph" => self.st.morph = flip(self.st.morph),
            "spike_on" => self.st.spike_on = flip(self.st.spike_on),
            "fly" => {
                let want = flip(self.st.fly);
                if want != self.st.fly {
                    let cam = self.cam;
                    let mut held = self.held;
                    camera::set_fly(&mut self.st, want, &cam, &mut held);
                    self.held = held;
                }
            }
            _ => {
                if let Some(i) = id.strip_prefix("warp_on.").and_then(|n| n.parse::<usize>().ok()) {
                    if i < 3 {
                        self.st.warp_on[i] = flip(self.st.warp_on[i]);
                    }
                } else if let Some(i) =
                    id.strip_prefix("lc_on.").and_then(|n| n.parse::<usize>().ok())
                {
                    if i < 5 {
                        self.st.lc_on[i] = flip(self.st.lc_on[i]);
                    }
                }
            }
        }
    }

    /// A pad on a segmented control moves it to the next choice and wraps.
    fn midi_step(&mut self, id: &str) {
        let n = match crate::midi::find(id).map(|d| d.kind) {
            Some(crate::midi::Kind::Select(n)) => n,
            _ => return self.midi_flag(id, None), // a toggle steps by flipping
        };
        let cur = match id {
            "scene" => self.st.scene,
            "mode" => self.st.mode,
            "bound" => self.st.bound,
            "warp_mode" => self.st.warp_mode,
            "aa" => self.st.aa,
            "upscale" => self.st.upscale,
            "res_pin" => self.st.res_pin,
            _ => return,
        };
        self.midi_set(id, ((cur + 1) % n) as f32);
    }

    /// The things that happen once. Every one of them is something a key already
    /// does, called the same way, so a pad and a key cannot drift apart.
    fn midi_fire(&mut self, id: &str) {
        match id {
            "cam.random" => {
                let cam = self.cam;
                camera::random_view(&mut self.st, &cam, &mut self.rng);
            }
            "settings.save" => self.save_default(),
            _ => {
                if let Some(i) = id.strip_prefix("fav.recall.").and_then(|n| n.parse().ok()) {
                    if i < 6 {
                        self.fav(i, false);
                    }
                } else if let Some(i) = id.strip_prefix("fav.store.").and_then(|n| n.parse().ok()) {
                    if i < 6 {
                        self.fav(i, true);
                    }
                }
            }
        }
    }

    pub(crate) fn set_res(&mut self, v: usize) {
        self.st.res_pin = v;
        if v == 1 {
            self.st.scale_q = 0.5;
        } else if v == 2 {
            self.st.scale_q = 1.0;
        }
        // NATIVE: a window can be resized to the render, which a browser tab cannot.
        // One render pixel per screen pixel, and the post pass has nothing to fit.
        if v == 3 {
            if let Some(w) = &self.window {
                if !self.st.fullscreen {
                    let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(FHD_W, FHD_H));
                }
            }
            self.toast("Rendering 1920 \u{d7} 1080");
        }
    }

    /// Everything the console can set, and the controller wiring with it — they are
    /// two files, but one thing to a user pressing ctrl+S.
    pub(crate) fn save_default(&mut self) {
        let mapped = store::save_midi(&self.midi.maps);
        match (store::save_settings(&self.st), mapped) {
            (Ok(p), Ok(())) => {
                self.note = format!("Saved {}", p.display());
                self.toast("Saved as default");
            }
            (Err(e), _) | (_, Err(e)) => {
                self.note = format!("Could not save: {e}");
                self.toast("Could not save");
            }
        }
    }

    pub(crate) fn reset_default(&mut self) {
        let keep = (self.st.backend, self.st.present, self.st.fullscreen, self.st.exclusive);
        self.st = self.builtin.clone();
        self.st.backend = keep.0;
        self.st.present = keep.1;
        self.st.fullscreen = keep.2;
        self.st.exclusive = keep.3;
        self.midi.maps.clear();
        self.midi.selected = None;
        match store::clear_settings().and_then(|()| store::clear_midi()) {
            Ok(()) => self.note = "Settings and mappings reset".into(),
            Err(e) => self.note = format!("Could not clear the file: {e}"),
        }
        self.toast("Reset");
    }

    pub(crate) fn save_template(&mut self) {
        let name = self.tpl_name.trim().to_string();
        if name.is_empty() {
            self.toast("Give the template a name first");
            return;
        }
        let t = Template { name: name.clone(), saved_at: store::now_ms(), data: self.st.clone() };
        if let Some(slot) = self.templates.iter_mut().find(|x| x.name == name) {
            *slot = t;
        } else {
            self.templates.push(t);
        }
        match store::save_templates(&self.templates) {
            Ok(()) => {
                self.note = format!("Template \u{201c}{name}\u{201d} saved");
                self.tpl_name.clear();
            }
            Err(e) => self.note = format!("Could not save the template: {e}"),
        }
    }

    /// Put back the settings and the six views the web page was left on. The
    /// snapshot ships with the binary, so this works with no network.
    pub(crate) fn load_web_default(&mut self) {
        let keep = (self.st.backend, self.st.present, self.st.fullscreen, self.st.exclusive);
        self.st = store::web_default();
        self.st.backend = keep.0;
        self.st.present = keep.1;
        self.st.fullscreen = keep.2;
        self.st.exclusive = keep.3;
        self.st.scale_q = match self.st.res_pin {
            2 | 3 => 1.0,
            _ => 0.5,
        };
        self.toast("Loaded the web page's settings and views");
    }

    pub(crate) fn load_template(&mut self, i: usize) {
        let Some(t) = self.templates.get(i) else { return };
        let name = t.name.clone();
        let keep = (self.st.backend, self.st.present, self.st.fullscreen, self.st.exclusive);
        self.st = t.data.clone();
        self.st.backend = keep.0;
        self.st.present = keep.1;
        self.st.fullscreen = keep.2;
        self.st.exclusive = keep.3;
        self.toast(format!("Loaded \u{201c}{name}\u{201d}"));
    }

    pub(crate) fn delete_template(&mut self, i: usize) {
        if i >= self.templates.len() {
            return;
        }
        let name = self.templates.remove(i).name;
        match store::save_templates(&self.templates) {
            Ok(()) => self.note = format!("Deleted \u{201c}{name}\u{201d}"),
            Err(e) => self.note = format!("Could not save the template list: {e}"),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let Some(window) = self.make_window(el, None) else {
            el.exit();
            return;
        };
        self.window = Some(window);
        let backend = self.st.backend;
        self.build_gfx(el, backend); // brings up the device and the console with it
        if self.st.fullscreen {
            self.set_fullscreen(true);
        }
        self.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone() else { return };
        let consumed = match &mut self.egui_state {
            Some(s) => s.on_window_event(&window, &event).consumed,
            None => false,
        };

        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = &mut self.gfx {
                    g.resize_surface(size.width, size.height);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(g) = &mut self.gfx {
                    g.reconfigure();
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                self.held.shift = self.mods.shift_key();
            }
            WindowEvent::Focused(false) => {
                // a key held while the window loses focus never sends its release
                self.held.clear();
                self.dragging = false;
                self.looking = false;
            }
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => {
                // keys are ignored while a text field has focus
                if self.egui_ctx.egui_wants_keyboard_input() && event.state == ElementState::Pressed
                {
                    return;
                }
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed, el);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                if down && consumed {
                    return;
                }
                match button {
                    MouseButton::Left => self.dragging = down && !self.st.fly,
                    MouseButton::Right => self.looking = down && self.st.fly,
                    _ => {}
                }
                if !down {
                    self.dragging = false;
                    self.looking = false;
                }
                let grab = self.looking;
                window.set_cursor_visible(!grab);
                let _ = window.set_cursor_grab(if grab {
                    winit::window::CursorGrabMode::Confined
                } else {
                    winit::window::CursorGrabMode::None
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if consumed {
                    return;
                }
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                if dy != 0.0 {
                    // spreading two fingers means "closer" when orbiting, "faster" when flying
                    if self.st.fly {
                        let v = self.st.fly_speed * (dy.signum() * 0.14).exp();
                        camera::set_fly_speed(&mut self.st, v);
                        self.toast(format!("{:.2} units/s", self.st.fly_speed));
                    } else {
                        let z = self.st.zoom * (1.0 - dy.signum() * 0.07);
                        camera::set_zoom(&mut self.st, z);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(b) = self.want_backend.take() {
                    if self.bench.on {
                        // swapping the device mid-run would measure two machines
                        self.toast("Not while a benchmark is running");
                    } else if b != self.st.backend && b.available() {
                        self.build_gfx(el, b);
                        self.toast(format!("{}", self.st.backend.label()));
                    } else if !b.available() {
                        self.toast(format!("{} is not available on this system", b.label()));
                    }
                }
                self.draw();
                if self.quit {
                    el.exit();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            let (dx, dy) = (delta.0 as f32, delta.1 as f32);
            if self.looking && self.st.fly {
                camera::fly_look(&mut self.st, dx, dy);
            } else if self.dragging && !self.st.fly {
                self.st.vel_az = -dx * 0.0032;
                self.st.vel_el = dy * 0.0024;
                self.st.drag_az += self.st.vel_az * 3.0;
                self.st.drag_el = (self.st.drag_el + self.st.vel_el * 3.0).clamp(-1.0, 1.0);
            }
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
