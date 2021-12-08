use anyhow::Result;
use rand::Rng;
use raw_window_handle::HasRawWindowHandle;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
    window::WindowBuilder,
};

use bytemuck::{Pod, Zeroable};
use wgpu::{util::DeviceExt, TextureFormat};

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

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    /// The width of the wgpu renderer in pixels.
    pub width: u32,

    /// The height of the wgpu renderer in pixels.
    pub height: u32,

    trig_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    multisampled_framebuffer: wgpu::TextureView,
    vertex_buffer: wgpu::Buffer,

    surface_format: TextureFormat,
}

impl State {
    const MSAA_SAMPLE_COUNT: u32 = 4;
    pub async fn new(window: &impl HasRawWindowHandle, width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::new(wgpu::Backends::all());

        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false, // If possible do not use a software renderer.
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
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let multisampled_framebuffer =
            create_multisampled_framebuffer(&device, &config, Self::MSAA_SAMPLE_COUNT);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&CELL),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let shader = device.create_shader_module(&wgpu::include_wgsl!("shader.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

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
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: Self::MSAA_SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: true,
            },
            multiview: None,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "f_main",
                targets: &[format.into()],
            }),
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
                count: Self::MSAA_SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: true,
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: "fs_main",
                targets: &[format.into()],
            }),
            multiview: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            width,
            height,
            trig_pipeline,
            line_pipeline,
            multisampled_framebuffer,
            vertex_buffer,
            surface_format: format,
        }
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if new_width > 0 && new_height > 0 {
            self.width = new_width;
            self.height = new_height;
            self.config.width = new_width;
            self.config.height = new_height;

            self.multisampled_framebuffer = create_multisampled_framebuffer(
                &self.device,
                &self.config,
                Self::MSAA_SAMPLE_COUNT,
            );

            self.surface.configure(&self.device, &self.config);
            // self.camera.aspect = self.config.width as f32 / self.config.height as f32;
        }
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
            rpass.set_pipeline(&self.trig_pipeline);
            rpass.draw(0..3, 0..1);

            rpass.set_pipeline(&self.line_pipeline);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.draw(0..CELL.len() as _, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = winit::event_loop::EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;

    let mut state = pollster::block_on({
        let PhysicalSize { width, height } = window.inner_size();
        State::new(&window, width, height)
    });

    let mut rng = rand::thread_rng();
    let particles: Vec<f32> = (0..1000 * 4).map(|_| rng.gen_range(-10. ..10.)).collect();
    let _particle_buffer = state
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particles"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::STORAGE,
        });

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(new_size) => {
                    state.resize(new_size.width, new_size.height);
                    window.request_redraw();
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    state.resize(new_inner_size.width, new_inner_size.height);
                    window.request_redraw();
                }
                _ => {}
            },
            Event::RedrawRequested(_) => match state.render() {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost) => {
                    state.resize(state.width, state.height);
                    window.request_redraw();
                }
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                Err(e) => {
                    eprintln!("{:?}", e);
                    window.request_redraw();
                }
            },
            _ => {}
        }
    });
}
