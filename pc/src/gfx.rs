//! The wgpu layer: one instance per graphics API, two passes, three lazily built
//! scene pipelines.
//!
//! The whole struct is thrown away and rebuilt when the backend changes, which is
//! what makes Vulkan / DirectX 12 / OpenGL a live switch rather than a restart.

use crate::shaderpp;
use crate::state::{Backend, Present};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

/// Matches `struct Uniforms` in shaders/scene.wgsl. Scalars are grouped in fours so
/// every member lands where std140 expects it, with no implicit padding.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct Uniforms {
    pub res: [f32; 2],
    pub time: f32,
    pub roll: f32,
    pub eye: [f32; 4],
    pub tgt: [f32; 4],
    pub col_a: [f32; 4],
    pub col_b: [f32; 4],
    pub warp_on: [f32; 4],
    pub lc_on: [f32; 4],
    pub scale: f32,
    pub spike: f32,
    pub steps: f32,
    pub fov: f32,
    pub shift: f32,
    pub morph: f32,
    pub pulse: f32,
    pub scene: f32,
    pub thick: f32,
    pub warp_amt: f32,
    pub warp_freq: f32,
    pub lc_twirl: f32,
    pub lc_relief: f32,
    pub lc_freq: f32,
    pub lc_thick: f32,
    pub eps: f32,
    pub omega: f32,
    pub bound: f32,
    pub bound_pad: f32,
    pub warp_mode: f32,
    pub mat_smooth: f32,
    pub mat_metal: f32,
    pub post_exposure: f32,
    pub post_glow: f32,
    pub post_fog: f32,
    pub post_vignette: f32,
    pub post_grain: f32,
    pub post_pad: f32,
    pub bg_low: [f32; 4],
    pub bg_high: [f32; 4],
    pub rim_col: [f32; 4],
    pub spike_col: [f32; 4],
}

/// Matches `struct Post` in shaders/post.wgsl.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct PostUniforms {
    pub out_res: [f32; 2],
    pub texel: [f32; 2],
    /// 0 resolve, 1 FXAA, 2 upscale
    pub mode: f32,
    pub sharpen: f32,
    pub pad: [f32; 2],
}

/// What one call to [`Gfx::render`] managed. A skipped frame is normal — the window
/// is minimised, or the swapchain wants reconfiguring — and is not an error.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Frame {
    Drawn,
    Skipped,
}

/// What the post pass does with the marched image. Matches `P.mode` in post.wgsl.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PostMode {
    /// straight read, or a bilinear down-sample when the target is bigger
    Resolve = 0,
    Fxaa = 1,
    /// the march ran below the window on purpose: reconstruct it
    Upscale = 2,
}

impl PostMode {
    pub fn label(self) -> &'static str {
        match self {
            PostMode::Resolve => "resolve",
            PostMode::Fxaa => "FXAA",
            PostMode::Upscale => "upscale",
        }
    }
}

pub struct EguiFrame {
    pub jobs: Vec<egui::ClippedPrimitive>,
    pub delta: egui::TexturesDelta,
    pub pixels_per_point: f32,
}

pub struct Gfx {
    window: Arc<Window>,
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub format: wgpu::TextureFormat,

    pub backend: Backend,
    pub present: Present,
    pub adapter_name: String,
    pub driver: String,
    pub is_software: bool,
    pub present_modes: Vec<wgpu::PresentMode>,

    scene_layout: wgpu::BindGroupLayout,
    scene_pipes: [Option<wgpu::RenderPipeline>; 3],
    scene_failed: [bool; 3],
    uni_buf: wgpu::Buffer,
    scene_bind: wgpu::BindGroup,

    post_pipe: wgpu::RenderPipeline,
    post_layout: wgpu::BindGroupLayout,
    post_buf: wgpu::Buffer,
    /// the second post pass has its own uniforms: same pipeline, different source
    post_buf2: wgpu::Buffer,
    sampler: wgpu::Sampler,
    post_bind: Option<wgpu::BindGroup>,

    target: Option<wgpu::Texture>,
    target_view: Option<wgpu::TextureView>,
    pub target_size: (u32, u32),

    /// window-sized rung between the two post passes, so antialiasing can run on
    /// the reconstructed image rather than instead of it
    mid: Option<wgpu::Texture>,
    mid_view: Option<wgpu::TextureView>,
    mid_bind: Option<wgpu::BindGroup>,
    mid_size: (u32, u32),

