pub mod camera;
pub mod model;
pub mod render;
pub mod text;
pub mod window;

use std::{collections::HashMap, sync::Arc};
use vulkano::{
	device::{Device, DeviceExtensions, Queue},
	format::Format,
	instance::{Instance, PhysicalDevice, QueueFamily},
	render_pass::RenderPass,
	swapchain::Surface,
	Version,
};
use winit::window::Window;

#[derive(Clone)]
pub struct RenderPasses {
	text: Arc<RenderPass>,
	screen: Arc<RenderPass>,
}
impl RenderPasses {
	fn new(device: &Arc<Device>, format: Format) -> Self {
		let text = Arc::new(
			vulkano::single_pass_renderpass!(device.clone(),
				attachments: {
					winding: { load: Clear, store: Store, format: Format::R8G8B8A8Unorm, samples: 1, }
				},
				pass: { color: [winding], depth_stencil: {} }
			)
			.unwrap(),
		);

		let screen = Arc::new(
			vulkano::single_pass_renderpass!(device.clone(),
				attachments: {
					color: { load: Clear, store: Store, format: format, samples: 1, },
					depth: { load: Clear, store: DontCare, format: Format::D16Unorm, samples: 1, }
				},
				pass: { color: [color], depth_stencil: {depth} }
			)
			.unwrap(),
		);

		Self { text, screen }
	}
}

pub struct Gfx {
	instance: Arc<Instance>,
	devices: Vec<Arc<DeviceCtx>>,
	render_passes: HashMap<Format, RenderPasses>,
}
impl Gfx {
	pub fn new() -> Self {
		let instance =
			Instance::new(None, Version::major_minor(1, 1), &vulkano_win::required_extensions(), None).unwrap();
		Self { instance, devices: vec![], render_passes: HashMap::new() }
	}

	pub fn instance(&self) -> &Arc<Instance> {
		&self.instance
	}

	pub fn get_or_create_render_pass(&mut self, device: &Arc<Device>, format: Format) -> &RenderPasses {
		self.render_passes.entry(format).or_insert_with(|| RenderPasses::new(device, format))
	}

	fn get_or_create_device(&mut self, surface: &Surface<Window>) -> &Arc<DeviceCtx> {
		let test_qfam = |q: QueueFamily| q.supports_graphics() && surface.is_supported(q).unwrap_or(false);

		// .enumerate() is a workaround for a lifetimes error
		if let Some((i, _)) = self.devices.iter().enumerate().filter(|(_, dq)| test_qfam(dq.queue.family())).next() {
			&self.devices[i]
		} else {
			let queue_family = PhysicalDevice::enumerate(&self.instance)
				.filter_map(|p| p.queue_families().find(|&q| test_qfam(q)))
				.next()
				.unwrap();
			let physical = queue_family.physical_device();

			let device_ext = DeviceExtensions { khr_swapchain: true, ..DeviceExtensions::none() };
			let (device, mut queues) = Device::new(
				physical,
				physical.supported_features(),
				&device_ext,
				[(queue_family, 0.5)].iter().cloned(),
			)
			.unwrap();
			let queue = queues.next().unwrap();

			let vs_3d = vs_3d::Shader::load(device.clone()).unwrap();
			let fs_3d = fs_3d::Shader::load(device.clone()).unwrap();
			let vs_text = vs_text::Shader::load(device.clone()).unwrap();
			let fs_text = fs_text::Shader::load(device.clone()).unwrap();
			let fs_textq = fs_textq::Shader::load(device.clone()).unwrap();
			let vs_sprite = vs_sprite::Shader::load(device.clone()).unwrap();
			let fs_text2 = fs_text2::Shader::load(device.clone()).unwrap();

			self.devices.push(Arc::new(DeviceCtx {
				device,
				queue,
				vs_3d,
				fs_3d,
				vs_text,
				fs_textq,
				fs_text,
				vs_sprite,
				fs_text2,
			}));

			self.devices.last().unwrap()
		}
	}
}

pub struct DeviceCtx {
	device: Arc<Device>,
	queue: Arc<Queue>,
	vs_3d: vs_3d::Shader,
	fs_3d: fs_3d::Shader,
	vs_text: vs_text::Shader,
	fs_text: fs_text::Shader,
	fs_textq: fs_textq::Shader,
	vs_sprite: vs_sprite::Shader,
	fs_text2: fs_text2::Shader,
}
impl DeviceCtx {
	pub fn device(&self) -> &Arc<Device> {
		&self.device
	}

	pub fn queue(&self) -> &Arc<Queue> {
		&self.queue
	}

	pub fn vs_3d(&self) -> &vs_3d::Shader {
		&self.vs_3d
	}

	pub fn fs_3d(&self) -> &fs_3d::Shader {
		&self.fs_3d
	}

	pub fn vs_text(&self) -> &vs_text::Shader {
		&self.vs_text
	}

	pub fn fs_text(&self) -> &fs_text::Shader {
		&self.fs_text
	}

	pub fn fs_textq(&self) -> &fs_textq::Shader {
		&self.fs_textq
	}

	pub fn vs_sprite(&self) -> &vs_sprite::Shader {
		&self.vs_sprite
	}

	pub fn fs_text2(&self) -> &fs_text2::Shader {
		&self.fs_text2
	}

	pub fn physical_device(&self) -> PhysicalDevice {
		self.device.physical_device()
	}
}

// pub struct Transform {
// 	pub position: Vector3<f32>,
// 	pub rotation: Quaternion<f32>,
// }

pub mod vs_3d {
	vulkano_shaders::shader! { ty: "vertex", path: "src/gfx/render/3d_vert.glsl" }
}
pub mod fs_3d {
	vulkano_shaders::shader! { ty: "fragment", path: "src/gfx/render/3d_frag.glsl" }
}
pub mod vs_text {
	vulkano_shaders::shader! { ty: "vertex", path: "src/gfx/render/text_vert.glsl" }
}
pub mod fs_text {
	vulkano_shaders::shader! { ty: "fragment", path: "src/gfx/render/text_frag.glsl" }
}
pub mod fs_textq {
	vulkano_shaders::shader! { ty: "fragment", path: "src/gfx/render/textq_frag.glsl" }
}
pub mod vs_sprite {
	vulkano_shaders::shader! { ty: "vertex", path: "src/gfx/render/sprite_vert.glsl" }
}
pub mod fs_text2 {
	vulkano_shaders::shader! { ty: "fragment", path: "src/gfx/render/text2_frag.glsl" }
}
