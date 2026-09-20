//! Everything the console can set, in one place.
//!
//! A one-for-one port of the `state` object in index.html, plus the handful of
//! settings only a native build has (which graphics API is live, how frames are
//! presented, full screen). Field order follows the page so the two can be diffed.

use serde::{Deserialize, Serialize};

pub const UM_PER_UNIT: f32 = 200.0;

/// How far each object actually reaches. Every jump stays inside it, close enough
/// that the object overruns the frame instead of sitting in the middle.
pub const SCENE_R: [f32; 3] = [1.55, 1.15, 1.10];

pub const OBJ_NAME: [&str; 3] = ["Brain", "Neuron", "Chrome"];
pub const UPSCALE_NAME: [&str; 4] = ["Off", "Quality", "Balanced", "Performance"];
/// Construction, Camera, Object, Post-processing, Rendering, Templates
pub const GROUPS: usize = 6;
pub const RES_NAME: [&str; 4] = ["Auto", "Half", "Native", "FHD"];
pub const AA_NAME: [&str; 3] = ["Off", "FXAA", "SSAA \u{d7}4"];
pub const BND_NAME: [&str; 3] = ["Sphere", "Box", "Auto"];
pub const WARP_NAME: [&str; 3] = ["Noise", "Twist", "Bend"];
pub const PATH_NAME: [&str; 3] = ["Orbit", "Fly-through", "Drift"];

pub const FHD_W: u32 = 1920;
pub const FHD_H: u32 = 1080;

/// NATIVE: the page stops at 320 because ANGLE unrolls the march at link time. A
/// native driver takes a dynamic bound, so the budget can go as far as the GPU can
/// pay for it. 320 stays the default, which is what keeps a native number
/// comparable with a browser one.
pub const STEPS_MAX: f32 = 768.0;

/// Which graphics API is driving the frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Backend {
    Vulkan,
    Dx12,
    Gl,
}

impl Backend {
    pub const ALL: [Backend; 3] = [Backend::Vulkan, Backend::Dx12, Backend::Gl];

    pub fn label(self) -> &'static str {
        match self {
            Backend::Vulkan => "Vulkan",
            Backend::Dx12 => "DirectX 12",
            Backend::Gl => "OpenGL",
        }
    }

    pub fn wgpu(self) -> wgpu::Backends {
        match self {
            Backend::Vulkan => wgpu::Backends::VULKAN,
            Backend::Dx12 => wgpu::Backends::DX12,
            Backend::Gl => wgpu::Backends::GL,
        }
    }

    pub fn parse(s: &str) -> Option<Backend> {
        match s.to_ascii_lowercase().as_str() {
            "vulkan" | "vk" => Some(Backend::Vulkan),
            "dx12" | "d3d12" | "directx" | "directx12" => Some(Backend::Dx12),
            "gl" | "opengl" | "ogl" => Some(Backend::Gl),
            _ => None,
        }
    }

    /// DirectX 12 only exists on Windows; everywhere else it is Vulkan or GL.
    pub fn available(self) -> bool {
        match self {
            Backend::Dx12 => cfg!(windows),
            Backend::Vulkan => !cfg!(target_os = "macos"),
            Backend::Gl => true,
        }
    }
}

/// How finished frames reach the screen. The page never had this choice — a tab is
/// locked to the compositor — and it is the single biggest thing between a native
/// build and a real measurement.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Present {
    /// Wait for the display. No tearing, and the frame rate can never read above it.
    Vsync,
    /// Newest finished frame wins, no tearing, no wait. Uncapped where the driver has it.
    Mailbox,
    /// Straight to the screen the moment it is done. Uncapped, and tears.
    Immediate,
}

impl Present {
    pub const ALL: [Present; 3] = [Present::Vsync, Present::Mailbox, Present::Immediate];

    pub fn label(self) -> &'static str {
        match self {
            Present::Vsync => "Vsync",
            Present::Mailbox => "Mailbox",
            Present::Immediate => "Uncapped",
        }
    }

    pub fn wgpu(self) -> wgpu::PresentMode {
        match self {
            Present::Vsync => wgpu::PresentMode::AutoVsync,
            Present::Mailbox => wgpu::PresentMode::Mailbox,
            Present::Immediate => wgpu::PresentMode::Immediate,
        }
    }
}

