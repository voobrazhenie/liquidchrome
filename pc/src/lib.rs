//! Cortical Flythrough — the native build of the ray-marched page in index.html.

pub mod app;
pub mod bench;
pub mod camera;
pub mod gfx;
pub mod midi;
pub mod shaderpp;
pub mod state;
pub mod store;
pub mod ui;

use state::Backend;

/// Read `--backend vulkan|dx12|gl` off the command line, if it is there.
pub fn backend_arg() -> Option<Backend> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "--backend" || a == "-b")?;
    Backend::parse(args.get(i + 1)?)
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::new()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = app::App::new(backend_arg());
    event_loop.run_app(&mut app)?;
    Ok(())
}
