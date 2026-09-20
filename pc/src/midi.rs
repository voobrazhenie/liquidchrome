//! MIDI in: driving the console from a controller, the way Ableton Live does it.
//!
//! Press **Map**, click a control, move a knob — that pairing is a `Mapping`, and it
//! lives in `midi.json` next to the binary rather than inside `State`, because
//! loading a template replaces every setting and must not also re-wire the
//! controller under the user's hands.
//!
//! Everything above the device layer is plain arithmetic with no I/O in it, so the
//! whole value path can be tested without a controller — which matters, because this
//! container has neither MIDI hardware nor the ALSA headers to build against. Only
//! `backend` is platform specific, and on anything but Windows it is a stub that
//! finds no ports.
//!
//! Smoothing is **time based**, like every other clock in the app: a mapping settles
//! at the same rate at 20 fps and at 200 fps, so a controller feels the same however
//! the frame rate is behaving.

use serde::{Deserialize, Serialize};

/// The longest a smoothed value takes to close most of the gap, in seconds, at
/// smoothing 1.0. Long enough to ride out a jumpy knob, short enough to still feel
/// like a hand is on it.
pub const SMOOTH_MAX: f32 = 0.5;

// ---- what can be mapped ------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A continuous value. The range is the one the console's own slider draws, so a
    /// fresh mapping spans exactly what the control is useful across.
    Float { lo: f32, hi: f32 },
    /// An on/off switch.
    Toggle,
    /// One of n choices; a note steps to the next, a CC scans across them.
    Select(usize),
    /// Something that happens once. No range and no smoothing to give it.
    Action,
}

pub struct ParamDef {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub kind: Kind,
}

impl ParamDef {
    /// Does this parameter take a min, a max and a smoothing amount? Only continuous
    /// ones do — the rest are edges, and an edge has nothing to interpolate.
    pub fn ranged(&self) -> bool {
        matches!(self.kind, Kind::Float { .. })
    }
}

const fn f(
    id: &'static str,
    name: &'static str,
    group: &'static str,
    lo: f32,
    hi: f32,
) -> ParamDef {
    ParamDef { id, name, group, kind: Kind::Float { lo, hi } }
}
const fn t(id: &'static str, name: &'static str, group: &'static str) -> ParamDef {
    ParamDef { id, name, group, kind: Kind::Toggle }
}
const fn s(id: &'static str, name: &'static str, group: &'static str, n: usize) -> ParamDef {
    ParamDef { id, name, group, kind: Kind::Select(n) }
}
const fn a(id: &'static str, name: &'static str, group: &'static str) -> ParamDef {
    ParamDef { id, name, group, kind: Kind::Action }
}

