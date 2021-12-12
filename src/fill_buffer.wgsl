struct Particle {
  pos: vec4<f32>;
  vel: vec4<f32>;
  life: f32;
};

[[block]]
struct ParticleData {
  data: [[stride(48)]] array<Particle>;
};

[[block]]
struct Uni {
  v: f32;
};

[[group(0), binding(0)]]
var<storage, read_write> particles: ParticleData;
[[group(1), binding(0)]]
var<uniform> rand: Uni;

fn hash31(p: f32) -> vec3<f32> {
  var p3 = fract(vec3<f32>(p) * vec3<f32>(.1031, .1030, .0973));
  p3 = p3 + dot(p3, p3.yzx + 33.33);
  return fract((p3.xxy + p3.yzz) * p3.zyx);
}

[[stage(compute), workgroup_size(256, 1, 1)]]
fn main(
    [[builtin(global_invocation_id)]] global_id: vec3<u32>,
) {
  let id = global_id.x;
  particles.data[id].pos = vec4<f32>(hash31(f32(id) + rand.v) * 2. - 1., 1.0);
}
