mod examples;
mod gfx;

use crate::gfx::{
	camera::Camera,
	model::Mesh,
	render::{render, RenderWindowState},
	window::Window,
};
use examples::{INDICES, NORMALS, VERTICES};
use gfx::Gfx;
use nalgebra::{Quaternion, Vector3};
use shipyard::{Get, ViewMut, Workload, World};
use std::collections::HashMap;
use vulkano::sync::GpuFuture;
use winit::{
	event::{Event, WindowEvent},
	event_loop::{ControlFlow, EventLoop},
};

fn main() {
	let mut gfx = Gfx::new();

	let event_loop = EventLoop::new();
	let window = Window::new(&mut gfx, &event_loop);
	let render_window_state = RenderWindowState::new(&mut gfx, &window);

	let (mesh, mesh_future) =
		Mesh::new(&window, VERTICES.iter().cloned(), NORMALS.iter().cloned(), INDICES.iter().cloned());

	let size = window.inner_size();
	let aspect_ratio = size.width as f32 / size.height as f32;
	let camera = Camera::new(Vector3::new(0.0, 0.0, -100.0), Quaternion::identity(), aspect_ratio, 90.0, 0.1, 1000.0);

	let mut windows = HashMap::new();

	let mut world = World::new();
	world.add_unique(gfx).unwrap();
	world.add_entity((mesh,));
	world.add_entity(("lazy dog.",));
	windows.insert(window.id(), world.add_entity((window, render_window_state, camera)));

	Workload::builder("default").with_system(&render).add_to_world(&world).unwrap();

	mesh_future.then_signal_fence_and_flush().unwrap().wait(None).unwrap();

	event_loop.run(move |event, _, control_flow| match event {
		Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
		Event::WindowEvent { event: WindowEvent::Resized(size), window_id, .. } => {
			world
				.run(|mut cameras: ViewMut<Camera>, mut states: ViewMut<RenderWindowState>| {
					let window_entity = windows[&window_id];
					let aspect_ratio = size.width as f32 / size.height as f32;
					(&mut cameras).get(window_entity).unwrap().set_projection(aspect_ratio, 90.0, 0.1, 1000.0);
					(&mut states).get(window_entity).unwrap().resize();
				})
				.unwrap();
		},
		Event::RedrawEventsCleared => world.run_default().unwrap(),
		_ => (),
	});
}