/// Every mappable control, with the range its console slider uses. The ids are
/// stable keys written into `midi.json`, deliberately *not* the display labels —
/// renaming a label in the console must not silently break somebody's mappings.
pub const PARAMS: &[ParamDef] = &[
    // construction
    s("scene", "Object", "Construction", 3),
    t("fly", "Fly mode", "Construction"),
    s("mode", "Flight path", "Construction", 3),
    // camera
    f("zoom", "Zoom", "Camera", 0.42, 1.8),
    f("fly_speed", "Fly speed", "Camera", 0.03, 12.0),
    f("fly_yaw", "Yaw", "Camera", -3.1416, 3.1416),
    f("fly_pitch", "Pitch", "Camera", -1.553, 1.553),
    f("drag_az", "Steer across", "Camera", -3.1416, 3.1416),
    f("drag_el", "Steer up", "Camera", -1.0, 1.0),
    a("cam.random", "New view", "Camera"),
    a("fav.recall.0", "Recall view 5", "Camera"),
    a("fav.recall.1", "Recall view 6", "Camera"),
    a("fav.recall.2", "Recall view 7", "Camera"),
    a("fav.recall.3", "Recall view 8", "Camera"),
    a("fav.recall.4", "Recall view 9", "Camera"),
    a("fav.recall.5", "Recall view 0", "Camera"),
    a("fav.store.0", "Store view 5", "Camera"),
    a("fav.store.1", "Store view 6", "Camera"),
    a("fav.store.2", "Store view 7", "Camera"),
    a("fav.store.3", "Store view 8", "Camera"),
    a("fav.store.4", "Store view 9", "Camera"),
    a("fav.store.5", "Store view 0", "Camera"),
    // object
    f("lc_relief", "Relief", "Object", 0.0, 0.90),
    f("lc_freq", "Terrain scale", "Object", 0.60, 11.0),
    f("lc_thick", "Slab thickness", "Object", 0.003, 0.120),
    f("lc_twirl", "Twirl", "Object", 0.0, 3.0),
    t("lc_on.0", "Slab", "Object"),
    t("lc_on.1", "Relief step", "Object"),
    t("lc_on.2", "Copy", "Object"),
    t("lc_on.3", "Intersect", "Object"),
    t("lc_on.4", "Twirl step", "Object"),
    f("steps", "Ray steps", "Object", 48.0, 768.0),
    f("eps", "Surface precision", "Object", 1.0, 12.0),
    f("omega", "Step relaxation", "Object", 1.00, 1.90),
    s("bound", "Bounds", "Object", 3),
    f("bound_pad", "Bound padding", "Object", 0.0, 0.60),
    f("density", "Cell density", "Object", 9.0, 22.0),
    t("warp_on.0", "Noise warp", "Object"),
    t("warp_on.1", "Twist warp", "Object"),
    t("warp_on.2", "Bend warp", "Object"),
    s("warp_mode", "Noise warp target", "Object", 2),
    f("warp_amt", "Warp strength", "Object", 0.0, 1.0),
    f("warp_freq", "Warp scale", "Object", 0.30, 4.00),
    f("thick", "Line width", "Object", 0.006, 0.070),
    f("spike", "Spike rate", "Object", 0.0, 1.0),
    f("mat_smooth", "Smoothness", "Object", 0.0, 1.0),
    f("mat_metal", "Metalness", "Object", 0.0, 1.0),
    t("running", "Flight", "Object"),
    t("morph", "Morph", "Object"),
    t("spike_on", "Impulses", "Object"),
    // post
    s("aa", "Antialiasing", "Post", 3),
    s("upscale", "Upscale", "Post", 4),
    f("sharpen", "Sharpen", "Post", 0.0, 1.0),
    f("post_exposure", "Exposure", "Post", 0.20, 6.00),
    f("post_glow", "Glow", "Post", 0.0, 3.0),
    f("post_fog", "Fog", "Post", 0.0, 2.0),
    f("post_vignette", "Vignette", "Post", 0.0, 1.50),
    f("post_grain", "Grain", "Post", 0.0, 0.12),
    // rendering
    s("res_pin", "Resolution", "Rendering", 4),
    a("settings.save", "Save as default", "Rendering"),
];

pub fn find(id: &str) -> Option<&'static ParamDef> {
    PARAMS.iter().find(|p| p.id == id)
}

// ---- messages ----------------------------------------------------------------

/// The two message shapes a control can be mapped to, exactly as Ableton allows:
/// a continuous controller, or a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Msg {
    Cc { ch: u8, cc: u8 },
    Note { ch: u8, note: u8 },
}

impl Msg {
    /// What arrived, or `None` for anything not mappable (clock, aftertouch, sysex).
    /// A note-on with zero velocity is a note-off, which many controllers send.
    pub fn parse(data: &[u8]) -> Option<(Msg, u8)> {
        if data.len() < 3 {
            return None;
        }
        let ch = data[0] & 0x0F;
        match data[0] & 0xF0 {
            0xB0 => Some((Msg::Cc { ch, cc: data[1] }, data[2])),
            0x90 => Some((Msg::Note { ch, note: data[1] }, data[2])),
            0x80 => Some((Msg::Note { ch, note: data[1] }, 0)),
            _ => None,
        }
    }

    /// Channels read 1-16 everywhere a person looks at them, and 0-15 on the wire.
    pub fn channel(&self) -> u8 {
        match self {
            Msg::Cc { ch, .. } | Msg::Note { ch, .. } => ch + 1,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Msg::Cc { cc, .. } => format!("CC {cc}"),
            Msg::Note { note, .. } => format!("Note {}", note_name(*note)),
        }
    }
}

/// Middle C is C3, which is what Ableton and most controllers print on their pads.
pub fn note_name(n: u8) -> String {
    const N: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", N[(n % 12) as usize], n as i32 / 12 - 2)
}

