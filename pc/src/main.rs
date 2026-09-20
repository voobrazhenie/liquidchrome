// A release build has no console behind it; a debug build keeps one for the log.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// A Legion runs two GPUs. These two exported symbols are what the NVIDIA and AMD
// drivers look for to hand the process the discrete one rather than the iGPU.
#[cfg(windows)]
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static NvOptimusEnablement: u32 = 1;
#[cfg(windows)]
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static AmdPowerXpressRequestHighPerformance: u32 = 1;

fn main() {
    env_logger::init();
    if let Err(e) = cortical_flythrough::run() {
        eprintln!("cortical-flythrough: {e}");
        std::process::exit(1);
    }
}
