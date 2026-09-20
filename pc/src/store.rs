//! Saved settings, named templates and the six viewpoints — on this computer only.
//!
//! The page keeps a browser copy and a Firestore document; the native build keeps
//! neither. Two JSON files under the user's config directory
//! (`%APPDATA%\CorticalFlythrough\` on Windows) are the whole store, and nothing
//! here touches the network. Every failure is reported as text the console can show
//! and is never fatal.

use crate::state::State;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const SETTINGS_FILE: &str = "settings.json";
pub const TEMPLATES_FILE: &str = "templates.json";
pub const MIDI_FILE: &str = "midi.json";
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct Saved {
    pub version: u32,
    pub saved_at: u64,
    pub data: State,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Template {
    pub name: String,
    pub saved_at: u64,
    pub data: State,
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// The folder the program is running from. Settings live next to the binary rather
/// than under `%APPDATA%`, so the whole thing — app, settings, templates, mappings —
/// copies to another machine or a USB stick as one folder and opens on the same
/// picture. There is no fallback by design: a read-only folder makes saving fail
/// with a message rather than quietly writing somewhere the user will not find.
pub fn config_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.parent().map(|p| p.to_path_buf())
}

pub fn settings_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join(SETTINGS_FILE))
}

pub fn templates_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join(TEMPLATES_FILE))
}

pub fn midi_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join(MIDI_FILE))
}

