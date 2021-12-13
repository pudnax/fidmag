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

fn ihash(x: u32) -> u32 {
  var a = x;
  a = (a ^ 61u) ^ (a >> 16u);
  a = a + (a << 3u);
  a = a ^ (a >> 4u);
  a = a * 0x27d4eb2du;
  a = a ^ (a >> 15u);
  return a;
}
fn hash(a: u32) -> f32 { return f32(ihash(a)) / f32(0x7FFFFFFFu) - 1.; }
fn rand4(seed: u32) -> vec4<f32> {
  return vec4<f32>(hash(seed ^ 0x34F85A93),
                   hash(seed ^ 0x85FB93D5),
                   hash(seed ^ 0x6253DF84),
		   hash(seed ^ 0x25FC3625));
}
fn rand3(seed: u32) -> vec3<f32> {
  return vec3<f32>(hash(seed ^ 0x348CD593u),
                   hash(seed ^ 0x8593FD5u),
                   hash(seed ^ 0x62A5D384u));
}

[[stage(compute), workgroup_size(256, 1, 1)]]
fn main(
    [[builtin(global_invocation_id)]] global_id: vec3<u32>,
) {
  let id = global_id.x;
  let uid = global_id.x;
  let p = &particles.data[uid];

  (*p).pos = vec4<f32>(rand3(id + u32(rand.v)), 1.0);
  (*p).vel = vec4<f32>(rand3(id + u32(rand.v + 1.)), 1.0) * 0.1;
  (*p).life = (hash(id + u32(rand.v)) * 0.5 + 0.5) * 100.;
}
