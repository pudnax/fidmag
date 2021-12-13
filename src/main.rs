#![feature(array_zip, array_from_fn)]
use std::time::{Duration, Instant};

use anyhow::Result;
use camera::Camera;
use gfx_ctx::Context;
use glam::Vec3;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        DeviceEvent, ElementState, Event, KeyboardInput, MouseScrollDelta, VirtualKeyCode,
        WindowEvent,
    },
    event_loop::ControlFlow,
    window::WindowBuilder,
};

mod camera;
mod gfx_ctx;

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = winit::event_loop::EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop)?;
    let size = window.inner_size();

    let mut context = pollster::block_on({
        let PhysicalSize { width, height } = size;
        let camera = Camera::new(
            1.5,
            0.5,
            1.25,
            Vec3::new(0.0, 0.0, 0.0),
            width as f32 / height as f32,
        );
        Context::new(&window, width, height, camera)
    });

    let mut mouse_dragged = false;

    let rotate_speed = 0.0025;
    let zoom_speed = 0.002;

    let mut last_update_inst = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::RedrawEventsCleared => {
                let target_frametime = Duration::from_secs_f64(1.0 / 60.0);
                let time_since_last_frame = last_update_inst.elapsed();
                if time_since_last_frame >= target_frametime {
                    window.request_redraw();
                    last_update_inst = Instant::now();
                } else {
                    *control_flow = ControlFlow::WaitUntil(
                        Instant::now() + target_frametime - time_since_last_frame,
                    );
                }
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(new_size) => {
                    context.resize(new_size.width, new_size.height);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    context.resize(new_inner_size.width, new_inner_size.height);
                }
                _ => {}
            },
            Event::DeviceEvent { ref event, .. } => match event {
                DeviceEvent::Button {
                    #[cfg(target_os = "macos")]
                        button: 0,
                    #[cfg(not(target_os = "macos"))]
                        button: 1,

                    state: statee,
                } => {
                    let is_pressed = *statee == ElementState::Pressed;
                    mouse_dragged = is_pressed;
                }
                DeviceEvent::MouseWheel { delta, .. } => {
                    let scroll_amount = -match delta {
                        // A mouse line is about 1 px.
                        MouseScrollDelta::LineDelta(_, scroll) => scroll * 1.0,
                        MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => {
                            *scroll as f32
                        }
                    };
                    context.camera.add_zoom(scroll_amount * zoom_speed);
                }
                DeviceEvent::MouseMotion { delta } => {
                    if mouse_dragged {
                        context.camera.add_yaw(-delta.0 as f32 * rotate_speed);
                        context.camera.add_pitch(delta.1 as f32 * rotate_speed);
                    }
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                // context.camera.add_yaw(-0.001);
                context.update();
                context.simulate(0.);
                match context.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        context.resize(context.width, context.height);
                        window.request_redraw();
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => {
                        eprintln!("{:?}", e);
                        window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    });
}
