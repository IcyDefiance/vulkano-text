use crate::{
	examples::{Normal, Vertex},
	gfx::{
		camera::Camera,
		model::Mesh,
		text::{ChInstance, Font},
		window::Window,
		Gfx, RenderPasses,
	},
};
use shipyard::{IntoIter, UniqueViewMut, View, ViewMut};
use std::{iter, sync::Arc};
use vulkano::{
	buffer::{cpu_pool::CpuBufferPoolChunk, BufferUsage, ImmutableBuffer, TypedBufferAccess},
	command_buffer::{
		AutoCommandBufferBuilder, CommandBufferUsage, DrawIndexedIndirectCommand, DrawIndirectCommand, DynamicState,
		SubpassContents,
	},
	descriptor::{
		descriptor_set::{PersistentDescriptorSet, UnsafeDescriptorSetLayout},
		DescriptorSet,
	},
	format::Format,
	image::{attachment::AttachmentImage, view::ImageView},
	memory::pool::StdMemoryPool,
	pipeline::{
		blend::{AttachmentBlend, BlendFactor, BlendOp},
		vertex::{OneVertexOneInstanceDefinition, TwoBuffersDefinition},
		viewport::Viewport,
		GraphicsPipeline, GraphicsPipelineAbstract,
	},
	render_pass::{Framebuffer, FramebufferAbstract, RenderPass, Subpass},
	sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
	swapchain,
	swapchain::AcquireError,
	sync,
	sync::{FlushError, GpuFuture},
};

pub struct RenderWindowState {
	font: Font,
	previous_frame_end: Option<Box<dyn GpuFuture + Send + Sync>>,
	text_framebuffer: Arc<dyn FramebufferAbstract + Send + Sync>,
	framebuffers: Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,
	pipeline_3d: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
	pipeline_text: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
	pipeline_textq: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
	pipeline_text2: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
	recreate_swapchain: bool,
	font_cmds: Arc<dyn TypedBufferAccess<Content = [DrawIndexedIndirectCommand]> + Send + Sync>,
	font_qcmds: Arc<dyn TypedBufferAccess<Content = [DrawIndirectCommand]> + Send + Sync>,
	font_instances: Arc<CpuBufferPoolChunk<ChInstance, Arc<StdMemoryPool>>>,
	triangle: Arc<ImmutableBuffer<[TriangleVertex]>>,
	text2_set: Arc<dyn DescriptorSet + Send + Sync>,
}
impl RenderWindowState {
	pub fn new(gfx: &mut Gfx, window: &Window) -> Self {
		let device_ctx = window.device_ctx();
		let device = device_ctx.device();

		let queue = device_ctx.queue();
		let render_pass = create_render_pass(gfx, window);

		let text_image = ImageView::new(
			AttachmentImage::sampled_input_attachment(
				device.clone(),
				window.images()[0].dimensions(),
				Format::R8G8B8A8Unorm,
			)
			.unwrap(),
		)
		.unwrap();

		let text_framebuffer = create_text_framebuffer(&render_pass.text, &text_image);
		let framebuffers = create_framebuffers(window, &render_pass.screen);
		let pipeline_3d = create_3d_pipeline(window, &render_pass.screen);
		let pipeline_text = create_text_pipeline(window, &render_pass.text);
		let pipeline_textq = create_textq_pipeline(window, &render_pass.text);
		let (pipeline_text2, pipeline_text2_layout) = create_text2_pipeline(window, &render_pass.screen);

		let mut font = Font::new(device, 16.0);
		let verts_future = font.load_block(queue, "Basic Latin");
		let (font_cmds, font_qcmds, font_instances) = font.draw("The quick brown fox jumps over the lazy dog.");

		let triangle: Vec<TriangleVertex> =
			vec![TriangleVertex { v_pos: [-1.0, -1.0] }, TriangleVertex { v_pos: [3.0, -1.0] }, TriangleVertex {
				v_pos: [-1.0, 3.0],
			}];
		let (triangle, triangle_future) =
			ImmutableBuffer::from_iter(triangle.into_iter(), BufferUsage::vertex_buffer(), queue.clone()).unwrap();

		let sampler = Sampler::new(
			device.clone(),
			Filter::Nearest,
			Filter::Nearest,
			MipmapMode::Nearest,
			SamplerAddressMode::Repeat,
			SamplerAddressMode::Repeat,
			SamplerAddressMode::Repeat,
			0.0,
			1.0,
			0.0,
			0.0,
		)
		.unwrap();
		let text2_set = Arc::new(
			PersistentDescriptorSet::start(pipeline_text2_layout)
				.add_sampled_image(text_image, sampler)
				.unwrap()
				.build()
				.unwrap(),
		);

		verts_future.join(triangle_future).then_signal_fence_and_flush().unwrap().wait(None).unwrap();

		Self {
			font,
			previous_frame_end: Some(Box::new(sync::now(device.clone()))),
			text_framebuffer,
			framebuffers,
			pipeline_3d,
			pipeline_text,
			pipeline_textq,
			pipeline_text2,
			recreate_swapchain: false,
			font_cmds,
			font_qcmds,
			font_instances,
			triangle,
			text2_set,
		}
	}

