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
  [[location(1)]] vel: vec3<f32>;
  [[location(2)]] life: f32;
};

[[stage(vertex)]]
fn vs_main(in: VertexInput) -> VertexOutput {
  let pos = in.pos.xyz;
  let vel = in.vel.xyz;
  let clip_pos = camera.view_proj * vec4<f32>(pos, 1.0);
  return VertexOutput(clip_pos, pos, vel, in.life);
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
  // if (length(in.world_position) > 1.) { discard; }
  let xaxis = step(0., in.world_position.x) * vec3<f32>(0., 0.1, 0.6);
  let zaxis = step(0., in.world_position.z) * vec3<f32>(0., 0.5, 0.1);
  var a = 1.5 - length(in.world_position);

  var col = vec3<f32>(.6, .1, .2);
  col = col + xaxis;
  col = col + zaxis;
  if (all(smoothStep(vec3<f32>(.2), vec3<f32>(.02), in.vel) <= vec3<f32>(0.5))) {
    a = a * 0.01;
  }
  return vec4<f32>(normalize(col) , a);
}