// ---- one mapping -------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    /// which controller it came from, by port name
    pub device: String,
    pub msg: Msg,
    /// a `PARAMS` id
    pub param: String,
    /// what the control reads at the bottom and the top of the message's travel.
    /// `min` above `max` is not a mistake — it inverts the knob, which is useful.
    pub min: f32,
    pub max: f32,
    /// 0 is instant; 1 takes `SMOOTH_MAX` seconds to settle
    pub smooth: f32,
    /// notes only: flip on each press, rather than being on while held
    pub toggle: bool,

    /// where the message says the value should be, 0..1 along `min`..`max`
    #[serde(skip)]
    pub want: f32,
    /// where it has got to, once smoothing has had its say
    #[serde(skip)]
    pub at: f32,
    /// has `at` been seeded yet? Until the first message arrives a mapping must not
    /// drive its parameter at all, or opening the app would slam every mapped
    /// control to zero.
    #[serde(skip)]
    pub live: bool,
    /// seconds since the last message, for the activity light
    #[serde(skip)]
    pub since: f32,
}

impl Default for Mapping {
    fn default() -> Self {
        Mapping {
            device: String::new(),
            msg: Msg::Cc { ch: 0, cc: 0 },
            param: String::new(),
            min: 0.0,
            max: 1.0,
            smooth: 0.0,
            toggle: true,
            want: 0.0,
            at: 0.0,
            live: false,
            since: 99.0,
        }
    }
}

impl Mapping {
    /// A fresh mapping spans the whole of whatever it was pointed at.
    pub fn new(device: String, msg: Msg, def: &ParamDef) -> Mapping {
        let (min, max) = match def.kind {
            Kind::Float { lo, hi } => (lo, hi),
            _ => (0.0, 1.0),
        };
        Mapping { device, msg, param: def.id.to_string(), min, max, ..Mapping::default() }
    }

    /// The value the parameter should be reading now.
    pub fn value(&self) -> f32 {
        self.min + self.at * (self.max - self.min)
    }
}

