use cosmic::iced::wgpu;
use cosmic::iced_core::{mouse, Rectangle};
use cosmic::iced_widget::shader::{self, Viewport};
use cosmic_bg_config::{ShaderContent, ShaderLanguage, ShaderSource};
use std::borrow::Cow;
use std::time::Instant;

/// WGSL preamble prepended to user shaders.
/// Provides iResolution and iTime uniforms compatible with cosmic-bg shaders.
const WGSL_PREAMBLE: &str = r#"
// cosmic-bg live wallpaper uniforms
@group(0) @binding(0) var<uniform> iResolution: vec2f;
@group(0) @binding(1) var<uniform> iTime: f32;
"#;

/// Full-screen vertex shader.
const VERTEX_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen triangle strip (4 vertices)
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0),
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return out;
}
"#;

/// Error when loading or compiling a shader.
#[derive(Debug)]
pub enum ShaderError {
    Io(std::io::Error),
    UnsupportedLanguage(ShaderLanguage),
    Compilation(String),
}

impl std::fmt::Display for ShaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShaderError::Io(e) => write!(f, "Failed to read shader file: {}", e),
            ShaderError::UnsupportedLanguage(lang) => {
                write!(f, "Unsupported shader language: {:?}", lang)
            }
            ShaderError::Compilation(msg) => write!(f, "Shader compilation failed: {}", msg),
        }
    }
}

impl std::error::Error for ShaderError {}

/// Detect shader language from source.
fn detect_language(source: &ShaderSource) -> ShaderLanguage {
    if let ShaderContent::Path(path) = &source.shader {
        if path
            .extension()
            .map_or(false, |ext| ext == "glsl" || ext == "frag")
        {
            return ShaderLanguage::Glsl;
        }
    }
    source.language
}

/// Load and prepare shader code with preamble.
fn load_shader_code(source: &ShaderSource) -> Result<String, ShaderError> {
    let shader_code = match &source.shader {
        ShaderContent::Path(path) => std::fs::read_to_string(path).map_err(ShaderError::Io)?,
        ShaderContent::Code(code) => code.clone(),
    };

    let language = detect_language(source);
    if language == ShaderLanguage::Glsl {
        return Err(ShaderError::UnsupportedLanguage(ShaderLanguage::Glsl));
    }

    // Combine preamble with user shader
    Ok(format!("{}\n{}", WGSL_PREAMBLE, shader_code))
}

/// A shader program for rendering background shaders.
///
/// This implements iced's `shader::Program` trait to render shaders
/// directly in iced's wgpu context without CPU round-trips.
#[derive(Clone)]
pub struct BackgroundShaderProgram {
    /// Shader code wrapped in Arc for cheap cloning into primitives.
    shader_code: std::sync::Arc<str>,
    start_time: Instant,
}

impl BackgroundShaderProgram {
    /// Create a new background shader program from a shader source.
    ///
    /// Returns `None` if the shader cannot be loaded (file not found, GLSL, etc.)
    pub fn new(source: &ShaderSource) -> Option<Self> {
        match load_shader_code(source) {
            Ok(shader_code) => Some(Self {
                shader_code: shader_code.into(),
                start_time: Instant::now(),
            }),
            Err(e) => {
                tracing::error!("Failed to load shader: {}", e);
                None
            }
        }
    }
}

impl<Message> shader::Program<Message> for BackgroundShaderProgram {
    type State = ();
    type Primitive = BackgroundShaderPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        BackgroundShaderPrimitive {
            // Arc::clone is O(1) - just increments refcount
            shader_code: std::sync::Arc::clone(&self.shader_code),
            time: self.start_time.elapsed().as_secs_f32(),
        }
    }
}

/// Per-frame data for shader rendering.
#[derive(Debug)]
pub struct BackgroundShaderPrimitive {
    /// Shader code is only used for initial pipeline creation.
    /// After the pipeline is cached, this field is ignored.
    shader_code: std::sync::Arc<str>,
    time: f32,
}

impl shader::Primitive for BackgroundShaderPrimitive {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        storage: &mut shader::Storage,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let current_hash = hash_shader_code(&self.shader_code);

        // Check if we need to create or recreate the pipeline
        let needs_new_pipeline = if let Some(existing) = storage.get::<BackgroundShaderPipeline>() {
            // Recreate if shader code changed (different user selected)
            if existing.shader_hash != current_hash {
                tracing::info!(
                    "Shader changed (hash {} -> {}), recreating pipeline",
                    existing.shader_hash,
                    current_hash
                );
                true
            } else {
                false
            }
        } else {
            // No pipeline exists yet
            true
        };

        if needs_new_pipeline {
            tracing::info!(
                "Creating shader pipeline: format={:?}, shader_code_len={}, hash={}",
                format,
                self.shader_code.len(),
                current_hash
            );
            match BackgroundShaderPipeline::new(device, format, &self.shader_code) {
                Ok(pipeline) => {
                    tracing::info!("Shader pipeline created successfully");
                    storage.store(pipeline);
                }
                Err(e) => {
                    tracing::error!("Failed to create shader pipeline: {}", e);
                    return;
                }
            }
        }

        // Update uniforms with physical resolution
        // The shader's @builtin(position) fragCoord is in physical pixels,
        // so we must use physical_size to match.
        if let Some(pipeline) = storage.get_mut::<BackgroundShaderPipeline>() {
            let physical_size = viewport.physical_size();
            let resolution = [physical_size.width as f32, physical_size.height as f32];
            pipeline.update(queue, self.time, resolution);
        }
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &shader::Storage,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        if let Some(pipeline) = storage.get::<BackgroundShaderPipeline>() {
            pipeline.render(encoder, target, clip_bounds);
        } else {
            tracing::warn!("Shader pipeline not available in render()");
        }
    }
}

/// Cached wgpu pipeline and resources for shader rendering.
struct BackgroundShaderPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    resolution_buffer: wgpu::Buffer,
    time_buffer: wgpu::Buffer,
    /// Hash of the shader code to detect when shader changes
    shader_hash: u64,
}

/// Compute a simple hash of shader code for change detection.
fn hash_shader_code(code: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    hasher.finish()
}

impl BackgroundShaderPipeline {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader_code: &str,
    ) -> Result<Self, ShaderError> {
        // Create shader module
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cosmic-greeter: background shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_code)),
        });

        // Create vertex shader module
        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cosmic-greeter: fullscreen vertex shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(VERTEX_SHADER)),
        });

        // Create uniform buffers
        let resolution_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cosmic-greeter: iResolution buffer"),
            size: std::mem::size_of::<[f32; 2]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cosmic-greeter: iTime buffer"),
            size: std::mem::size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cosmic-greeter: shader bind group layout"),
            entries: &[
                // iResolution
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
                // iTime
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cosmic-greeter: shader bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: resolution_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: time_buffer.as_entire_binding(),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cosmic-greeter: shader pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cosmic-greeter: background shader pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_module,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            pipeline,
            bind_group,
            resolution_buffer,
            time_buffer,
            shader_hash: hash_shader_code(shader_code),
        })
    }

    fn update(&self, queue: &wgpu::Queue, time: f32, resolution: [f32; 2]) {
        queue.write_buffer(&self.time_buffer, 0, bytemuck::bytes_of(&time));
        queue.write_buffer(
            &self.resolution_buffer,
            0,
            bytemuck::cast_slice(&resolution),
        );
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("cosmic-greeter: background shader pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..4, 0..1); // 4 vertices for triangle strip fullscreen quad
    }
}

unsafe impl Send for BackgroundShaderPipeline {}
unsafe impl Sync for BackgroundShaderPipeline {}
