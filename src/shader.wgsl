[[block]]
struct Camera {
    view_pos: vec4<f32>;
    view_proj: mat4x4<f32>;
};
[[group(0), binding(0)]]
var<uniform> camera: Camera;

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] world_position: vec4<f32>;
    [[location(1)]] uv: vec2<f32>;
};

[[stage(vertex)]]
fn v_main([[builtin(vertex_index)]] in_vertex_index: u32) -> VertexOutput {
    let vertex_idx = i32(in_vertex_index);
    var out : VertexOutput;
    out.uv = vec2<f32>(f32((vertex_idx << 1u) & 2), f32(vertex_idx & 2));
    let x = f32(vertex_idx - 1);
    let y = f32((vertex_idx & 1) * 2 - 1);
    let pos = vec3<f32>(x, y, 0.0);
    out.clip_position = camera.view_proj * vec4<f32>(pos, 1.0);
    out.world_position = vec4<f32>(pos, 1.);
    return out;
}

[[stage(fragment)]]
fn f_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(in.uv - 0.5, 1., 1.0);
}