/// One saved viewpoint: the whole camera, including which way it was navigating.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Fav {
    pub fly: bool,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub mode: usize,
    pub clock: f32,
    pub az: f32,
    pub el: f32,
    pub zoom: f32,
}

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub mode: usize,
    pub scene: usize,
    pub thick: f32,
    pub aa: usize,
    pub res_pin: usize,
    pub warp_amt: f32,
    pub warp_freq: f32,
    pub warp_on: [bool; 3],
    /// noise warp: 0 bends the space, 1 the surface
    pub warp_mode: usize,
    pub lc_on: [bool; 5],
    pub eps: f32,
    pub steps_pin: bool,
    pub omega: f32,
    pub bound: usize,
    pub bound_pad: f32,
    pub lc_relief: f32,
    pub lc_freq: f32,
    pub lc_thick: f32,
    pub lc_twirl: f32,
    /// structure + accent colour per object: brain, neuron, LiquidChrome
    pub colors: [[[f32; 3]; 2]; 3],
    pub steps: f32,
    pub density: f32,
    pub spike: f32,
    pub spike_on: bool,
    pub zoom: f32,
    pub running: bool,
    pub morph: bool,
    /// free navigation: world position, look angles, units per second
    pub fly: bool,
    pub fly_pos: [f32; 3],
    pub fly_yaw: f32,
    pub fly_pitch: f32,
    pub fly_speed: f32,
    /// six camera slots, recalled with 5-0 and stored with shift+5-0
    pub favs: [Option<Fav>; 6],
    /// which console groups are open. A Vec rather than an array so that adding a
    /// group does not make every settings file already on disk unreadable.
    pub groups: Vec<bool>,
    // ---- native only ----
    /// the surface: highlight width and how metallic it reads
    pub mat_smooth: f32,
    pub mat_metal: f32,
    /// the passes after it. All of these were constants in the page's shader.
    pub post_exposure: f32,
    pub post_glow: f32,
    pub post_fog: f32,
    pub post_vignette: f32,
    pub post_grain: f32,
    /// the background gradient, low (looking down) and high (looking up)
    pub bg_low: [f32; 3],
    pub bg_high: [f32; 3],
    /// The two tints the page kept as constants in its lighting, and the reason an
    /// object with both colours black still was not black: the fresnel rim, and the
    /// back light that also colours the impulses and half the glow.
    pub rim_col: [f32; 3],
    pub spike_col: [f32; 3],
    /// 0 off, else render below the window and reconstruct: 1 quality, 2 balanced,
    /// 3 performance
    pub upscale: usize,
    pub sharpen: f32,
    pub backend: Backend,
    pub present: Present,
    pub fullscreen: bool,
    pub exclusive: bool,

    // ---- not saved: measured, adaptive or per-run ----
    #[serde(skip)]
    pub scale_q: f32,
    #[serde(skip, default = "one")]
    pub fov: f32,
    #[serde(skip)]
    pub shift: f32,
    #[serde(skip, default = "one")]
    pub fit: f32,
    #[serde(skip)]
    pub clock: f32,
    #[serde(skip)]
    pub m_clock: f32,
    #[serde(skip)]
    pub p_clock: f32,
    #[serde(skip)]
    pub drag_az: f32,
    #[serde(skip)]
    pub drag_el: f32,
    #[serde(skip)]
    pub vel_az: f32,
    #[serde(skip)]
    pub vel_el: f32,
}

fn one() -> f32 {
    1.0
}

