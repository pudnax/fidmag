mod line;

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use glam::{vec3, Vec3, Vec4};
use rand::Rng;
use raw_window_handle::HasRawWindowHandle;
use wgpu::util::DeviceExt;

use crate::{
    camera::{Camera, CameraUniform},
    gfx_ctx::line::draw_lines_command,
};

const WORKGROUP_SIZE: u32 = 256;
pub fn dispatch_size(len: u32) -> u32 {
    let subgroup_size = WORKGROUP_SIZE;
    let padded_size = (subgroup_size - len % subgroup_size) % subgroup_size;
    (len + padded_size) / subgroup_size
}

fn create_multisampled_framebuffer(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    sample_count: u32,
) -> wgpu::TextureView {
    let multisampled_texture_extent = wgpu::Extent3d {
        width: config.width,
        height: config.height,
        depth_or_array_layers: 1,
    };
    let multisampled_frame_descriptor = &wgpu::TextureDescriptor {
        size: multisampled_texture_extent,
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        label: None,
    };

    device
        .create_texture(multisampled_frame_descriptor)
        .create_view(&wgpu::TextureViewDescriptor::default())
}

pub fn create_depth_texture(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    sample_count: u32,
) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: config.width,
        height: config.height,
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT, /*  | wgpu::TextureUsages::TEXTURE_BINDING */
    };
    let texture = device.create_texture(&desc);

    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

#[derive(Clone, Copy, Debug)]
struct Charge {
    q: f32,
    pos: Vec3,
}

impl Charge {
    fn new_rand(rng: &mut impl Rng) -> Self {
        let q_range = 1.;
        Self {
            q: rng.gen_range(-q_range..q_range),
            pos: Vec3::from([0., 0., 0.].map(|_| rng.gen_range(-1.0..1.0))),
        }
    }
}

fn get_charge(pos: Vec3, charge: Charge) -> Vec3 {
    let pc = pos - charge.pos;
    let r2 = pc.dot(pc);
    pc * (charge.q / r2.powf(1.5) + 1.0e-3)
}

fn get_field(p: Vec3, charges: &[Charge]) -> Vec3 {
    charges
        .iter()
        .fold(Vec3::ZERO, |acc, &q| acc + get_charge(p, q))
}

fn get_field_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    depth: u32,
) -> wgpu::TextureView {
    let mut rng = rand::thread_rng();
    let charges: Vec<Charge> = (0..6).map(|_| Charge::new_rand(&mut rng)).collect();
    let texture_data: Vec<Vec4> = (0..width * height * depth)
        .map(|id| {
            let i = id as f32;
            let (width, height, depth) = (width as f32, height as f32, depth as f32);
            let [x, y, z] = [i % width, (i / width) % height, i / (width * height)];

            let p = vec3(x, y, z) / vec3(width, height, depth) * 2.0 - 1.0;

            get_field(p, &charges).extend(1.)
        })
        .collect();
    let tex = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("Field Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
        },
        bytemuck::cast_slice(&texture_data),
    );
    tex.create_view(&Default::default())
}

fn draw_particles_command(
    device: &wgpu::Device,
    sample_count: u32,
    format: wgpu::TextureFormat,
    camera_bind_group_layout: &wgpu::BindGroupLayout,
    camera_bind_group: &wgpu::BindGroup,
    particle_buffer: &wgpu::Buffer,
    particles_num: u32,
) -> wgpu::RenderBundle {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Particle Pipeline Descriptor"),
        bind_group_layouts: &[camera_bind_group_layout],
        push_constant_ranges: &[],
    });

    let shader = device.create_shader_module(&wgpu::include_wgsl!("particle.wgsl"));

    let draw_particles_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Draw Particle Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Particle>() as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &Particle::VERTEX_FORMAT,
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            }],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::PointList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview: None,
    });

    let mut encoder = device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
        label: Some("Particle Bundle Encoder"),
        color_formats: &[format],
        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
            format: wgpu::TextureFormat::Depth32Float,
            depth_read_only: false,
            stencil_read_only: false,
        }),
        sample_count,
        multiview: None,
    });
    encoder.set_pipeline(&draw_particles_pipeline);
    encoder.set_vertex_buffer(0, particle_buffer.slice(..));
    encoder.set_bind_group(0, camera_bind_group, &[]);
    encoder.draw(0..particles_num, 0..1);
    encoder.finish(&wgpu::RenderBundleDescriptor {
        label: Some("Draw Particles Bundle"),
    })
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Particle {
    pos: Vec4,
    vel: Vec4,
    lifetime: f32,
    _padding: [f32; 3],
}