// ---- what a frame of MIDI asks for -------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// a continuous value, sent every frame while the mapping is live
    Value(&'static str, f32),
    /// an on/off switch, on the edge only
    Set(&'static str, bool),
    /// step a segmented control to its next choice
    Step(&'static str),
    /// do the thing once
    Fire(&'static str),
}

/// Above this a controller's switch reads as pressed, below it as released. The
/// halfway point, which is what every host uses.
const HALF: u8 = 64;

// ---- the runtime -------------------------------------------------------------

/// One input port, and whether we are listening to it. Several controllers at once
/// is the normal case, so this is a list the app keeps in step with what is plugged
/// in — not a device to pick from a menu.
#[derive(Clone, Debug)]
pub struct Device {
    pub name: String,
    /// listening. New devices arrive switched on: plugging something in and having
    /// it do nothing is the worse surprise.
    pub on: bool,
    /// connected right now, as opposed to only remembered from a mapping
    pub present: bool,
    /// seconds since anything arrived from it
    pub since: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Learn {
    Off,
    /// map mode: every mappable control is showing itself, waiting to be picked
    Picking,
    /// a control has been picked and the next message it hears binds to it
    Armed(&'static str),
}

pub struct Midi {
    pub maps: Vec<Mapping>,
    pub learn: Learn,
    /// which card the Delete button would remove
    pub selected: Option<usize>,
    pub open: bool,
    pub devices: Vec<Device>,
    /// the last message seen, whatever it was, so the window can say what it heard
    pub last: Option<(String, Msg, u8)>,
    /// set when a mapping is made or dropped, so the caller knows to write the file
    pub dirty: bool,
    hub: backend::Hub,
    rescan: f32,
}

impl Midi {
    pub fn new(maps: Vec<Mapping>) -> Midi {
        let mut m = Midi {
            maps,
            learn: Learn::Off,
            selected: None,
            open: false,
            devices: Vec::new(),
            last: None,
            dirty: false,
            hub: backend::Hub::new(),
            rescan: 9.0,
        };
        m.poll_devices();
        m
    }

    pub fn mapping_mode(&self) -> bool {
        self.learn != Learn::Off
    }

    /// Is this control mapped, and is it the one waiting for a message?
    pub fn state_of(&self, id: &str) -> (bool, bool) {
        let armed = matches!(self.learn, Learn::Armed(a) if a == id);
        (self.maps.iter().any(|m| m.param == id), armed)
    }

    /// How recently this control's mapping heard something, for the moving light on
    /// its card. `None` when it is not mapped at all.
    pub fn since(&self, id: &str) -> Option<f32> {
        self.maps.iter().find(|m| m.param == id).map(|m| m.since)
    }

    /// Look at a control in map mode.
    pub fn pick(&mut self, id: &'static str) {
        self.learn = Learn::Armed(id);
    }

    pub fn forget(&mut self, i: usize) {
        if i < self.maps.len() {
            self.maps.remove(i);
            self.selected = None;
            self.dirty = true;
        }
    }

    /// Re-read the port list and open or drop connections to match. Cheap enough to
    /// call every frame; it only does the work about once a second.
    pub fn poll_devices(&mut self) {
        let ports = self.hub.ports();
        for name in &ports {
            if !self.devices.iter().any(|d| &d.name == name) {
                self.devices.push(Device {
                    name: name.clone(),
                    on: true,
                    present: true,
                    since: 99.0,
                });
            }
        }
        for d in &mut self.devices {
            d.present = ports.iter().any(|p| p == &d.name);
        }
        // a device only mappings remember, with nothing plugged in, is worth showing
        // so its cards do not look like they belong to nowhere
        for m in &self.maps {
            if !m.device.is_empty() && !self.devices.iter().any(|d| d.name == m.device) {
                self.devices.push(Device {
                    name: m.device.clone(),
                    on: true,
                    present: false,
                    since: 99.0,
                });
            }
        }
        let want: Vec<String> =
            self.devices.iter().filter(|d| d.on && d.present).map(|d| d.name.clone()).collect();
        self.hub.sync(&want);
    }

    /// Take everything that arrived since the last frame, move the smoothed values
    /// on by `dt`, and say what the app should do about it.
    ///
    /// `frozen` drops incoming messages on the floor without acting on them: a
    /// benchmark run has to render the same frames on every machine, and a knob
    /// moved half way through would make it measure two different things.
    pub fn poll(&mut self, dt: f32, frozen: bool) -> Vec<Out> {
        self.rescan += dt;
        if self.rescan > 1.0 {
            self.rescan = 0.0;
            self.poll_devices();
        }
        for d in &mut self.devices {
            d.since += dt;
        }
        for m in &mut self.maps {
            m.since += dt;
        }

        let mut out = Vec::new();
        for (device, data) in self.hub.drain() {
            let Some((msg, val)) = Msg::parse(&data) else { continue };
            if let Some(d) = self.devices.iter_mut().find(|d| d.name == device) {
                d.since = 0.0;
            }
            if frozen {
                continue;
            }
            self.last = Some((device.clone(), msg, val));
            if let Learn::Armed(id) = self.learn {
                self.bind(&device, msg, id);
                continue;
            }
            self.deliver(&device, msg, val, &mut out);
        }

        if !frozen {
            for m in &mut self.maps {
                if !m.live {
                    continue;
                }
                let Some(def) = find(&m.param) else { continue };
                if !def.ranged() {
                    continue;
                }
                m.at = ease(m.at, m.want, m.smooth, dt);
                out.push(Out::Value(def.id, m.value()));
            }
        }
        out
    }

    /// Bind the armed control to whatever just arrived, replacing any mapping that
    /// already used this message or this parameter — one message, one destination,
    /// as Ableton has it.
    fn bind(&mut self, device: &str, msg: Msg, id: &'static str) {
        let Some(def) = find(id) else { return };
        self.maps.retain(|m| !(m.device == device && m.msg == msg) && m.param != id);
        self.maps.push(Mapping::new(device.to_string(), msg, def));
        self.selected = Some(self.maps.len() - 1);
        self.learn = Learn::Picking;
        self.dirty = true;
    }

    fn deliver(&mut self, device: &str, msg: Msg, val: u8, out: &mut Vec<Out>) {
        for m in &mut self.maps {
            if m.device != device || m.msg != msg {
                continue;
            }
            let Some(def) = find(&m.param) else { continue };
            m.since = 0.0;
            match (msg, def.kind) {
                // a knob or fader: straight across the range
                (Msg::Cc { .. }, Kind::Float { .. }) => {
                    m.want = val as f32 / 127.0;
                    if !m.live {
                        m.at = m.want; // arrive where the knob is, do not sweep to it
                        m.live = true;
                    }
                }
                // a pad on a continuous value: slam between the two ends, which is
                // what makes a note worth mapping to a slider at all
                (Msg::Note { .. }, Kind::Float { .. }) => {
                    let down = val > 0;
                    if m.toggle {
                        if down {
                            m.want = if m.want > 0.5 { 0.0 } else { 1.0 };
                        }
                    } else {
                        m.want = if down { 1.0 } else { 0.0 };
                    }
                    m.live = true;
                }
                (_, Kind::Toggle) => match msg {
                    Msg::Cc { .. } => out.push(Out::Set(def.id, val >= HALF)),
                    // latching: the press flips it and the release says nothing, or
                    // letting go would put it straight back
                    Msg::Note { .. } if m.toggle => {
                        if val > 0 {
                            out.push(Out::Step(def.id));
                        }
                    }
                    Msg::Note { .. } => out.push(Out::Set(def.id, val > 0)),
                },
                (Msg::Note { .. }, Kind::Select(_)) => {
                    if val > 0 {
                        out.push(Out::Step(def.id));
                    }
                }
                (Msg::Cc { .. }, Kind::Select(n)) => {
                    let i = ((val as usize * n) / 128).min(n.saturating_sub(1));
                    out.push(Out::Value(def.id, i as f32));
                }
                (Msg::Note { .. }, Kind::Action) => {
                    if val > 0 {
                        out.push(Out::Fire(def.id));
                    }
                }
                (Msg::Cc { .. }, Kind::Action) => {
                    // only on the way up, so holding a switch down does not repeat
                    let was = m.want > 0.5;
                    m.want = if val >= HALF { 1.0 } else { 0.0 };
                    if !was && m.want > 0.5 {
                        out.push(Out::Fire(def.id));
                    }
                }
            }
        }
    }
}

/// One step of an exponential approach, written so the result depends on how much
/// time has passed and not on how many frames it took — the same discipline the
/// animation clocks keep. `smooth` 0 is instant.
pub fn ease(at: f32, want: f32, smooth: f32, dt: f32) -> f32 {
    let tau = smooth.clamp(0.0, 1.0) * SMOOTH_MAX;
    if tau <= 1e-4 || dt <= 0.0 {
        return want;
    }
    at + (want - at) * (1.0 - (-dt / tau).exp())
}

// ---- the device layer --------------------------------------------------------

/// Windows is the only platform this app ships on, and `midir`'s Linux path wants
/// ALSA headers the build container does not have — so the real backend is behind
/// `cfg(windows)` and everything else gets a stub that finds no ports. The mapping
/// engine above does not know the difference, which is what lets it be tested here.
#[cfg(windows)]
mod backend {
    use std::collections::HashMap;
    use std::sync::mpsc::{channel, Receiver, Sender};

    pub struct Hub {
        tx: Sender<(String, Vec<u8>)>,
        rx: Receiver<(String, Vec<u8>)>,
        open: HashMap<String, midir::MidiInputConnection<()>>,
    }

    impl Hub {
        pub fn new() -> Hub {
            let (tx, rx) = channel();
            Hub { tx, rx, open: HashMap::new() }
        }

        pub fn ports(&self) -> Vec<String> {
            let Ok(mi) = midir::MidiInput::new("Cortical Flythrough") else {
                return Vec::new();
            };
            mi.ports().iter().filter_map(|p| mi.port_name(p).ok()).collect()
        }

        /// Open what is wanted and not already open, drop what is no longer wanted.
        /// A connection carries its own callback thread, so the name it was opened
        /// under is baked in and a reconnect is a fresh one.
        pub fn sync(&mut self, want: &[String]) {
            self.open.retain(|name, _| want.iter().any(|w| w == name));
            for name in want {
                if self.open.contains_key(name) {
                    continue;
                }
                let Ok(mut mi) = midir::MidiInput::new("Cortical Flythrough") else { continue };
                mi.ignore(midir::Ignore::None);
                let Some(port) = mi
                    .ports()
                    .into_iter()
                    .find(|p| mi.port_name(p).map(|n| &n == name).unwrap_or(false))
                else {
                    continue;
                };
                let tx = self.tx.clone();
                let who = name.clone();
                // the receiver outlives every connection, so a send that fails means
                // the app is going away and there is nothing useful to do about it
                if let Ok(c) = mi.connect(
                    &port,
                    "cortical-flythrough-in",
                    move |_t, data, _| {
                        let _ = tx.send((who.clone(), data.to_vec()));
                    },
                    (),
                ) {
                    self.open.insert(name.clone(), c);
                }
            }
        }

        pub fn drain(&mut self) -> Vec<(String, Vec<u8>)> {
            self.rx.try_iter().collect()
        }
    }
}

#[cfg(not(windows))]
mod backend {
    pub struct Hub;

    impl Hub {
        pub fn new() -> Hub {
            Hub
        }
        pub fn ports(&self) -> Vec<String> {
            Vec::new()
        }
        pub fn sync(&mut self, _want: &[String]) {}
        pub fn drain(&mut self) -> Vec<(String, Vec<u8>)> {
            Vec::new()
        }
    }
}

/// Push a message in as if a controller had sent it. Only for tests — the value
/// path is worth checking on a machine with no MIDI in it at all.
#[cfg(test)]
impl Midi {
    pub fn feed(&mut self, device: &str, data: &[u8], dt: f32) -> Vec<Out> {
        let Some((msg, val)) = Msg::parse(data) else { return Vec::new() };
        let mut out = Vec::new();
        if let Learn::Armed(id) = self.learn {
            self.bind(device, msg, id);
        } else {
            self.deliver(device, msg, val, &mut out);
        }
        out.extend(self.advance(dt));
        out
    }

    pub fn advance(&mut self, dt: f32) -> Vec<Out> {
        let mut out = Vec::new();
        for m in &mut self.maps {
            if !m.live {
                continue;
            }
            let Some(def) = find(&m.param) else { continue };
            if !def.ranged() {
                continue;
            }
            m.at = ease(m.at, m.want, m.smooth, dt);
            out.push(Out::Value(def.id, m.value()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(param: &str) -> Midi {
        let def = find(param).expect("a real parameter");
        let mut m = Midi::new(Vec::new());
        m.maps.push(Mapping::new("test".into(), Msg::Cc { ch: 0, cc: 7 }, def));
        m
    }

    #[test]
    fn a_controller_spans_the_whole_range() {
        let mut m = one("post_exposure");
        // a fresh mapping covers the slider's own range, so nothing has to be set up
        assert_eq!((m.maps[0].min, m.maps[0].max), (0.20, 6.00));

        m.feed("test", &[0xB0, 7, 0], 0.016);
        assert!((m.maps[0].value() - 0.20).abs() < 1e-4, "bottom of the travel");
        m.feed("test", &[0xB0, 7, 127], 0.016);
        assert!((m.maps[0].value() - 6.00).abs() < 1e-4, "top of the travel");
        m.feed("test", &[0xB0, 7, 64], 0.016);
        let mid = m.maps[0].value();
        assert!(mid > 3.0 && mid < 3.2, "and the middle is the middle, got {mid}");
    }

    #[test]
    fn a_range_can_be_put_in_backwards() {
        let mut m = one("post_glow");
        m.maps[0].min = 3.0;
        m.maps[0].max = 0.0;
        m.feed("test", &[0xB0, 7, 0], 0.016);
        assert!((m.maps[0].value() - 3.0).abs() < 1e-4, "min above max inverts the knob");
        m.feed("test", &[0xB0, 7, 127], 0.016);
        assert!(m.maps[0].value().abs() < 1e-4);
    }

    /// The same property the benchmark has: what happens depends on how much time
    /// passed, not on how many frames it took.
    #[test]
    fn smoothing_is_the_same_at_any_frame_rate() {
        let settle = |fps: f32| {
            let mut m = one("post_glow");
            m.maps[0].smooth = 1.0;
            m.feed("test", &[0xB0, 7, 0], 0.0);
            m.maps[0].want = 1.0;
            let dt = 1.0 / fps;
            for _ in 0..(fps as usize) {
                m.advance(dt);
            }
            m.maps[0].at
        };
        let slow = settle(20.0);
        let fast = settle(200.0);
        assert!((slow - fast).abs() < 1e-3, "20 fps gave {slow}, 200 fps gave {fast}");
        assert!(slow > 0.85, "a second of smoothing still gets most of the way there");
    }

    #[test]
    fn smoothing_off_is_instant() {
        let mut m = one("post_glow");
        m.maps[0].smooth = 0.0;
        m.feed("test", &[0xB0, 7, 127], 1.0 / 240.0);
        assert!((m.maps[0].at - 1.0).abs() < 1e-6, "one frame, all the way");
    }

    /// Until a message arrives a mapping must stay out of the way — otherwise
    /// opening the app would slam every mapped control to the bottom of its range.
    #[test]
    fn a_mapping_says_nothing_until_it_hears_something() {
        let mut m = one("post_glow");
        assert!(m.advance(0.016).is_empty(), "silent before the first message");
        m.feed("test", &[0xB0, 7, 100], 0.016);
        assert!(!m.advance(0.016).is_empty(), "and speaks after it");
    }

    /// A knob that is already somewhere should be picked up where it is, not swept
    /// to from zero.
    #[test]
    fn the_first_message_is_arrived_at_not_swept_to() {
        let mut m = one("post_glow");
        m.maps[0].smooth = 1.0;
        m.feed("test", &[0xB0, 7, 127], 0.016);
        assert!((m.maps[0].at - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_pad_flips_a_switch_once_per_press() {
        let def = find("morph").unwrap();
        let mut m = Midi::new(Vec::new());
        m.maps.push(Mapping::new("test".into(), Msg::Note { ch: 4, note: 60 }, def));

        let out = m.feed("test", &[0x94, 60, 100], 0.016);
        assert_eq!(out, vec![Out::Step("morph")], "note on flips it");
        let out = m.feed("test", &[0x94, 60, 0], 0.016);
        assert!(out.is_empty(), "note off does not flip it back");
        let out = m.feed("test", &[0x84, 60, 40], 0.016);
        assert!(out.is_empty(), "and neither does a real note-off");
    }

    #[test]
    fn a_pad_can_be_held_instead_of_latched() {
        let def = find("morph").unwrap();
        let mut m = Midi::new(Vec::new());
        m.maps.push(Mapping::new("test".into(), Msg::Note { ch: 0, note: 48 }, def));
        m.maps[0].toggle = false;
        assert_eq!(m.feed("test", &[0x90, 48, 100], 0.016), vec![Out::Set("morph", true)]);
        assert_eq!(m.feed("test", &[0x90, 48, 0], 0.016), vec![Out::Set("morph", false)]);
    }

    #[test]
    fn a_pad_fires_an_action_on_the_way_down_only() {
        let def = find("cam.random").unwrap();
        let mut m = Midi::new(Vec::new());
        m.maps.push(Mapping::new("test".into(), Msg::Note { ch: 0, note: 36 }, def));
        assert_eq!(m.feed("test", &[0x90, 36, 127], 0.016), vec![Out::Fire("cam.random")]);
        assert!(m.feed("test", &[0x90, 36, 0], 0.016).is_empty());
    }

    #[test]
    fn a_controller_scans_across_a_segmented_control() {
        let def = find("aa").unwrap();
        let mut m = Midi::new(Vec::new());
        m.maps.push(Mapping::new("test".into(), Msg::Cc { ch: 0, cc: 20 }, def));
        assert_eq!(m.feed("test", &[0xB0, 20, 0], 0.016), vec![Out::Value("aa", 0.0)]);
        assert_eq!(m.feed("test", &[0xB0, 20, 127], 0.016), vec![Out::Value("aa", 2.0)]);
    }

    #[test]
    fn learning_binds_the_next_message_and_replaces_what_it_lands_on() {
        let mut m = Midi::new(Vec::new());
        m.learn = Learn::Picking;
        m.pick("post_glow");
        m.feed("akai", &[0xB2, 13, 64], 0.016);
        assert_eq!(m.maps.len(), 1);
        assert_eq!(m.maps[0].param, "post_glow");
        assert_eq!(m.maps[0].msg, Msg::Cc { ch: 2, cc: 13 });
        assert_eq!(m.learn, Learn::Picking, "still mapping, ready for the next one");

        // the same message pointed somewhere else moves, rather than doubling up
        m.pick("post_fog");
        m.feed("akai", &[0xB2, 13, 64], 0.016);
        assert_eq!(m.maps.len(), 1);
        assert_eq!(m.maps[0].param, "post_fog");
    }

    #[test]
    fn messages_that_are_not_notes_or_controllers_are_ignored() {
        assert!(Msg::parse(&[0xF8]).is_none(), "clock");
        assert!(Msg::parse(&[0xE0, 0, 64]).is_none(), "pitch bend");
        assert!(Msg::parse(&[0xB0, 1]).is_none(), "truncated");
    }

    #[test]
    fn channels_and_notes_read_the_way_a_controller_prints_them() {
        assert_eq!(Msg::Cc { ch: 4, cc: 13 }.channel(), 5, "0-15 on the wire, 1-16 to a person");
        assert_eq!(note_name(60), "C3", "middle C, as Ableton names it");
        assert_eq!(note_name(72), "C4", "an octave up");
        assert_eq!(note_name(80), "G#4");
        assert_eq!(note_name(41), "F1");
        assert_eq!(note_name(0), "C-2", "the bottom of the range still reads");
    }

    #[test]
    fn a_mapping_survives_a_round_trip_through_the_file() {
        let mut m = one("lc_relief");
        m.maps[0].smooth = 0.4;
        m.maps[0].min = 0.9;
        m.maps[0].max = 0.1;
        let text = serde_json::to_string(&m.maps).unwrap();
        let back: Vec<Mapping> = serde_json::from_str(&text).unwrap();
        assert_eq!(back[0].param, "lc_relief");
        assert_eq!(back[0].msg, Msg::Cc { ch: 0, cc: 7 });
        assert_eq!((back[0].min, back[0].max, back[0].smooth), (0.9, 0.1, 0.4));
        assert!(!back[0].live, "and it waits for a message again rather than jumping");
    }

    #[test]
    fn a_file_from_an_older_build_still_opens() {
        let older = r#"[{"device":"x","msg":{"Cc":{"ch":1,"cc":9}},"param":"post_fog"}]"#;
        let back: Vec<Mapping> = serde_json::from_str(older).unwrap();
        assert_eq!(back[0].param, "post_fog");
        assert_eq!((back[0].min, back[0].max), (0.0, 1.0), "the fields it predates default");
    }

    #[test]
    fn every_parameter_has_its_own_name() {
        for (i, p) in PARAMS.iter().enumerate() {
            assert!(!PARAMS[..i].iter().any(|q| q.id == p.id), "{} is in the table twice", p.id);
        }
    }

    /// The console names parameters and `App` acts on them; this table is the only
    /// thing joining the two. An id in one and not the others is a control that
    /// looks mappable and quietly does nothing, which is worse than not offering it.
    #[test]
    fn the_console_and_the_app_agree_on_the_table() {
        let console = include_str!("ui.rs");
        let app = include_str!("app.rs");
        for p in PARAMS {
            let quoted = format!("\"{}\"", p.id);
            assert!(console.contains(&quoted), "{} is in the table but not in the console", p.id);
            // the favourites and the array switches are reached by prefix, not by
            // their whole id, so accept the prefix the dispatch actually matches on
            let stem = p.id.rsplit_once('.').map(|(a, _)| format!("\"{a}.")).unwrap_or(quoted);
            assert!(app.contains(&stem), "{} is in the table but the app never acts on it", p.id);
        }
    }

    #[test]
    fn the_console_never_names_a_parameter_that_does_not_exist() {
        let console = include_str!("ui.rs");
        for call in ["c.slider(", "c.seg(", "c.chip_toggle(", "c.action(", "c.target("] {
            for part in console.split(call).skip(1) {
                let Some(rest) = part.trim_start().strip_prefix('"') else { continue };
                let Some((id, _)) = rest.split_once('"') else { continue };
                assert!(find(id).is_some(), "the console maps {id}, which is not in the table");
            }
        }
    }
}