impl Default for State {
    fn default() -> Self {
        State {
            mode: 0,
            scene: 0,
            thick: 0.028,
            aa: 1,
            res_pin: 1, // Half, to pay for the march budget below
            warp_amt: 0.74,
            warp_freq: 3.75,
            warp_on: [true, false, false],
            warp_mode: 0,
            lc_on: [true; 5],
            eps: 12.00,
            steps_pin: true,
            omega: 1.30,
            bound: 2,
            bound_pad: 0.05,
            lc_relief: 0.26,
            lc_freq: 4.00,
            lc_thick: 0.022,
            lc_twirl: 1.90,
            colors: [
                [rgb(0x2F, 0xD9, 0xC0), rgb(0xFF, 0xC9, 0x8A)],
                [rgb(0x2F, 0xD9, 0xC0), rgb(0xFF, 0xC9, 0x8A)],
                [rgb(0x00, 0x9D, 0xFF), rgb(0xFF, 0x42, 0x71)],
            ],
            steps: 320.0,
            density: 15.0,
            spike: 0.55,
            spike_on: true,
            zoom: 1.0,
            running: true,
            morph: true,
            fly: false,
            fly_pos: [0.0, 0.0, 3.0],
            fly_yaw: 0.0,
            fly_pitch: 0.0,
            fly_speed: 0.60,
            favs: [None; 6],
            mat_smooth: 0.5, // exp2(1 + 9.169925*0.5) = 48 exactly, the page's exponent
            mat_metal: 0.0,
            post_exposure: 1.0,
            post_glow: 1.0,
            post_fog: 1.0,
            post_vignette: 0.55,
            post_grain: 0.022,
            bg_low: [0.010, 0.015, 0.026],
            bg_high: [0.020, 0.032, 0.055],
            rim_col: [0.620, 0.898, 1.000],
            spike_col: [0.608, 0.482, 1.000],
            upscale: 0,
            sharpen: 0.35,
            groups: vec![true; GROUPS],
            backend: default_backend(),
            present: Present::Vsync,
            fullscreen: false,
            exclusive: false,

            scale_q: 0.50, // 1.0 == one sample per physical pixel
            fov: 1.30,
            shift: 0.0,
            fit: 1.0,
            clock: 12.0,
            m_clock: 0.0,
            p_clock: 6.0,
            drag_az: 0.0,
            drag_el: 0.0,
            vel_az: 0.0,
            vel_el: 0.0,
        }
    }
}

pub fn default_backend() -> Backend {
    if cfg!(windows) {
        Backend::Vulkan
    } else {
        Backend::Vulkan
    }
}

impl State {
    /// Is this console group open? Reads short — a settings file written before a
    /// group existed simply opens the new one.
    pub fn group_open(&self, i: usize) -> bool {
        self.groups.get(i).copied().unwrap_or(true)
    }

    pub fn set_group(&mut self, i: usize, open: bool) {
        if self.groups.len() <= i {
            self.groups.resize(i + 1, true);
        }
        self.groups[i] = open;
    }

    /// How much of the window the march covers when the upscaler is on. These are
    /// the ratios the vendors' own presets use, so the labels mean what people
    /// expect them to mean.
    pub fn upscale_ratio(&self) -> Option<f32> {
        match self.upscale {
            1 => Some(0.667), // quality
            2 => Some(0.588), // balanced
            3 => Some(0.500), // performance
            _ => None,
        }
    }

    /// A warp divides the marching step by up to this much, so the ray needs
    /// proportionally more of them to cover the same distance.
    pub fn warp_k(&self) -> f32 {
        let r = 1.74_f32;
        let mut k = 1.0_f32;
        if self.warp_on[2] {
            k *= (1.0 + (1.60 * self.warp_amt * r).powi(2)).sqrt();
        }
        if self.warp_on[1] {
            k *= (1.0 + (1.70 * self.warp_amt * r).powi(2)).sqrt();
        }
        if self.warp_on[0] {
            let kn = 0.55 * self.warp_amt * self.warp_freq;
            // the object warp's height scales with its own frequency, so its bound
            // does not grow with the scale slider — and it is only paid in the band
            k *= if self.warp_mode == 0 { 1.0 + kn * 2.2 } else { 1.0 + 0.34 * self.warp_amt };
        }
        if self.scene == 2 && self.lc_on[1] {
            k *= 1.0 + self.lc_relief * self.lc_freq * 3.0;
        }
        k.min(3.2)
    }

