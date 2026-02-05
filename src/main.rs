use std::{iter, mem, sync::Arc};

use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

mod my3d_lib;
mod obj_parser;


use my3d_lib::*;
use glam::Vec3A;
use wgpu::util::DeviceExt;

// GPU-friendly structures (must be 16-byte aligned)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuCamera {
    position: [f32; 3],
    _padding1: f32,
    forward: [f32; 3],
    _padding2: f32,
    right: [f32; 3],
    _padding3: f32,
    up: [f32; 3],
    _padding4: f32,
    focal_distance: f32,
    aperture_radius: f32,
    aspect_ratio: f32,
    frame: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuMaterial {
    albedo: [f32; 3],
    roughness: f32,
    emission: [f32; 3],
    _padding: f32,
}

impl From<Material> for GpuMaterial {
    fn from(mat: Material) -> Self {
        Self {
            albedo: [mat.albedo.x, mat.albedo.y, mat.albedo.z],
            roughness: mat.roughness,
            emission: [mat.emission.x, mat.emission.y, mat.emission.z],
            _padding: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    _padding: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuFace {
    indices: [u32; 3],
    material_idx: u32,
    normal0: [f32; 3],
    _padding1: f32,
    normal1: [f32; 3],
    _padding2: f32,
    normal2: [f32; 3],
    _padding3: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuSceneInfo {
    num_faces: u32,
    num_materials: u32,
    _padding: [u32; 2],
}

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    window: Arc<Window>,

    // Raytracing pipeline
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,

    // Textures
    render_texture: wgpu::Texture,
    render_texture_view: wgpu::TextureView,

    accumulation_texture_a: wgpu::Texture,
    accumulation_texture_a_view: wgpu::TextureView,
    accumulation_texture_b: wgpu::Texture,
    accumulation_texture_b_view: wgpu::TextureView,

    // Track which is current
    accumulation_swap: bool,


    // Buffers
    camera_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    face_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    scene_info_buffer: wgpu::Buffer,
    rand_seed_buffer: wgpu::Buffer,
    sample_count_buffer: wgpu::Buffer,

    // Bind groups
    render_bind_group: wgpu::BindGroup,

    // Camera state
    camera_pos: Vec3A,
    yaw: f32,
    pitch: f32,
    forward: Vec3A,
    right: Vec3A,
    up: Vec3A,
    focal_distance: f32,
    aperture_radius: f32,

    // Input state
    keys_down: std::collections::HashSet<KeyCode>,
    mouse_delta: (f32, f32),
    input_locked: bool,

    // Frame counter
    frame: u32,
    sample_count: u32,

    // Scene data
    num_faces: u32,
    num_materials: u32,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };

        // Load the scene
        let world: World = generate_map();

        // Convert to GPU format
        let mut gpu_vertices = Vec::new();
        let mut gpu_faces = Vec::new();
        let mut gpu_materials = Vec::new();

        for mesh in &world.baked_meshes {
            let vertex_offset = gpu_vertices.len() as u32;

            // Add vertices
            for vert in &mesh.vertices {
                gpu_vertices.push(GpuVertex {
                    position: [vert.x, vert.y, vert.z],
                    _padding: 0.0,
                });
            }

            // Add faces
            for face in &mesh.faces {
                gpu_faces.push(GpuFace {
                    indices: [
                        face.indices[0] as u32 + vertex_offset,
                        face.indices[1] as u32 + vertex_offset,
                        face.indices[2] as u32 + vertex_offset,
                    ],
                    material_idx: (face.material_idx + gpu_materials.len()) as u32,
                    normal0: [face.normals[0].x, face.normals[0].y, face.normals[0].z],
                    _padding1: 0.0,
                    normal1: [face.normals[1].x, face.normals[1].y, face.normals[1].z],
                    _padding2: 0.0,
                    normal2: [face.normals[2].x, face.normals[2].y, face.normals[2].z],
                    _padding3: 0.0,
                });
            }

            // Add materials (this will duplicate, but keeps indexing simple)
            for mat in &mesh.materials {
                gpu_materials.push(GpuMaterial::from(*mat));
            }
        }

        let num_faces = gpu_faces.len() as u32;
        let num_materials = gpu_materials.len() as u32;

        println!("Loaded scene: {} vertices, {} faces, {} materials",
                 gpu_vertices.len(), num_faces, num_materials);

        // Create buffers
        use wgpu::util::DeviceExt;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&gpu_vertices),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let face_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Face Buffer"),
            contents: bytemuck::cast_slice(&gpu_faces),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material Buffer"),
            contents: bytemuck::cast_slice(&gpu_materials),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let scene_info = GpuSceneInfo {
            num_faces,
            num_materials,
            _padding: [0; 2],
        };

        let scene_info_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene Info Buffer"),
            contents: bytemuck::cast_slice(&[scene_info]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let rand_seed_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rand Seed Buffer"),
            contents: bytemuck::cast_slice(&[0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sample_count_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sample count Buffer"),
            contents: bytemuck::cast_slice(&[0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Camera setup
        let camera_pos = Vec3A::new(0.0, 0.0, 0.0);
        let yaw = 0.0f32;
        let pitch = 0.0f32;

        let mut forward = Vec3A::default();
        forward.x = yaw.sin() * pitch.cos();
        forward.y = yaw.cos() * pitch.cos();
        forward.z = pitch.sin();

        let world_up = Vec3A::new(0.0, 0.0, 1.0);
        let right = forward.cross(world_up).normalize();
        let up = right.cross(forward).normalize();

        let aspect_ratio = size.width as f32 / size.height as f32;

        let gpu_camera = GpuCamera {
            position: [camera_pos.x, camera_pos.y, camera_pos.z],
            _padding1: 0.0,
            forward: [forward.x, forward.y, forward.z],
            _padding2: 0.0,
            right: [right.x, right.y, right.z],
            _padding3: 0.0,
            up: [up.x, up.y, up.z],
            _padding4: 0.0,
            focal_distance: 4.0,
            aperture_radius: 0.05,
            aspect_ratio,
            frame: 0,
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[gpu_camera]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create textures
        let texture_size = wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        };

        let render_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let render_texture_view = render_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create first accumulation texture
        let accumulation_texture_a = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Accumulation Texture A"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let accumulation_texture_a_view = accumulation_texture_a.create_view(&wgpu::TextureViewDescriptor::default());

        // Create second accumulation texture
        let accumulation_texture_b = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Accumulation Texture B"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let accumulation_texture_b_view = accumulation_texture_b.create_view(&wgpu::TextureViewDescriptor::default());


        // Load shaders
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/raytracer.wgsl").into()
            ),
        });

        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/display.wgsl").into()),
        });

        // Create bind group layouts
        let compute_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                // Camera
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Scene info
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Vertices
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Faces
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Materials
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Render texture
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Accumulation texture read
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Accumulation texture write
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Random seed
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Sample count
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create compute pipeline
        let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[&compute_bind_group_layout],
            immediate_size: 0,
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Create render pipeline for displaying the texture
        let render_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: &render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&render_texture_view),
                },
            ],
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&render_bind_group_layout],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });


        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
            compute_pipeline,
            render_pipeline,
            render_texture,
            render_texture_view,
            accumulation_texture_a,
            accumulation_texture_a_view,
            accumulation_texture_b,
            accumulation_texture_b_view,
            accumulation_swap: false,
            camera_buffer,
            vertex_buffer,
            face_buffer,
            material_buffer,
            scene_info_buffer,
            rand_seed_buffer,
            sample_count_buffer,
            render_bind_group,
            camera_pos,
            yaw,
            pitch,
            forward,
            right,
            up,
            focal_distance: 4.0,
            aperture_radius: 0.05,
            keys_down: std::collections::HashSet::new(),
            mouse_delta: (0.0, 0.0),
            input_locked: false,
            frame: 0,
            sample_count: 0,
            num_faces,
            num_materials,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;

            // Recreate textures
            let texture_size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };

            self.render_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Render Texture"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            self.render_texture_view = self.render_texture.create_view(&wgpu::TextureViewDescriptor::default());

            // Recreate both accumulation textures
            self.accumulation_texture_a = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Accumulation Texture A"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });

            self.accumulation_texture_a_view = self.accumulation_texture_a.create_view(&wgpu::TextureViewDescriptor::default());

            self.accumulation_texture_b = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Accumulation Texture B"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });

            self.accumulation_texture_b_view = self.accumulation_texture_b.create_view(&wgpu::TextureViewDescriptor::default());

            // Reset swap state
            self.accumulation_swap = false;

            // Update render bind group (for display)
            let render_bind_group_layout = self.render_pipeline.get_bind_group_layout(0);
            self.render_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render Bind Group"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.render_texture_view),
                    },
                ],
            });

            // Don't need to recreate compute_bind_group here since we do it every frame

            // Reset accumulation
            self.sample_count = 0;
        }
    }

    fn update(&mut self, dt: f32) {
        let speed = 2.0;
        let mouse_sensitivity = 0.002;

        // Update camera rotation
        self.yaw -= self.mouse_delta.0 * mouse_sensitivity;
        self.pitch -= self.mouse_delta.1 * mouse_sensitivity;
        self.mouse_delta = (0.0, 0.0);

        // Clamp pitch
        self.pitch = self.pitch.clamp(-std::f32::consts::FRAC_PI_2 + 0.01, std::f32::consts::FRAC_PI_2 - 0.01);

        // Update forward vector
        self.forward.x = self.yaw.sin() * self.pitch.cos();
        self.forward.y = self.yaw.cos() * self.pitch.cos();
        self.forward.z = self.pitch.sin();

        let world_up = Vec3A::new(0.0, 0.0, 1.0);
        self.right = self.forward.cross(world_up).normalize();
        self.up = self.right.cross(self.forward).normalize();

        if !self.input_locked {
            let mut moved = false;

            // Movement
            let amount = speed * dt;
            if self.keys_down.contains(&KeyCode::KeyW) {
                self.camera_pos += self.forward * amount;
                moved = true;
            }
            if self.keys_down.contains(&KeyCode::KeyS) {
                self.camera_pos -= self.forward * amount;
                moved = true;
            }
            if self.keys_down.contains(&KeyCode::KeyD) {
                self.camera_pos += self.right * amount;
                moved = true;
            }
            if self.keys_down.contains(&KeyCode::KeyA) {
                self.camera_pos -= self.right * amount;
                moved = true;
            }
            if self.keys_down.contains(&KeyCode::Space) {
                self.camera_pos -= self.up * amount;
                moved = true;
            }
            if self.keys_down.contains(&KeyCode::ShiftLeft) {
                self.camera_pos += self.up * amount;
                moved = true;
            }

            if moved {
                self.reset_accumulation_textures()
            }
        }

        // Update camera buffer
        let aspect_ratio = self.config.width as f32 / self.config.height as f32;
        let gpu_camera = GpuCamera {
            position: [self.camera_pos.x, self.camera_pos.y, self.camera_pos.z],
            _padding1: 0.0,
            forward: [self.forward.x, self.forward.y, self.forward.z],
            _padding2: 0.0,
            right: [self.right.x, self.right.y, self.right.z],
            _padding3: 0.0,
            up: [self.up.x, self.up.y, self.up.z],
            _padding4: 0.0,
            focal_distance: self.focal_distance,
            aperture_radius: self.aperture_radius,
            aspect_ratio,
            frame: self.frame,
        };

        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[gpu_camera]));
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if !self.is_surface_configured {
            return Ok(());
        }

        // Determine which texture is input and which is output
        let (input_view, output_view) = if self.accumulation_swap {
            (&self.accumulation_texture_b_view, &self.accumulation_texture_a_view)
        } else {
            (&self.accumulation_texture_a_view, &self.accumulation_texture_b_view)
        };

        self.queue.write_buffer(&self.rand_seed_buffer, 0, bytemuck::cast_slice(&[self.frame]));
        self.queue.write_buffer(&self.sample_count_buffer, 0, bytemuck::cast_slice(&[self.sample_count]));

        // Create bind group for this frame
        let compute_bind_group_layout = self.compute_pipeline.get_bind_group_layout(0);
        let compute_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.scene_info_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.face_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.material_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&self.render_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(input_view),  // Read from this
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(output_view), // Write to this
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.rand_seed_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.sample_count_buffer.as_entire_binding(),
                },
            ],
        });

        // Run compute shader
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });

            // Bind all the data to group 0
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &compute_bind_group, &[]);

            let workgroup_size = 8;
            let dispatch_x = (self.config.width + workgroup_size - 1) / workgroup_size;
            let dispatch_y = (self.config.height + workgroup_size - 1) / workgroup_size;

            compute_pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }

        self.queue.submit(Some(encoder.finish()));

        self.accumulation_swap ^= true;

        // Render to screen
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.render_bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();

        self.frame += 1;
        self.sample_count += 1;

        self.window.request_redraw();

        Ok(())
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        if is_pressed {
            self.keys_down.insert(code);
        } else {
            self.keys_down.remove(&code);
        }

        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            (KeyCode::KeyL, true) => {
                self.input_locked = !self.input_locked;
            },
            (KeyCode::ArrowUp, true) => {
                self.focal_distance += 0.02;
                self.reset_accumulation_textures();
            },
            (KeyCode::ArrowDown, true) => {
                self.focal_distance -= 0.02;
                self.reset_accumulation_textures();
            },
            (KeyCode::ArrowLeft, true) => {
                self.aperture_radius += 0.02;
            },
            (KeyCode::ArrowRight, true) => {
                self.aperture_radius -= 0.02;
            },
            _ => {}
        }


    }


    fn reset_accumulation_textures(&mut self) {
        self.sample_count = 0;

        // Recreate both accumulation textures
        self.accumulation_texture_a = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Accumulation Texture A"),
            size: self.accumulation_texture_a.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        self.accumulation_texture_a_view = self.accumulation_texture_a.create_view(&wgpu::TextureViewDescriptor::default());

        self.accumulation_texture_b = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Accumulation Texture B"),
            size: self.accumulation_texture_a.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        self.accumulation_texture_b_view = self.accumulation_texture_b.create_view(&wgpu::TextureViewDescriptor::default());

    }
}

