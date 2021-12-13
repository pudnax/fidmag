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
  return vec4<f32>(hash(seed ^ 0x34F85A93u),
                   hash(seed ^ 0x85FB93D5u),
                   hash(seed ^ 0x6253DF84u),
		   hash(seed ^ 0x25FC3625u));
}
fn rand3(seed: u32) -> vec3<f32> {
  return vec3<f32>(hash(seed ^ 0x348CD593u),
                   hash(seed ^ 0x8593FD5u),
                   hash(seed ^ 0x62A5D384u));
}

struct Particle {
  pos: vec4<f32>;
  vel: vec4<f32>;
  life: f32;
};

fn generate_particle(seed: u32) -> Particle {
  var p : Particle;

  p.pos = vec4<f32>(rand3(seed), 1.0);
  p.vel = vec4<f32>(rand3(seed + 1u), 1.0) * 0.1;
  p.life = (hash(seed) * 0.5 + 0.5) * 100.;
  return p;
}

[[block]]
struct ParticleData {
  data: [[stride(48)]] array<Particle>;
};

[[block]]
struct Time {
  dt: f32;
  instant: f32;
};

[[group(0), binding(0)]]
var<storage, read_write> particles: ParticleData;
[[group(1), binding(0)]]
var<uniform> time: Time;

[[stage(compute), workgroup_size(256, 1, 1)]]
fn integrate(
    [[builtin(global_invocation_id)]] global_id: vec3<u32>,
) {
  let id = global_id.x;
  let p = &particles.data[id];
  let curr_pos = (*p).pos;
  let curr_vel = (*p).vel;
  let curr_life = (*p).life;

  // let new_pos = curr_pos + curr_vel * time.dt;
  let new_pos = curr_pos + curr_vel * 0.1;
  let new_life = curr_life - 1.;

  if (new_life < 0. || new_pos.x > 1. || new_pos.x < -1.
                    || new_pos.y > 1. || new_pos.y < -1.
		    || new_pos.z > 1. || new_pos.z < -1.) {
    (*p) = generate_particle(id);
    return;
  }
  (*p).pos = new_pos;
  (*p).life = new_life;
}

[[block]]
struct Uniform {
  v: f32;
};

[[group(1), binding(0)]]
var<uniform> rand: Uniform;

[[stage(compute), workgroup_size(256, 1, 1)]]
fn fill(
    [[builtin(global_invocation_id)]] global_id: vec3<u32>,
) {
  let id = global_id.x;
  let p = &particles.data[id];

  (*p) = generate_particle(id);
}