    pub egui: egui_wgpu::Renderer,
    /// what the last frame actually did: "direct", "resolve", "FXAA", "upscale",
    /// or a reconstruction with antialiasing after it
    pub post_path: &'static str,
}

impl Gfx {
    pub fn new(window: Arc<Window>, backend: Backend, present: Present) -> Result<Gfx, String> {
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend.wgpu(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("no surface on {}: {e}", backend.label()))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("{} found no GPU: {e}", backend.label()))?;

        let info = adapter.get_info();
        let adapter_name =
            if info.name.is_empty() { "unknown".to_string() } else { info.name.clone() };
        let driver = format!("{} {}", info.driver, info.driver_info).trim().to_string();
        let is_software = matches!(info.device_type, wgpu::DeviceType::Cpu);

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cortical"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("{} would not open a device: {e}", backend.label()))?;

        let caps = surface.get_capabilities(&adapter);
        // The shader applies its own gamma, so an sRGB surface would apply it twice.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let present_modes = caps.present_modes.clone();
        let present_mode = pick_present(present, &present_modes);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        // ---- scene ----
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uni_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene uniforms"),
            contents: bytemuck::bytes_of(&Uniforms::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let scene_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bind"),
            layout: &scene_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uni_buf.as_entire_binding() }],
        });

        // ---- post ----
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let post_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("post uniforms"),
            contents: bytemuck::bytes_of(&PostUniforms::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let post_buf2 = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("post uniforms 2"),
            contents: bytemuck::bytes_of(&PostUniforms::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let post_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(shaderpp::POST_SRC.into()),
        });
        let post_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post pipeline layout"),
            bind_group_layouts: &[Some(&post_layout)],
            immediate_size: 0,
        });
        let post_pipe = make_pipeline(&device, &post_pl, &post_module, format, "post");

        let egui = egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());

        Ok(Gfx {
            window,
            _instance: instance,
            surface,
            device,
            queue,
            config,
            format,
            backend,
            present,
            adapter_name,
            driver,
            is_software,
            present_modes,
            scene_layout,
            scene_pipes: [None, None, None],
            scene_failed: [false; 3],
            uni_buf,
            scene_bind,
            post_pipe,
            post_layout,
            post_buf,
            post_buf2,
            sampler,
            post_bind: None,
            target: None,
            target_view: None,
            target_size: (0, 0),
            mid: None,
            mid_view: None,
            mid_bind: None,
            mid_size: (0, 0),
            egui,
            post_path: "resolve",
        })
    }

    /// One program per object, built the first time that object is shown. Each
    /// carries only its own SDF, which keeps the shader a third of the size and
    /// means opening the app compiles one object, not three.
    pub fn scene_pipe(&mut self, scene: usize) -> bool {
        if self.scene_pipes[scene].is_some() {
            return true;
        }
        if self.scene_failed[scene] {
            return false;
        }
        let src = shaderpp::scene_shader(scene);
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&self.scene_layout)],
            immediate_size: 0,
        });
        let pipe = make_pipeline(&self.device, &layout, &module, self.format, "scene");
        if let Some(err) = pollster::block_on(scope.pop()) {
            log::error!("object {scene} would not build: {err}");
            self.scene_failed[scene] = true;
            return false;
        }
        self.scene_pipes[scene] = Some(pipe);
        true
    }

    pub fn scene_available(&self, scene: usize) -> bool {
        !self.scene_failed[scene]
    }

    pub fn resize_surface(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 || (w == self.config.width && h == self.config.height) {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn set_present(&mut self, p: Present) {
        let mode = pick_present(p, &self.present_modes);
        self.present = p;
        if mode != self.config.present_mode {
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// True when the driver actually offers the mode that was asked for.
    pub fn present_honoured(&self, p: Present) -> bool {
        self.present_modes.contains(&p.wgpu()) || matches!(p, Present::Vsync) // AutoVsync always resolves to something
    }

    fn ensure_target(&mut self, rw: u32, rh: u32) {
        if self.target_size == (rw, rh) && self.target.is_some() {
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene target"),
            size: wgpu::Extent3d { width: rw.max(1), height: rh.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.post_bind = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post bind"),
            layout: &self.post_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.post_buf.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
        self.target = Some(tex);
        self.target_view = Some(view);
        self.target_size = (rw, rh);
    }

    fn ensure_mid(&mut self, w: u32, h: u32) {
        if self.mid_size == (w, h) && self.mid.is_some() {
            return;
        }
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("post rung"),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.mid_bind = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post rung bind"),
            layout: &self.post_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.post_buf2.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
        self.mid = Some(tex);
        self.mid_view = Some(view);
        self.mid_size = (w, h);
    }

    /// Draw one frame: the march, then the post pass, then the console over it.
    ///
    /// `rw`/`rh` is the size the scene is marched at and `base` is what it is meant
    /// to become — the resolution the console asked for. When the march already
    /// matches the window and no antialiasing is asked for, it goes straight to the
    /// screen and the post pass is skipped — a full-screen texture round trip the
    /// browser can never avoid.
    ///
    /// Otherwise the first post pass writes the *base* image, and a second one
    /// stretches that to the window, antialiasing it on the way when asked. Two
    /// passes is what lets the upscaler rebuild 1920x1080 out of 960x540 and only
    /// then meet the monitor, instead of reconstructing straight to the window and
    /// making the chosen resolution meaningless.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        scene: usize,
        uniforms: &Uniforms,
        rw: u32,
        rh: u32,
        base: (u32, u32),
        post: PostMode,
        then_fxaa: bool,
        sharpen: f32,
        egui_frame: EguiFrame,
    ) -> Frame {
        let (w, h) = (self.config.width, self.config.height);
        let (bw, bh) = base;
        let direct = post == PostMode::Resolve && !then_fxaa && rw == w && rh == h;
        // A reconstruction that is not already window sized needs somewhere to land
        // before it is stretched, and antialiasing after one needs the same rung.
        // Either way the rung is *base* sized, never window sized.
        let two_pass = !direct && (then_fxaa || (post == PostMode::Upscale && (bw, bh) != (w, h)));
        self.post_path = match (direct, post, two_pass, then_fxaa) {
            (true, ..) => "direct",
            (_, PostMode::Upscale, _, true) => "upscale+FXAA",
            (_, PostMode::Resolve, _, true) => "resolve+FXAA",
            (_, PostMode::Upscale, true, false) => "upscale+fit",
            (_, p, ..) => p.label(),
        };
        if !direct {
            self.ensure_target(rw, rh);
        }
        if two_pass {
            self.ensure_mid(bw, bh);
        }

        self.queue.write_buffer(&self.uni_buf, 0, bytemuck::bytes_of(uniforms));
        // `out_res` is the size being written, `texel` is 1 / the size being read.
        // The first pass writes the rung when there is one, and the window when the
        // base already is the window.
        let (o1w, o1h) = if two_pass { (bw, bh) } else { (w, h) };
        self.queue.write_buffer(
            &self.post_buf,
            0,
            bytemuck::bytes_of(&PostUniforms {
                out_res: [o1w as f32, o1h as f32],
                texel: [1.0 / rw as f32, 1.0 / rh as f32],
                mode: post as u32 as f32,
                sharpen,
                pad: [0.0; 2],
            }),
        );
        if two_pass {
            // the second pass reads the base-sized rung and stretches it to the
            // window, antialiasing on the way when that was asked for
            self.queue.write_buffer(
                &self.post_buf2,
                0,
                bytemuck::bytes_of(&PostUniforms {
                    out_res: [w as f32, h as f32],
                    texel: [1.0 / bw as f32, 1.0 / bh as f32],
                    mode: if then_fxaa { PostMode::Fxaa } else { PostMode::Resolve } as u32 as f32,
                    sharpen: 0.0,
                    pad: [0.0; 2],
                }),
            );
        }

        // egui hands over each texture change exactly once, so the upload has to
        // happen whether or not this frame reaches the screen — a dropped delta
        // leaves the console with no font atlas for the rest of the session.
        let EguiFrame { jobs, mut delta, pixels_per_point } = egui_frame;
        for (id, images) in &delta.set {
            for image in images {
                self.egui.update_texture(&self.device, &self.queue, *id, image);
            }
        }
        // Freeing has to happen at each of the three places the frame can end, and
        // the delta has to be emptied as well as read: `TexturesDelta` asserts on
        // drop that someone dealt with it, which a debug build turns into a panic on
        // the very first frame. A helper function would borrow all of `self`, so it
        // is a macro.
        macro_rules! free_textures {
            () => {
                for id in &delta.free {
                    self.egui.free_texture(id);
                }
                delta.clear();
            };
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.reconfigure();
                free_textures!();
                return Frame::Skipped;
            }
            _ => {
                free_textures!();
                return Frame::Skipped;
            }
        };
        let screen = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        {
            let view = if direct { &screen } else { self.target_view.as_ref().unwrap() };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("march"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(p) = &self.scene_pipes[scene] {
                pass.set_pipeline(p);
                pass.set_bind_group(0, &self.scene_bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }

        if !direct {
            let view = if two_pass { self.mid_view.as_ref().unwrap() } else { &screen };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.post_pipe);
            pass.set_bind_group(0, self.post_bind.as_ref().unwrap(), &[]);
            pass.draw(0..3, 0..1);
        }

        if two_pass {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post fit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &screen,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.post_pipe);
            pass.set_bind_group(0, self.mid_bind.as_ref().unwrap(), &[]);
            pass.draw(0..3, 0..1);
        }

        // ---- the console, over the top ----
        let desc = egui_wgpu::ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point };
        let extra = self.egui.update_buffers(&self.device, &self.queue, &mut enc, &jobs, &desc);
        {
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &screen,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.egui.render(&mut pass.forget_lifetime(), &jobs, &desc);
        }
        self.queue.submit(extra.into_iter().chain(std::iter::once(enc.finish())));
        free_textures!();
        self.window.pre_present_notify();
        self.queue.present(frame);
        Frame::Drawn
    }

    pub fn reconfigure(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }
}

fn pick_present(want: Present, have: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    let w = want.wgpu();
    match want {
        Present::Vsync => wgpu::PresentMode::AutoVsync,
        _ if have.contains(&w) => w,
        // asked for uncapped and the driver has no such thing: mailbox, then vsync
        _ if have.contains(&wgpu::PresentMode::Mailbox) => wgpu::PresentMode::Mailbox,
        _ => wgpu::PresentMode::AutoVsync,
    }
}

fn make_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vsMain"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fsMain"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Everything the shader is told about this frame. Kept out of `App` so it can be
/// checked without a window: the switches the console shows as buttons reach the
/// shader as 0 or 1, and two of them are gates — impulses off means no spike rate at
/// all, and the twirl step off means no twirl, whatever its slider says.
pub fn uniforms(
    st: &crate::state::State,
    cam: &crate::camera::Cam,
    rw: u32,
    rh: u32,
    steps: f32,
) -> Uniforms {
    let (a, b) = (st.col_a(), st.col_b());
    let f = |v: bool| if v { 1.0 } else { 0.0 };
    Uniforms {
        res: [rw as f32, rh as f32],
        time: st.clock,
        roll: cam.roll,
        eye: [cam.eye[0], cam.eye[1], cam.eye[2], 0.0],
        tgt: [cam.tgt[0], cam.tgt[1], cam.tgt[2], 0.0],
        col_a: [a[0], a[1], a[2], 0.0],
        col_b: [b[0], b[1], b[2], 0.0],
        warp_on: [f(st.warp_on[0]), f(st.warp_on[1]), f(st.warp_on[2]), 0.0],
        lc_on: [f(st.lc_on[0]), f(st.lc_on[1]), f(st.lc_on[2]), f(st.lc_on[3])],
        scale: st.density,
        spike: if st.spike_on { st.spike } else { 0.0 },
        steps,
        fov: st.fov,
        shift: st.shift,
        morph: st.m_clock,
        pulse: st.p_clock,
        scene: st.scene as f32,
        thick: st.thick,
        warp_amt: st.warp_amt,
        warp_freq: st.warp_freq,
        lc_twirl: if st.lc_on[4] { st.lc_twirl } else { 0.0 },
        lc_relief: st.lc_relief,
        lc_freq: st.lc_freq,
        lc_thick: st.lc_thick,
        eps: 1.0 / st.eps,
        omega: st.omega,
        bound: st.bound as f32,
        bound_pad: st.bound_pad,
        warp_mode: st.warp_mode as f32,
        mat_smooth: st.mat_smooth,
        mat_metal: st.mat_metal,
        post_exposure: st.post_exposure,
        post_glow: st.post_glow,
        post_fog: st.post_fog,
        post_vignette: st.post_vignette,
        post_grain: st.post_grain,
        post_pad: 0.0,
        bg_low: [st.bg_low[0], st.bg_low[1], st.bg_low[2], 0.0],
        bg_high: [st.bg_high[0], st.bg_high[1], st.bg_high[2], 0.0],
        rim_col: [st.rim_col[0], st.rim_col[1], st.rim_col[2], 0.0],
        spike_col: [st.spike_col[0], st.spike_col[1], st.spike_col[2], 0.0],
    }
}
