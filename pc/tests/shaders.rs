//! What this replaces: there is no GPU in the build container, so the shaders are
//! checked the way a driver would check them. naga parses and validates the WGSL,
//! then the SPIR-V, HLSL and GLSL backends are run over it — the same three
//! translations wgpu performs for Vulkan, DirectX 12 and OpenGL. A shader that
//! survives all three is one all three backends can consume.

use naga::back;
use naga::valid::{Capabilities, ValidationFlags, Validator};

fn validate(label: &str, src: &str) -> (naga::Module, naga::valid::ModuleInfo) {
    let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| {
        panic!("{label}: WGSL parse failed:\n{}", e.emit_to_string(src));
    });
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{label}: WGSL validation failed: {e:?}"));
    (module, info)
}

fn to_spirv(label: &str, module: &naga::Module, info: &naga::valid::ModuleInfo) {
    let opts = back::spv::Options::default();
    let words = back::spv::write_vec(module, info, &opts, None)
        .unwrap_or_else(|e| panic!("{label}: SPIR-V (Vulkan) backend failed: {e:?}"));
    assert!(words.len() > 32, "{label}: SPIR-V output is suspiciously short");
}

fn to_hlsl(label: &str, module: &naga::Module, info: &naga::valid::ModuleInfo) {
    let opts = back::hlsl::Options::default();
    let pipeline = back::hlsl::PipelineOptions { entry_point: None };
    let mut src = String::new();
    let mut w = back::hlsl::Writer::new(&mut src, &opts, &pipeline);
    w.write(module, info, None)
        .unwrap_or_else(|e| panic!("{label}: HLSL (DirectX 12) backend failed: {e:?}"));
    assert!(src.contains("float"), "{label}: HLSL output looks empty");
}

fn to_glsl(
    label: &str,
    module: &naga::Module,
    info: &naga::valid::ModuleInfo,
    stage: naga::ShaderStage,
    entry: &str,
) {
    let opts =
        back::glsl::Options { version: back::glsl::Version::Desktop(410), ..Default::default() };
    let pipeline = back::glsl::PipelineOptions {
        shader_stage: stage,
        entry_point: entry.to_string(),
        multiview: None,
    };
    let mut src = String::new();
    let mut w = back::glsl::Writer::new(
        &mut src,
        module,
        info,
        &opts,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .unwrap_or_else(|e| panic!("{label}: GLSL (OpenGL) writer failed: {e:?}"));
    w.write().unwrap_or_else(|e| panic!("{label}: GLSL (OpenGL) backend failed: {e:?}"));
    assert!(src.contains("void main"), "{label}: GLSL output has no entry point");
}

fn check(label: &str, src: &str) {
    let (module, info) = validate(label, src);
    to_spirv(label, &module, &info);
    to_hlsl(label, &module, &info);
    to_glsl(label, &module, &info, naga::ShaderStage::Vertex, "vsMain");
    to_glsl(label, &module, &info, naga::ShaderStage::Fragment, "fsMain");
}

#[test]
fn every_object_builds_on_every_backend() {
    for (i, define) in cortical_flythrough::shaderpp::SCENE_DEFINES.iter().enumerate() {
        let src = cortical_flythrough::shaderpp::scene_shader(i);
        check(define, &src);
    }
}

#[test]
fn post_pass_builds_on_every_backend() {
    check("post", cortical_flythrough::shaderpp::POST_SRC);
}

/// The object being built is the only SDF in its shader — that is the whole point of
/// the split, and it is what keeps each pipeline a third of the size.
#[test]
fn each_object_carries_only_its_own_sdf() {
    let brain = cortical_flythrough::shaderpp::scene_shader(0);
    assert!(brain.contains("fn shell"));
    assert!(!brain.contains("fn liquid") && !brain.contains("fn neuron"));

    let neuron = cortical_flythrough::shaderpp::scene_shader(1);
    assert!(neuron.contains("fn neuron"));
    assert!(!neuron.contains("fn shell") && !neuron.contains("fn liquid"));

    let chrome = cortical_flythrough::shaderpp::scene_shader(2);
    assert!(chrome.contains("fn liquid"));
    assert!(!chrome.contains("fn shell") && !chrome.contains("fn neuron"));
}

/// The uniform buffer is written by Rust and read by the shader. If the two ever
/// disagree about its shape the picture is garbage rather than broken, so the size
/// naga computes for the shader's struct is checked against the Rust one.
#[test]
fn uniform_block_matches_the_rust_struct() {
    let src = cortical_flythrough::shaderpp::scene_shader(0);
    let module = naga::front::wgsl::parse_str(&src).expect("parse");
    let ctx = module.to_ctx();
    let (_, var) = module
        .global_variables
        .iter()
        .find(|(_, v)| v.space == naga::AddressSpace::Uniform)
        .expect("the scene shader has a uniform block");
    let size = module.types[var.ty].inner.size(ctx) as usize;
    assert_eq!(
        size,
        std::mem::size_of::<cortical_flythrough::gfx::Uniforms>(),
        "shaders/scene.wgsl and gfx::Uniforms disagree about the uniform block"
    );

    let post =
        naga::front::wgsl::parse_str(cortical_flythrough::shaderpp::POST_SRC).expect("parse");
    let pctx = post.to_ctx();
    let (_, pvar) = post
        .global_variables
        .iter()
        .find(|(_, v)| v.space == naga::AddressSpace::Uniform)
        .expect("the post shader has a uniform block");
    assert_eq!(
        post.types[pvar.ty].inner.size(pctx) as usize,
        std::mem::size_of::<cortical_flythrough::gfx::PostUniforms>()
    );
}

/// Field order matters as much as the total size: the WGSL members and the Rust
/// members have to line up name for name, in order.
#[test]
fn uniform_fields_line_up() {
    let src = cortical_flythrough::shaderpp::scene_shader(0);
    let block = src
        .split("struct Uniforms {")
        .nth(1)
        .and_then(|s| s.split("};").next())
        .expect("struct Uniforms");
    let wgsl: Vec<String> = block
        .lines()
        .filter_map(|l| l.trim().split(':').next().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty() && !s.starts_with("//"))
        .collect();
    // the same members, in the same order, as gfx::Uniforms
    let rust = [
        "res",
        "time",
        "roll",
        "eye",
        "tgt",
        "colA",
        "colB",
        "warpOn",
        "lcOn",
        "scale",
        "spike",
        "steps",
        "fov",
        "shift",
        "morph",
        "pulse",
        "scene",
        "thick",
        "warpAmt",
        "warpFreq",
        "lcTwirl",
        "lcRelief",
        "lcFreq",
        "lcThick",
        "eps",
        "omega",
        "bound",
        "boundPad",
        "warpMode",
        "matSmooth",
        "matMetal",
        "postExposure",
        "postGlow",
        "postFog",
        "postVignette",
        "postGrain",
        "postPad",
        "bgLow",
        "bgHigh",
        "rimCol",
        "spikeCol",
    ];
    assert_eq!(wgsl, rust, "shaders/scene.wgsl uniform members drifted from gfx::Uniforms");
}
