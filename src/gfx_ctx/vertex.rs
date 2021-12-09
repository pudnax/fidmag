use bytemuck::{Pod, Zeroable};

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
pub const CELL: [Vertex; 16] = [
    v!(-1.0, -1.0, -1.0), v!( 1.0, -1.0, -1.0),
    v!(-1.0, -1.0, -1.0), v!(-1.0, -1.0,  1.0),
    v!(-1.0, -1.0,  1.0), v!( 1.0, -1.0,  1.0),
    v!( 1.0, -1.0,  1.0), v!( 1.0, -1.0, -1.0),
    v!(-1.0,  1.0, -1.0), v!( 1.0,  1.0, -1.0),
    v!(-1.0,  1.0, -1.0), v!(-1.0,  1.0,  1.0),
    v!(-1.0,  1.0,  1.0), v!( 1.0,  1.0,  1.0),
    v!( 1.0,  1.0,  1.0), v!( 1.0,  1.0, -1.0),
];
