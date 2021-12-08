use anyhow::Result;
use rand::Rng;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
    window::WindowBuilder,
};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    v: [f32; 3],
}

impl Vertex {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { v: [x, y, z] }
    }
}

macro_rules! v {
    ($x:expr, $y:expr, $z:expr) => {
        Vertex::new($x, $y, $z)
    };
}

#[rustfmt::skip]
const CELL: [Vertex; 16] = [
    v!(-0.5, -0.5, -0.5), v!( 0.5, -0.5, -0.5),
    v!(-0.5, -0.5, -0.5), v!(-0.5, -0.5,  0.5),
    v!(-0.5, -0.5,  0.5), v!( 0.5, -0.5,  0.5),
    v!( 0.5, -0.5,  0.5), v!( 0.5, -0.5, -0.5),
    v!(-0.5,  0.5, -0.5), v!( 0.5,  0.5, -0.5),
    v!(-0.5,  0.5, -0.5), v!(-0.5,  0.5,  0.5),
    v!(-0.5,  0.5,  0.5), v!( 0.5,  0.5,  0.5),
    v!( 0.5,  0.5,  0.5), v!( 0.5,  0.5, -0.5),
];

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

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = winit::event_loop::EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);

    let surface = unsafe { instance.create_surface(&window) };

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .unwrap();

    let features = adapter.features();
    let limits = adapter.limits();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("Device"),
            features,
            limits,
        },
        None,
    ))?;

    let swapchain_format = surface.get_preferred_format(&adapter).unwrap();
    let size = window.inner_size();
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Mailbox,
    };

    surface.configure(&device, &config);
    let sample_count = 4;
    let multisampled_framebuffer = create_multisampled_framebuffer(&device, &config, sample_count);

    let mut rng = rand::thread_rng();
    let particles: Vec<f32> = (0..1000 * 4).map(|_| rng.gen_range(-10. ..10.)).collect();
    let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Particles"),
        contents: bytemuck::cast_slice(&particles),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let shader = device.create_shader_module(&wgpu::include_wgsl!("shader.wgsl"));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: true,
        },
        multiview: None,
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "f_main",
            targets: &[swapchain_format.into()],
        }),
    });

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&CELL),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let line_shader = device.create_shader_module(&wgpu::include_wgsl!("line.wgsl"));

    let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Line Pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &line_shader,
            entry_point: "vs_main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3],
            }],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: true,
        },
        fragment: Some(wgpu::FragmentState {
            module: &line_shader,
            entry_point: "fs_main",
            targets: &[swapchain_format.into()],
        }),
        multiview: None,
    });

    event_loop.run(move |event, _, control_flow| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&instance, &adapter, &shader, &pipeline_layout);

        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                // Reconfigure the surface with the new size
                config.width = size.width;
                config.height = size.height;
                surface.configure(&device, &config);
            }
            Event::RedrawRequested(_) => {
                let frame = surface
                    .get_current_texture()
                    .expect("Failed to acquire next swap chain texture");
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[wgpu::RenderPassColorAttachment {
                            view: &multisampled_framebuffer,
                            resolve_target: Some(&view),
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.15,
                                    b: 0.15,
                                    a: 1.0,
                                }),
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&pipeline);
                    rpass.draw(0..3, 0..1);

                    rpass.set_pipeline(&line_pipeline);
                    rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    rpass.draw(0..CELL.len() as _, 0..1);
                }

                queue.submit(Some(encoder.finish()));
                frame.present();
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
