//! Emit the ported scene shader as GLSL ES 300, the way wgpu's OpenGL backend does.
//!
//! Used by tools/parity.mjs: the emitted shader and the original WebGL shader from
//! index.html are rendered side by side in headless Chromium and the two images
//! are compared, which is how the port is checked without a GPU here.
//!
//!     cargo run --example emit_glsl -- <0|1|2> <out.frag>

use naga::back::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scene: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let out = args.get(2).cloned().unwrap_or_else(|| "scene.frag".into());

    let src = cortical_flythrough::shaderpp::scene_shader(scene);
    let module = naga::front::wgsl::parse_str(&src).expect("parse");
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .expect("validate");

    let options = glsl::Options {
        version: glsl::Version::Embedded { version: 300, is_webgl: true },
        // the same flag wgpu's GL backend sets: clip space is flipped so that
        // gl_FragCoord lands where @builtin(position) does
        writer_flags: glsl::WriterFlags::ADJUST_COORDINATE_SPACE,
        ..Default::default()
    };
    let pipeline = glsl::PipelineOptions {
        shader_stage: naga::ShaderStage::Fragment,
        entry_point: "fsMain".to_string(),
        multiview: None,
    };
    let mut text = String::new();
    let mut w = glsl::Writer::new(
        &mut text,
        &module,
        &info,
        &options,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .expect("writer");
    w.write().expect("write");
    std::fs::write(&out, &text).expect("write file");
    eprintln!("{out}: {} bytes", text.len());
}