#[allow(dead_code)]
impl Particle {
    const VERTEX_FORMAT: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];
    fn new(pos: Vec4, vel: Vec4, lifetime: f32) -> Self {
        Self {
            pos,
            vel,
            lifetime,
            _padding: [0.; 3],
        }
    }

    fn new_rand(
        pos_range: Range<f32>,
        vel_range: Range<f32>,
        life_range: Range<f32>,
        rng: &mut impl Rng,
    ) -> Self {
        use std::array::from_fn;
        Self::new(
            Vec4::from(from_fn(|_| rng.gen_range(pos_range.clone()))),
            Vec4::from(from_fn(|_| rng.gen_range(vel_range.clone()))),
            rng.gen_range(life_range),
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct SharedUniform {
    dt: f32,
    time: f32,
    seed: u32,
}

pub struct Context {
    surface: wgpu::Surface,
    pub device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::TextureView,

    pub width: u32,
    pub height: u32,

    multisampled_framebuffer: wgpu::TextureView,

    draw_lines_command: wgpu::RenderBundle,

    pub camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,

    draw_particles_command: wgpu::RenderBundle,
    particle_num: u32,
    _particle_buffer: wgpu::Buffer,
    particle_bind_group: wgpu::BindGroup,

    rand_uniform: wgpu::Buffer,
    _rand_uniform_binding: wgpu::BindGroup,

    integrate_pipeline: wgpu::ComputePipeline,
    simulation_pipeline: wgpu::ComputePipeline,

    field_texture_binding: wgpu::BindGroup,
}

impl Context {
    const MSAA_SAMPLE_COUNT: u32 = 4;
    pub async fn new(
        window: &impl HasRawWindowHandle,
        width: u32,
        height: u32,
        camera: Camera,
    ) -> Self {
        let instance = wgpu::Instance::new(wgpu::Backends::all());

        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        dbg!(&adapter.get_info());
        let features = adapter.features();
        let limits = adapter.limits();
        let format = surface.get_preferred_format(&adapter).unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    features,
                    limits,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Immediate,
        };
        surface.configure(&device, &config);

        let depth_texture = create_depth_texture(&device, &config, Self::MSAA_SAMPLE_COUNT);

        let multisampled_framebuffer =
            create_multisampled_framebuffer(&device, &config, Self::MSAA_SAMPLE_COUNT);

        let mut camera_uniform = CameraUniform::default();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let draw_lines_command = draw_lines_command(
            &device,
            Self::MSAA_SAMPLE_COUNT,
            format,
            &camera_bind_group_layout,
            &camera_bind_group,
        );

        let particle_num = 1e6 as u32;
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particles"),
            size: particle_num as u64 * std::mem::size_of::<Particle>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let particle_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let particle_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Bind Group"),
            layout: &particle_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: particle_buffer.as_entire_binding(),
            }],
        });
        let rand_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rand Uniform"),
            size: std::mem::size_of::<f32>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rand_uniform_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });
        let rand_uniform_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &&rand_uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: rand_uniform.as_entire_binding(),
            }],
        });
        let sim_shader = device.create_shader_module(&wgpu::include_wgsl!("simulation.wgsl"));
        let fill_shader = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Fill Pipeline Layout"),
                bind_group_layouts: &[&particle_bind_group_layout, &rand_uniform_layout],
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Fill Pipeline"),
                layout: Some(&pipeline_layout),
                module: &sim_shader,
                entry_point: "fill",
            })
        };
        {
            let mut encoder = device.create_command_encoder(&Default::default());
            let mut cpass = encoder.begin_compute_pass(&Default::default());
            cpass.set_pipeline(&fill_shader);
            cpass.set_bind_group(0, &particle_bind_group, &[]);
            cpass.set_bind_group(1, &rand_uniform_binding, &[]);
            cpass.dispatch(dispatch_size(particle_num), 1, 1);
            drop(cpass);
            queue.submit(Some(encoder.finish()));
        }

        let draw_particles_command = draw_particles_command(
            &device,
            Self::MSAA_SAMPLE_COUNT,
            format,
            &camera_bind_group_layout,
            &camera_bind_group,
            &particle_buffer,
            particle_num,
        );

        let [width, height, depth] = [0, 0, 0].map(|_| 32 * 2);
        let field_texture = get_field_texture(&device, &queue, width, height, depth);
        let field_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Field Sampler"),
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            address_mode_w: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let field_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Field Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        count: None,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    },
                ],
            });
        let field_texture_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Field Texture Bind Group"),
            layout: &field_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&field_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&field_sampler),
                },
            ],
        });

        let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Time"),
            size: std::mem::size_of::<[f32; 2]>() as _,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let time_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Time Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let integrate_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&particle_bind_group_layout],
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Integration Pipeline"),
                layout: Some(&layout),
                module: &sim_shader,
                entry_point: "integrate",
            })
        };

        let simulation_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Simulation Pipeline Layout"),
                bind_group_layouts: &[
                    &particle_bind_group_layout,
                    &field_texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Simulation Pipeline"),
                layout: Some(&pipeline_layout),
                module: &sim_shader,
                entry_point: "compute_field",
            })
        };

        Self {
            surface,
            device,
            queue,
            config,
            depth_texture,
            width,
            height,
            draw_lines_command,
            multisampled_framebuffer,
            camera,
            camera_buffer,
            camera_uniform,

            draw_particles_command,
            _particle_buffer: particle_buffer,
            particle_bind_group,
            particle_num,

            rand_uniform,
            _rand_uniform_binding: rand_uniform_binding,

            integrate_pipeline,
            simulation_pipeline,

            field_texture_binding,
        }
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if new_width > 0 && new_height > 0 {
            self.width = new_width;
            self.height = new_height;
            self.config.width = new_width;
            self.config.height = new_height;

            self.depth_texture =
                create_depth_texture(&self.device, &self.config, Self::MSAA_SAMPLE_COUNT);
            self.multisampled_framebuffer = create_multisampled_framebuffer(
                &self.device,
                &self.config,
                Self::MSAA_SAMPLE_COUNT,
            );

            self.surface.configure(&self.device, &self.config);
            self.camera.aspect = self.config.width as f32 / self.config.height as f32;
        }
    }

    pub fn update(&mut self) {
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
        self.queue
            .write_buffer(&self.rand_uniform, 0, &rand::random::<f32>().to_le_bytes());
    }

    pub fn simulate(&mut self, _dt: f32) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Compute Encoder"),
            });

        let mut cpass = encoder.begin_compute_pass(&Default::default());

        cpass.set_pipeline(&self.simulation_pipeline);
        cpass.set_bind_group(0, &self.particle_bind_group, &[]);
        cpass.set_bind_group(1, &self.field_texture_binding, &[]);
        cpass.dispatch(dispatch_size(self.particle_num), 1, 1);

        cpass.set_pipeline(&self.integrate_pipeline);
        cpass.set_bind_group(0, &self.particle_bind_group, &[]);
        cpass.dispatch(dispatch_size(self.particle_num), 1, 1);

        drop(cpass);

        self.queue.submit(Some(encoder.finish()));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.multisampled_framebuffer,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.015,
                            g: 0.015,
                            b: 0.015,
                            a: 1.0,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            rpass.execute_bundles(
                [&self.draw_lines_command, &self.draw_particles_command]
                    .iter()
                    .cloned(),
            );
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