    /// Pinned by hand the slider is the budget; on auto it is scaled by however much
    /// the warp shortens each step.
    pub fn march_steps(&self, cap: f32) -> f32 {
        let want = if self.steps_pin { self.steps } else { (self.steps * self.warp_k()).round() };
        want.min(cap).max(8.0)
    }

    pub fn col_a(&self) -> [f32; 3] {
        self.colors[self.scene][0]
    }
    pub fn col_b(&self) -> [f32; 3] {
        self.colors[self.scene][1]
    }
}

/// The three sizes a frame is decided by. `base` is what the Resolution control
/// asks for, `marched` is what the shader actually runs at, and the window — which
/// the last post pass always writes to — is the caller's own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizes {
    pub base: (u32, u32),
    pub marched: (u32, u32),
    /// portrait compensation for the camera radius, from the *base* aspect
    pub fit: f32,
    /// the march is a fraction of the base and the post pass rebuilds it. Kept as a
    /// fact rather than recomputed, so the frame loop and the sizing cannot drift
    /// apart — and it is not `marched != base`, which SSAA also makes true.
    pub upscaling: bool,
}

/// How big to march, and what to reconstruct to, for one frame.
///
/// Base first, marched second — never the other way round. The upscaler divides the
/// resolution that was *chosen*, so FHD plus Performance marches half of 1920x1080
/// rather than half of whatever the window happens to be. That is the whole point of
/// pinning a resolution while a projector is watching.
///
/// `warm` is the first few frames after a start or a backend switch, when neither
/// FHD nor the upscaler is honoured and the image is kept tiny.
pub fn frame_size(win: (u32, u32), st: &State, warm: bool, march_cap: f32) -> Sizes {
    // SSAA marches at 2x per axis and lets the post pass filter it down, so the
    // budget applies to the marched size, not to what is displayed
    let ss = if st.aa == 2 { 2 } else { 1 };

    let (mut bw, mut bh);
    if st.res_pin == 3 && !warm {
        // FHD is a literal size, and by long-standing intent it escapes the clamps
        // below: chosen by hand it outranks them.
        bw = FHD_W;
        bh = FHD_H;
    } else {
        // Half and Native pin the scale; Auto hands it to the frame-time controller.
        // Either way it is the same field, so a pinned resolution can still be given
        // up when the frame time becomes untenable.
        let q = st.scale_q;
        bw = ((win.0 as f32 * q).round() as u32).max(2);
        bh = ((win.1 as f32 * q).round() as u32).max(2);
        let max_px: f32 = if warm {
            1.1e5
        } else if march_cap <= 96.0 {
            4.0e5 // software
        } else if st.res_pin != 0 {
            3.4e7 // pinned by hand: let the GPU stretch
        } else {
            3.0e6 // auto stays inside what a per-pixel marcher can sustain
        };
        let px = (bw * bh * ss * ss) as f32;
        if px > max_px {
            let k = (max_px / px).sqrt();
            bw = (((bw as f32) * k).round() as u32).max(2);
            bh = (((bh as f32) * k).round() as u32).max(2);
        }
    }

    // The upscaler marches a fraction of the base and the post pass rebuilds the
    // rest. SSAA still multiplies on top — supersampling means marching more pixels
    // than the output by definition, and it hands the reconstruction a cleaner image
    // to work from.
    let ratio = if warm { None } else { st.upscale_ratio() };
    let (mw, mh) = match ratio {
        Some(r) => (
            ((bw as f32 * r).round() as u32 * ss).max(2),
            ((bh as f32 * r).round() as u32 * ss).max(2),
        ),
        None => ((bw * ss).max(2), (bh * ss).max(2)),
    };

    // The framing follows the picture, which is the base — with FHD the image is
    // 16:9 however tall the window is.
    let fit = if bh as f32 > bw as f32 * 1.25 { 0.92 } else { 1.0 };
    Sizes { base: (bw, bh), marched: (mw, mh), fit, upscaling: ratio.is_some() }
}
