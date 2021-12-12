[[block]]
struct Camera {
  view_pos: vec4<f32>;
  view_proj: mat4x4<f32>;
};
[[group(0), binding(0)]] var<uniform> camera : Camera;

struct VertexInput {
  [[builtin(position)]] pos: vec4<f32>;
};

struct VertexOutput {
  [[builtin(position)]] clip_position: vec4<f32>;
  [[location(0)]] world_position: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main([[location(0)]] position: vec4<f32>) -> VertexOutput {
  let pos = camera.view_proj * vec4<f32>(position.xyz, 1.0);
  return VertexOutput(pos, position.xyz);
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
  // if (length(in.world_position) > 1.) {
  //   discard;
  // }
  let a = 1. - length(in.world_position);
  return vec4<f32>(.6, .1, .2, 1.);
}
