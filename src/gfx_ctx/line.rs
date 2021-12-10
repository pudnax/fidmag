use bytemuck::{Pod, Zeroable};
use wgpu::{util::DeviceExt, Device};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
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
pub const VERTICES: [Vertex; 16] = [
    v!(-1.0, -1.0, -1.0), v!( 1.0, -1.0, -1.0),
    v!(-1.0, -1.0, -1.0), v!(-1.0, -1.0,  1.0),
    v!(-1.0, -1.0,  1.0), v!( 1.0, -1.0,  1.0),
    v!( 1.0, -1.0,  1.0), v!( 1.0, -1.0, -1.0),
    v!(-1.0,  1.0, -1.0), v!( 1.0,  1.0, -1.0),
    v!(-1.0,  1.0, -1.0), v!(-1.0,  1.0,  1.0),
    v!(-1.0,  1.0,  1.0), v!( 1.0,  1.0,  1.0),
    v!( 1.0,  1.0,  1.0), v!( 1.0,  1.0, -1.0),
];

pub fn draw_lines_command(
    device: &Device,
    sample_count: u32,
    format: wgpu::TextureFormat,
    camera_bind_group_layout: &wgpu::BindGroupLayout,
    camera_bind_group: &wgpu::BindGroup,
) -> wgpu::RenderBundle {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&VERTICES),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let line_shader = device.create_shader_module(&wgpu::include_wgsl!("line.wgsl"));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Line Pipeline Layout"),
        bind_group_layouts: &[camera_bind_group_layout],
        push_constant_ranges: &[],
    });

    let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Line Pipeline"),
        layout: Some(&pipeline_layout),
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
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
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

    let mut encoder = device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
        label: Some("Line Bundle Encoder"),
        color_formats: &[format],
        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
            format: wgpu::TextureFormat::Depth32Float,
            depth_read_only: false,
            stencil_read_only: false,
        }),
        sample_count,
        multiview: None,
    });
    encoder.set_pipeline(&line_pipeline);
    encoder.set_bind_group(0, camera_bind_group, &[]);
    encoder.set_vertex_buffer(0, vertex_buffer.slice(..));
    encoder.draw(0..VERTICES.len() as _, 0..1);
    encoder.finish(&wgpu::RenderBundleDescriptor {
        label: Some("Draw Lines Bundle"),
    })
}