/// Write through a temporary file and rename, so a crash mid-write cannot leave a
/// half-written settings file behind.
fn write_atomic(path: &PathBuf, text: &str) -> Result<(), String> {
    let dir = path.parent().ok_or("no parent directory")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    // Windows will not rename onto an existing file
    let _ = std::fs::remove_file(path);
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn load_settings() -> Option<State> {
    let path = settings_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let saved: Saved = serde_json::from_str(&text).ok()?;
    Some(saved.data)
}

pub fn save_settings(st: &State) -> Result<PathBuf, String> {
    let path = settings_path().ok_or("no config directory")?;
    let saved = Saved { version: VERSION, saved_at: now_ms(), data: st.clone() };
    let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
    write_atomic(&path, &text)?;
    Ok(path)
}

pub fn clear_settings() -> Result<(), String> {
    let path = settings_path().ok_or("no config directory")?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn load_templates() -> Vec<Template> {
    let Some(path) = templates_path() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_templates(list: &[Template]) -> Result<(), String> {
    let path = templates_path().ok_or("no config directory")?;
    let text = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    write_atomic(&path, &text)
}

/// MIDI mappings are their own file, not part of `State`. Loading a template
/// replaces every setting; it must not also silently re-wire the controller.
pub fn load_midi() -> Vec<crate::midi::Mapping> {
    let Some(path) = midi_path() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_midi(list: &[crate::midi::Mapping]) -> Result<(), String> {
    let path = midi_path().ok_or("no config directory")?;
    let text = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    write_atomic(&path, &text)
}

pub fn clear_midi() -> Result<(), String> {
    let path = midi_path().ok_or("no config directory")?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Save the report next to the settings, so a run can be kept without the clipboard.
pub fn save_report(text: &str) -> Result<PathBuf, String> {
    let dir = config_dir().ok_or("no config directory")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("benchmark-{}.txt", now_ms()));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

// ---- the web page's own settings ---------------------------------------------
//
// The page saves a different shape: camelCase names, colours as "#RRGGBB", and
// switches as 0/1 rather than true/false. Reading it here is what lets a fresh
// install open on the same picture, the same object and the same six views as the
// browser — without the app ever going near the network. The snapshot below was
// taken from the page's saved settings and ships with the binary; a newer export
// can be dropped in at any time with `--import <file>`.

/// The page's settings as they were when this build was made.
pub const WEB_DEFAULT: &str = include_str!("../presets/web-default.json");

fn f32_at(v: &serde_json::Value, key: &str) -> Option<f32> {
    v.get(key)?.as_f64().map(|x| x as f32)
}

fn usize_at(v: &serde_json::Value, key: &str) -> Option<usize> {
    v.get(key)?.as_f64().map(|x| x as usize)
}

/// The page writes 0/1 for switches and `true`/`false` for a couple of them.
fn bool_at(v: &serde_json::Value, key: &str) -> Option<bool> {
    let x = v.get(key)?;
    x.as_bool().or_else(|| x.as_f64().map(|n| n != 0.0))
}

fn bools_at<const N: usize>(v: &serde_json::Value, key: &str, out: &mut [bool; N]) {
    if let Some(a) = v.get(key).and_then(|x| x.as_array()) {
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(x) = a.get(i) {
                *slot = x.as_bool().or_else(|| x.as_f64().map(|n| n != 0.0)).unwrap_or(*slot);
            }
        }
    }
}

fn vec3_at(v: &serde_json::Value, key: &str) -> Option<[f32; 3]> {
    let a = v.get(key)?.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}

/// `#RRGGBB` as the shader wants it: 0..1 per channel, no colour-space conversion,
/// exactly what the page does with `parseInt(hex)/255`.
fn hex(s: &str) -> Option<[f32; 3]> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() < 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

/// Read the page's settings, in any of the three shapes they come in: the bare
/// object, the browser's `{data, at}` wrapper, or the cloud document that carries
/// the whole thing as one JSON string.
pub fn from_web_json(text: &str) -> Result<State, String> {
    let root: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;

    // the cloud document keeps the settings as a string inside `fields.json`
    let unwrapped = root
        .pointer("/fields/json/stringValue")
        .and_then(|s| s.as_str())
        .map(|s| serde_json::from_str::<serde_json::Value>(s).map_err(|e| e.to_string()))
        .transpose()?;
    let root = unwrapped.unwrap_or(root);
    // the browser's own copy wraps it once more
    let w = match root.get("data") {
        Some(d) if d.is_object() => d.clone(),
        _ => root,
    };
    if !w.is_object() {
        return Err("this is not a settings file from the page".into());
    }

    let mut st = State::default();
    if let Some(v) = usize_at(&w, "scene") {
        st.scene = v.min(2);
    }
    if let Some(v) = usize_at(&w, "mode") {
        st.mode = v.min(2);
    }
    if let Some(v) = usize_at(&w, "aa") {
        st.aa = v.min(2);
    }
    if let Some(v) = usize_at(&w, "resPin") {
        st.res_pin = v.min(3);
    }
    if let Some(v) = usize_at(&w, "bound") {
        st.bound = v.min(2);
    }
    if let Some(v) = usize_at(&w, "warpMode") {
        st.warp_mode = v.min(1);
    }
    if let Some(v) = bool_at(&w, "fly") {
        st.fly = v;
    }
    if let Some(v) = bool_at(&w, "stepsPin") {
        st.steps_pin = v;
    }
    if let Some(v) = bool_at(&w, "spikeOn") {
        st.spike_on = v;
    }
    if let Some(v) = bool_at(&w, "running") {
        st.running = v;
    }
    if let Some(v) = bool_at(&w, "morph") {
        st.morph = v;
    }
    for (key, slot) in [
        ("boundPad", &mut st.bound_pad),
        ("omega", &mut st.omega),
        ("eps", &mut st.eps),
        ("steps", &mut st.steps),
        ("warpAmt", &mut st.warp_amt),
        ("warpFreq", &mut st.warp_freq),
        ("lcRelief", &mut st.lc_relief),
        ("lcFreq", &mut st.lc_freq),
        ("lcThick", &mut st.lc_thick),
        ("lcTwirl", &mut st.lc_twirl),
        ("density", &mut st.density),
        ("spike", &mut st.spike),
        ("thick", &mut st.thick),
        ("zoom", &mut st.zoom),
        ("flySpeed", &mut st.fly_speed),
        ("flyYaw", &mut st.fly_yaw),
        ("flyPitch", &mut st.fly_pitch),
    ] {
        if let Some(v) = f32_at(&w, key) {
            *slot = v;
        }
    }
    bools_at(&w, "warpOn", &mut st.warp_on);
    bools_at(&w, "lcOn", &mut st.lc_on);
    if let Some(v) = vec3_at(&w, "flyPos") {
        st.fly_pos = v;
    }
    if let Some(rows) = w.get("colors").and_then(|c| c.as_array()) {
        for (i, row) in rows.iter().take(3).enumerate() {
            if let Some(pair) = row.as_array() {
                for (j, c) in pair.iter().take(2).enumerate() {
                    if let Some(rgb) = c.as_str().and_then(hex) {
                        st.colors[i][j] = rgb;
                    }
                }
            }
        }
    }
    if let Some(list) = w.get("favs").and_then(|f| f.as_array()) {
        for (i, f) in list.iter().take(6).enumerate() {
            st.favs[i] = fav_from_web(f);
        }
    }
    if let Some(g) = w.get("groups").and_then(|g| g.as_array()) {
        for (i, open) in g.iter().enumerate() {
            let on = open.as_bool().or_else(|| open.as_f64().map(|n| n != 0.0)).unwrap_or(true);
            st.set_group(i, on);
        }
    }
    Ok(st)
}

fn fav_from_web(f: &serde_json::Value) -> Option<crate::state::Fav> {
    if !f.is_object() {
        return None;
    }
    Some(crate::state::Fav {
        fly: bool_at(f, "fly").unwrap_or(false),
        pos: vec3_at(f, "pos")?,
        yaw: f32_at(f, "yaw").unwrap_or(0.0),
        pitch: f32_at(f, "pitch").unwrap_or(0.0),
        mode: usize_at(f, "mode").unwrap_or(0).min(2),
        clock: f32_at(f, "clock").unwrap_or(0.0),
        az: f32_at(f, "az").unwrap_or(0.0),
        el: f32_at(f, "el").unwrap_or(0.0),
        zoom: f32_at(f, "zoom").unwrap_or(1.0),
    })
}

/// The settings the page was last left on, as shipped with this build.
pub fn web_default() -> State {
    from_web_json(WEB_DEFAULT).unwrap_or_default()
}

pub fn import_web_file(path: &str) -> Result<State, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    from_web_json(&text)
}
