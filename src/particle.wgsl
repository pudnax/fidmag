[[block]]
struct Camera {
  view_pos: vec4<f32>;
  view_proj: mat4x4<f32>;
};
[[group(0), binding(0)]] var<uniform> camera : Camera;

struct VertexInput {
  [[location(0)]] pos: vec4<f32>;
  [[location(1)]] vel: vec4<f32>;
  [[location(2)]] life: f32;
};

struct VertexOutput {
  [[builtin(position)]] clip_position: vec4<f32>;
  [[location(0)]] world_position: vec3<f32>;
  [[location(1)]] life: f32;
};

[[stage(vertex)]]
fn vs_main(in: VertexInput) -> VertexOutput {
  let pos = in.pos.xyz;
  let clip_pos = camera.view_proj * vec4<f32>(pos, 1.0);
  return VertexOutput(clip_pos, pos, in.life);
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
  // if (length(in.world_position) > 1.) { discard; }
  let a = 1. - length(in.world_position);
  let col = vec3<f32>(.6, .1, .2) * in.life / 100.;
  return vec4<f32>(col, 1.);
}