	pub fn resize(&mut self) {
		self.recreate_swapchain = true;
	}
}

pub fn render(
	mut gfx: UniqueViewMut<Gfx>,
	mut windows: ViewMut<Window>,
	mut states: ViewMut<RenderWindowState>,
	cameras: ViewMut<Camera>,
	meshes: View<Mesh>,
	strings: View<&'static str>,
) {
	for (mut window, mut state, camera) in (&mut windows, &mut states, &cameras).iter() {
		state.previous_frame_end.as_mut().unwrap().cleanup_finished();

		if state.recreate_swapchain && window.inner_size() != [0, 0].into() {
			window.recreate_swapchain();
			*state = RenderWindowState::new(&mut gfx, &window);
		}

		let device_ctx = window.device_ctx();
		let device = device_ctx.device();
		let queue = device_ctx.queue();

		let (image_num, suboptimal, acquire_future) =
			match swapchain::acquire_next_image(window.swapchain().clone(), None) {
				Ok(r) => r,
				Err(AcquireError::OutOfDate) => {
					state.recreate_swapchain = true;
					return;
				},
				Err(e) => panic!("Failed to acquire next image: {:?}", e),
			};

		if suboptimal {
			state.recreate_swapchain = true;
		}

		let mut builder =
			AutoCommandBufferBuilder::primary(device.clone(), queue.family(), CommandBufferUsage::OneTimeSubmit)
				.unwrap();
		builder.begin_render_pass(state.text_framebuffer.clone(), SubpassContents::Inline, vec![[0.0].into()]).unwrap();

		let pc = crate::gfx::vs_text::ty::PushConstant {
			pos: [-0.9, -0.8],
			target_size: [window.inner_size().width as f32, window.inner_size().height as f32],
			scale: state.font.scale,
		};
		let block_info = &state.font.block_info["Basic Latin"];
		for string in strings.iter() {
			let (font_cmds, font_qcmds, font_instances) = state.font.draw(string);
			builder
				.draw_indexed_indirect(
					state.pipeline_text.clone(),
					&DynamicState::none(),
					vec![block_info.verts.clone(), font_instances.clone()],
					block_info.indices.clone(),
					font_cmds.clone(),
					(),
					pc,
					vec![],
				)
				.unwrap()
				.draw_indirect(
					state.pipeline_textq.clone(),
					&DynamicState::none(),
					vec![block_info.qverts.clone(), font_instances.clone()],
					font_qcmds.clone(),
					(),
					pc,
					vec![],
				)
				.unwrap();
		}
		builder.end_render_pass().unwrap();

		builder
			.begin_render_pass(state.framebuffers[image_num].clone(), SubpassContents::Inline, vec![
				[0.0, 0.0, 0.0, 1.0].into(),
				1f32.into(),
			])
			.unwrap();
		for mesh in meshes.iter() {
			let pc = crate::gfx::vs_3d::ty::PushConstant {
				_dummy0: [0; 4],
				camera_pos: (*camera.position()).into(),
				camera_rot: (*camera.rotation().as_vector()).into(),
				camera_proj: (*camera.projection()).into(),
			};
			builder
				.draw_indexed(
					state.pipeline_3d.clone(),
					&DynamicState::none(),
					vec![mesh.vertices().clone(), mesh.normals().clone()],
					mesh.indices().clone(),
					(),
					pc,
					vec![],
				)
				.unwrap();
		}

		builder
			.draw(
				state.pipeline_text2.clone(),
				&DynamicState::none(),
				vec![state.triangle.clone()],
				state.text2_set.clone(),
				(),
				vec![],
			)
			.unwrap();

		builder.end_render_pass().unwrap();
		let command_buffer = builder.build().unwrap();

		let future = state
			.previous_frame_end
			.take()
			.unwrap()
			.join(acquire_future)
			.then_execute(queue.clone(), command_buffer)
			.unwrap()
			.then_swapchain_present(queue.clone(), window.swapchain().clone(), image_num)
			.then_signal_fence_and_flush();

		match future {
			Ok(future) => state.previous_frame_end = Some(Box::new(future)),
			Err(FlushError::OutOfDate) => {
				state.recreate_swapchain = true;
				state.previous_frame_end = Some(Box::new(sync::now(device.clone())));
			},
			Err(e) => {
				println!("Failed to flush future: {:?}", e);
				state.previous_frame_end = Some(Box::new(sync::now(device.clone())));
			},
		}
	}
}

