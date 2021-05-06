use crate::{gfx::DeviceCtx, Gfx};
use std::sync::Arc;
use vulkano::{
	image::{ImageUsage, SwapchainImage},
	swapchain::{ColorSpace, FullscreenExclusive, PresentMode, SurfaceTransform, Swapchain, SwapchainCreationError},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
	dpi::PhysicalSize,
	event_loop::EventLoop,
	window::{Window as WinitWindow, WindowBuilder, WindowId},
};

pub struct Window {
	device_ctx: Arc<DeviceCtx>,
	swapchain: Arc<Swapchain<WinitWindow>>,
	images: Vec<Arc<SwapchainImage<WinitWindow>>>,
}
impl Window {
	pub fn new(gfx: &mut Gfx, event_loop: &EventLoop<()>) -> Self {
		let surface = WindowBuilder::new().build_vk_surface(event_loop, gfx.instance().clone()).unwrap();

		let device_ctx = gfx.get_or_create_device(&surface).clone();
		let physical = device_ctx.physical_device();

		let (swapchain, images) = {
			let caps = surface.capabilities(physical).unwrap();
			let format = caps.supported_formats[0].0;
			let alpha = caps.supported_composite_alpha.iter().next().unwrap();

			Swapchain::start(device_ctx.device.clone(), surface.clone())
				.num_images(caps.min_image_count)
				.format(format)
				.dimensions(surface.window().inner_size().into())
				.usage(ImageUsage::color_attachment())
				.transform(SurfaceTransform::Identity)
				.composite_alpha(alpha)
				.present_mode(PresentMode::Fifo)
				.fullscreen_exclusive(FullscreenExclusive::Default)
				.build()
				.unwrap()
		};

		Self { device_ctx, swapchain, images }
	}

	pub fn recreate_swapchain(&mut self) {
		let dimensions: [u32; 2] = self.swapchain.surface().window().inner_size().into();
		let (new_swapchain, new_images) = match self.swapchain.recreate().dimensions(dimensions).build() {
			Ok(r) => r,
			Err(SwapchainCreationError::UnsupportedDimensions) => return,
			Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
		};

		self.swapchain = new_swapchain;
		self.images = new_images;
	}

	pub fn id(&self) -> WindowId {
		self.swapchain.surface().window().id()
	}

	pub fn inner_size(&self) -> PhysicalSize<u32> {
		self.swapchain.surface().window().inner_size()
	}

	pub fn device_ctx(&self) -> &Arc<DeviceCtx> {
		&self.device_ctx
	}

	pub fn swapchain(&self) -> &Arc<Swapchain<WinitWindow>> {
		&self.swapchain
	}

	pub fn images(&self) -> &Vec<Arc<SwapchainImage<WinitWindow>>> {
		&self.images
	}
}
