struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] uv : vec2<f32>;
};

[[stage(vertex)]]
fn v_main([[builtin(vertex_index)]] in_vertex_index: u32) -> VertexOutput {
    let vertex_idx = i32(in_vertex_index);
    var out : VertexOutput;
    out.uv = vec2<f32>(f32((vertex_idx << 1u) & 2), f32(vertex_idx & 2));
    let x = f32(vertex_idx - 1);
    let y = f32((vertex_idx & 1) * 2 - 1);
    out.position = vec4<f32>(x / 2.0, y / 2.0, 0.0, 1.0);
    return out;
}

[[stage(fragment)]]
fn f_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(in.uv - 0.5, 1., 1.0);
}
