mod line;

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
pub fn dispatch_optimal_size(len: u32, subgroup_size: u32) -> u32 {
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
    inner_r: f32,
    outter_r: f32,
}

impl Charge {
    fn new(q: f32, pos: Vec3, inner_r: f32, outter_r: f32) -> Self {
        Self {
            q,
            pos,
            inner_r,
            outter_r,
        }
    }

    fn new_rand(rng: &mut impl Rng) -> Self {
        // let max_pos = 3. * Vec3::ONE;
        Self {
            q: rng.gen_range(-10. ..10.),
            pos: Vec3::from([0., 0., 0.].map(|_| rng.gen_range(-3. ..3.))),
            // pos: rng.gen_range(-max_pos..max_pos),
            inner_r: rng.gen_range(0. ..1.),
            outter_r: rng.gen_range(1. ..3.),
        }
    }
}

fn get_charge(pos: Vec3, charge: Charge) -> Vec3 {
    let pc = pos - charge.pos;
    let r2 = pc.dot(pc);
    pc * (charge.q / r2.powf(1.5) + 1.0e-2)
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
    let charges: Vec<Charge> = (0..9).map(|_| Charge::new_rand(&mut rng)).collect();
    let texture_data: Vec<Vec4> = (0..width * height * depth)
        .map(|i| {
            let x = (i % width) as f32;
            let y = (i / width) as f32;
            let z = (i / (width * height)) as f32;
            let p = vec3(x, y, z) / vec3(width as f32, height as f32, depth as f32) * 2.0 - 1.0;
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
            usage: wgpu::TextureUsages::STORAGE_BINDING,
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
                array_stride: std::mem::size_of::<[f32; 4]>() as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x4],
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

pub struct Context {
    surface: wgpu::Surface,
    pub device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::TextureView,

    pub width: u32,
    pub height: u32,

    trig_pipeline: wgpu::RenderPipeline,
    multisampled_framebuffer: wgpu::TextureView,

    draw_lines_command: wgpu::RenderBundle,

    pub camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    draw_particles_command: wgpu::RenderBundle,
    fill_shader: wgpu::ComputePipeline,
    particle_num: u32,
    particle_buffer: wgpu::Buffer,
    particle_buffer2: wgpu::Buffer,
    particle_bind_group: wgpu::BindGroup,

    rand_uniform: wgpu::Buffer,
    rand_uniform_binding: wgpu::BindGroup,

    field_texture: wgpu::TextureView,
}

fn hash31(p: f32) -> [f32; 4] {
    let mut p3 = [p * 0.1031, p * 0.1030, p * 0.0973].map(|x| x.fract());
    p3 = p3.zip([p3[1], p3[2], p3[0]]).map(|(x, y)| x * (y + 33.33));
    let (px, py, pz) = (p3[0], p3[1], p3[2]);
    [px, px, py, 1.]
        .zip([py, pz, pz, 1.])
        .zip([pz, py, px, 1.])
        .map(|((x, y), z)| ((x + y) * z).fract() * 2.0 - 1.)
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Particle {
    pos: Vec4,
    vel: Vec4,
    lifetime: f32,
    _padding: [f32; 3],
}

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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Line Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(&wgpu::include_wgsl!("shader.wgsl"));

        let trig_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "v_main",
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
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
                count: Self::MSAA_SAMPLE_COUNT,
                ..Default::default()
            },
            multiview: None,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "f_main",
                targets: &[format.into()],
            }),
        });

        let draw_lines_command = draw_lines_command(
            &device,
            Self::MSAA_SAMPLE_COUNT,
            format,
            &camera_bind_group_layout,
            &camera_bind_group,
        );

        let particle_num = 1e6 as u32;
        // let mut rng = rand::thread_rng();
        let particles: Vec<f32> = (0..particle_num * 4).flat_map(|i| hash31(i as _)).collect();
        // let particles: Vec<f32> = (0..particle_num)
        //     .map(|_| rand::random::<f32>() * 2. - 1.)
        //     .collect();
        let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particles CPU"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            contents: bytemuck::cast_slice(&particles),
        });
        let particle_buffer2 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particles"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            size: particle_num as u64 * std::mem::size_of::<[f32; 4]>() as u64,
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
        let fill_shader = {
            let shader = device.create_shader_module(&wgpu::include_wgsl!("fill_buffer.wgsl"));
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Fill Pipeline Layout"),
                bind_group_layouts: &[&particle_bind_group_layout, &rand_uniform_layout],
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Fill Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: "main",
            })
        };

        let draw_particles_command = draw_particles_command(
            &device,
            Self::MSAA_SAMPLE_COUNT,
            format,
            &camera_bind_group_layout,
            &camera_bind_group,
            &particle_buffer,
            particle_num,
        );

        let (width, height, depth) = (64, 64, 64);
        let field_texture = get_field_texture(&device, &queue, width, height, depth);

        Self {
            surface,
            device,
            queue,
            config,
            depth_texture,
            width,
            height,
            trig_pipeline,
            draw_lines_command,
            multisampled_framebuffer,
            camera,
            camera_buffer,
            camera_bind_group,
            camera_uniform,

            draw_particles_command,
            particle_buffer,
            particle_buffer2,
            particle_bind_group,
            particle_num,
            fill_shader,

            rand_uniform,
            rand_uniform_binding,

            field_texture,
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

        self.queue
            .write_buffer(&self.rand_uniform, 0, &rand::random::<f32>().to_le_bytes());

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.multisampled_framebuffer,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
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

            rpass.set_pipeline(&self.trig_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