pub struct App {
    state: Option<State>,
    last_frame_time: std::time::Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None,
            last_frame_time: std::time::Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("GPU Raytracer");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        let mut state = Some(pollster::block_on(State::new(window)).unwrap());
        
        if let Some(state) = &mut state {
            let size = state.window.inner_size();
            state.resize(size.width, size.height); // This configures the surface!
        }

        self.state = state;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(state) => state,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let dt = (now - self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

                state.update(dt);

                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let size = state.window.inner_size();
                        state.resize(size.width, size.height);
                    }
                    Err(e) => {
                        log::error!("Unable to render: {}", e);
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(code),
                    state: key_state,
                    ..
                },
                ..
            } => {
                state.handle_key(event_loop, code, key_state.is_pressed())
            },
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        let state = match &mut self.state {
            Some(state) => state,
            None => return,
        };

        if let DeviceEvent::MouseMotion { delta } = event {
            if !state.input_locked {
                state.mouse_delta.0 -= delta.0 as f32;
                state.mouse_delta.1 -= delta.1 as f32;

                state.reset_accumulation_textures();
            }
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

fn main() {
    run().unwrap();
}

// Scene generation (reusing your existing code)
fn generate_map() -> World {
    let mut world = World { meshes: vec![], baked_meshes: vec![] };

    // Add Cornell box
    world.meshes.extend(obj_parser::load_glb("src/models/low_poly_house.glb"));

    world.bake_meshes();
    world
}