fn create_render_pass<'a>(gfx: &'a mut Gfx, window: &Window) -> RenderPasses {
	gfx.get_or_create_render_pass(window.device_ctx().device(), window.swapchain().format()).clone()
}

fn create_text_framebuffer(
	render_pass: &Arc<RenderPass>,
	text_image: &Arc<ImageView<Arc<AttachmentImage>>>,
) -> Arc<dyn FramebufferAbstract + Send + Sync> {
	Arc::new(Framebuffer::start(render_pass.clone()).add(text_image.clone()).unwrap().build().unwrap())
}

fn create_framebuffers(
	window: &Window,
	render_pass: &Arc<RenderPass>,
) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
	let device = window.device_ctx().device();
	let dimensions = window.images()[0].dimensions();
	let depth_buffer =
		ImageView::new(AttachmentImage::transient(device.clone(), dimensions, Format::D16Unorm).unwrap()).unwrap();

	window
		.images()
		.iter()
		.map(|image| {
			let view = ImageView::new(image.clone()).unwrap();
			Arc::new(
				Framebuffer::start(render_pass.clone())
					.add(view)
					.unwrap()
					.add(depth_buffer.clone())
					.unwrap()
					.build()
					.unwrap(),
			) as Arc<dyn FramebufferAbstract + Send + Sync>
		})
		.collect::<Vec<_>>()
}

fn create_3d_pipeline(
	window: &Window,
	render_pass: &Arc<RenderPass>,
) -> Arc<dyn GraphicsPipelineAbstract + Send + Sync> {
	let device_ctx = window.device_ctx();
	let dimensions = window.images()[0].dimensions();

	Arc::new(
		GraphicsPipeline::start()
			.vertex_input(TwoBuffersDefinition::<Vertex, Normal>::new())
			.vertex_shader(device_ctx.vs_3d().main_entry_point(), ())
			.triangle_list()
			.viewports_dynamic_scissors_irrelevant(1)
			.viewports(iter::once(Viewport {
				origin: [0.0, 0.0],
				dimensions: [dimensions[0] as f32, dimensions[1] as f32],
				depth_range: 0.0..1.0,
			}))
			.fragment_shader(device_ctx.fs_3d().main_entry_point(), ())
			.depth_stencil_simple_depth()
			.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
			.build(device_ctx.device().clone())
			.unwrap(),
	)
}

