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
struct Time {
  dt: f32;
  instant: f32;
};

[[group(0), binding(0)]]
var<storage, read_write> particles: ParticleData;
// [[group(1), binding(0)]]
// var<uniform> time: Time;

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
  let new_life = curr_life - 0.5;

  if (new_life < 0. || new_pos.x > 1. || new_pos.x < -1.
                    || new_pos.y > 1. || new_pos.y < -1.
		    || new_pos.z > 1. || new_pos.z < -1.) {
    (*p).pos = vec4<f32>(.5);
    return;
  }
  (*p).pos = new_pos;
}
