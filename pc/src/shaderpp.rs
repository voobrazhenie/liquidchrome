//! A three-line preprocessor.
//!
//! WGSL has none, and the scene shader needs the same `#ifdef` split the WebGL page
//! uses: one pipeline per object, each carrying only its own SDF. Lines of the form
//! `//#ifdef NAME` … `//#endif` are kept when NAME is the object being built and
//! dropped otherwise. Nesting is allowed; anything else is passed through untouched.

/// Strip every `//#ifdef` block whose name is not `define`.
pub fn preprocess(src: &str, define: &str) -> Result<String, String> {
    let mut out = String::with_capacity(src.len());
    // one entry per open block: whether its lines are being kept
    let mut stack: Vec<bool> = Vec::new();

    for (n, line) in src.lines().enumerate() {
        let t = line.trim_start();
        if let Some(name) = t.strip_prefix("//#ifdef ") {
            let keeping = stack.iter().all(|k| *k);
            stack.push(keeping && name.trim() == define);
            continue;
        }
        if t.starts_with("//#endif") {
            if stack.pop().is_none() {
                return Err(format!("line {}: //#endif without //#ifdef", n + 1));
            }
            continue;
        }
        if stack.iter().all(|k| *k) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !stack.is_empty() {
        return Err(format!("{} unclosed //#ifdef block(s)", stack.len()));
    }
    Ok(out)
}

/// The three objects, in the order the console shows them.
pub const SCENE_DEFINES: [&str; 3] = ["SCENE_BRAIN", "SCENE_NEURON", "SCENE_CHROME"];

pub const SCENE_SRC: &str = include_str!("../shaders/scene.wgsl");
pub const POST_SRC: &str = include_str!("../shaders/post.wgsl");

/// The scene shader for one object, ready to hand to `create_shader_module`.
pub fn scene_shader(scene: usize) -> String {
    preprocess(SCENE_SRC, SCENE_DEFINES[scene.min(2)]).expect("scene.wgsl preprocessor")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_named_block() {
        let src = "a\n//#ifdef X\nx\n//#endif\n//#ifdef Y\ny\n//#endif\nb\n";
        assert_eq!(preprocess(src, "X").unwrap(), "a\nx\nb\n");
        assert_eq!(preprocess(src, "Y").unwrap(), "a\ny\nb\n");
        assert_eq!(preprocess(src, "Z").unwrap(), "a\nb\n");
    }

    #[test]
    fn unbalanced_blocks_are_an_error() {
        assert!(preprocess("//#ifdef X\n", "X").is_err());
        assert!(preprocess("//#endif\n", "X").is_err());
    }
}