const BLEND_ADD: AttachmentBlend = AttachmentBlend {
	enabled: true,
	color_op: BlendOp::Add,
	color_source: BlendFactor::One,
	color_destination: BlendFactor::One,
	alpha_op: BlendOp::Add,
	alpha_source: BlendFactor::One,
	alpha_destination: BlendFactor::One,
	mask_red: true,
	mask_green: true,
	mask_blue: true,
	mask_alpha: true,
};

fn create_text_pipeline(
	window: &Window,
	render_pass: &Arc<RenderPass>,
) -> Arc<dyn GraphicsPipelineAbstract + Send + Sync> {
	let device_ctx = window.device_ctx();
	let dimensions = window.images()[0].dimensions();

	Arc::new(
		GraphicsPipeline::start()
			.vertex_input(OneVertexOneInstanceDefinition::<TriangleVertex, ChInstance>::new())
			.vertex_shader(device_ctx.vs_text().main_entry_point(), ())
			.triangle_list()
			.viewports_dynamic_scissors_irrelevant(1)
			.viewports(iter::once(Viewport {
				origin: [0.0, 0.0],
				dimensions: [dimensions[0] as f32, dimensions[1] as f32],
				depth_range: 0.0..1.0,
			}))
			.fragment_shader(device_ctx.fs_text().main_entry_point(), ())
			.blend_collective(BLEND_ADD)
			.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
			.build(device_ctx.device().clone())
			.unwrap(),
	)
}

fn create_textq_pipeline(
	window: &Window,
	render_pass: &Arc<RenderPass>,
) -> Arc<dyn GraphicsPipelineAbstract + Send + Sync> {
	let device_ctx = window.device_ctx();
	let dimensions = window.images()[0].dimensions();

	Arc::new(
		GraphicsPipeline::start()
			.vertex_input(OneVertexOneInstanceDefinition::<TriangleVertex, ChInstance>::new())
			.vertex_shader(device_ctx.vs_text().main_entry_point(), ())
			.triangle_list()
			.viewports_dynamic_scissors_irrelevant(1)
			.viewports(iter::once(Viewport {
				origin: [0.0, 0.0],
				dimensions: [dimensions[0] as f32, dimensions[1] as f32],
				depth_range: 0.0..1.0,
			}))
			.fragment_shader(device_ctx.fs_textq().main_entry_point(), ())
			.blend_collective(BLEND_ADD)
			.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
			.build(device_ctx.device().clone())
			.unwrap(),
	)
}

fn create_text2_pipeline(
	window: &Window,
	render_pass: &Arc<RenderPass>,
) -> (Arc<dyn GraphicsPipelineAbstract + Send + Sync>, Arc<UnsafeDescriptorSetLayout>) {
	let device_ctx = window.device_ctx();
	let dimensions = window.images()[0].dimensions();

	let pipeline = Arc::new(
		GraphicsPipeline::start()
			.vertex_input_single_buffer::<TriangleVertex>()
			.vertex_shader(device_ctx.vs_sprite().main_entry_point(), ())
			.triangle_list()
			.viewports_dynamic_scissors_irrelevant(1)
			.viewports(iter::once(Viewport {
				origin: [0.0, 0.0],
				dimensions: [dimensions[0] as f32, dimensions[1] as f32],
				depth_range: 0.0..1.0,
			}))
			.fragment_shader(device_ctx.fs_text2().main_entry_point(), ())
			.blend_collective(BLEND_ADD)
			.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
			.build(device_ctx.device().clone())
			.unwrap(),
	);

	let layout = pipeline.layout().descriptor_set_layout(0).unwrap().clone();

	(pipeline, layout)
}

#[derive(Debug, Default, Copy, Clone)]
pub struct TriangleVertex {
	pub v_pos: [f32; 2],
}
vulkano::impl_vertex!(TriangleVertex, v_pos);
