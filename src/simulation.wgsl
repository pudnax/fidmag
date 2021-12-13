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

  p.pos = rand4(seed);
  p.vel = rand4(seed + 1u) * 0.1;
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
  let new_life = curr_life - curr_vel.w;

  if (new_life < 0. || abs(new_pos.x) > 1.
                    || abs(new_pos.y) > 1.
		    || abs(new_pos.z) > 1.) {
    (*p) = generate_particle(id);
    return;
  }
  (*p).pos = new_pos;
  (*p).life = new_life;
}

[[group(2), binding(0)]]
var field_texture: texture_3d<f32>;
[[group(2), binding(1)]]
var field_sampler: sampler;

fn get_charge(pos: vec3<f32>) -> vec3<f32> {
  let chargePosition = vec3<f32>(0.); // + .5;
  let q = -0.2;
  let pc = pos - chargePosition;
  let r2 = dot(pc, pc);
  let E = pc * q / (pow(r2, 1.5) + 1.0e-2);
  return E;
}

fn get_field(p: vec3<f32>) -> vec3<f32> {
  var res = textureSampleLevel(field_texture, field_sampler, p, 0.).xyz;
  res = res + get_charge(p);
  return res;
}

[[stage(compute), workgroup_size(256, 1, 1)]]
fn compute_field(
    [[builtin(global_invocation_id)]] global_id: vec3<u32>,
) {
  let id = global_id.x;
  let p = &particles.data[id];
  let curr_pos = (*p).pos.xyz;
  let curr_vel = (*p).vel;
  let curr_life = (*p).life;

  (*p).vel = vec4<f32>(get_field(curr_pos) * 0.1 * curr_life, curr_vel.w);
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
