use crate::gfx::render::TriangleVertex;
use async_std::{channel, channel::Sender, task::block_on};
use font_kit::{font::Font as KFont, hinting::HintingOptions, outline::OutlineSink};
use harfbuzz_rs::{shape, Face, Font as HFont, GlyphInfo, GlyphPosition, UnicodeBuffer};
use lazy_static::lazy_static;
use nalgebra::Vector2;
use pathfinder_geometry::{line_segment::LineSegment2F, vector::Vector2F};
use std::{collections::HashMap, sync::Arc, thread};
use unic_char_range::CharRange;
use unic_ucd_block::BlockIter;
use vulkano::{
	buffer::{cpu_pool::CpuBufferPoolChunk, BufferUsage, CpuBufferPool, ImmutableBuffer},
	command_buffer::{DrawIndexedIndirectCommand, DrawIndirectCommand},
	device::{Device, Queue},
	memory::pool::StdMemoryPool,
	sync::GpuFuture,
};

type LoadFontReturn = (BlockInfo, HashMap<u32, GlyphInfo2>, Box<dyn GpuFuture + Send + Sync>);
type LoadFontParams = (&'static str, Arc<Queue>, Sender<LoadFontReturn>);

lazy_static! {
	static ref BLOCKS: HashMap<&'static str, CharRange> =
		BlockIter::new().map(|block| (block.name, block.range)).collect();
	static ref LOAD_FONT: Sender<LoadFontParams> = {
		let (send, recv) = channel::unbounded::<LoadFontParams>();

		thread::spawn(move || {
			block_on(async {
				let font = KFont::from_path("res/Roboto-Regular.ttf", 0).unwrap();

				while let Ok((block, queue, send)) = recv.recv().await {
					let mut sink = TriangleBuilder::new();
					let mut glyph_info = HashMap::new();

					for ch in BLOCKS[block] {
						if let Some(glyph_id) = font.glyph_for_char(ch) {
							let index_start = sink.indices.len();
							let vert_start = sink.verts.len() - 2;
							let qvert_start = sink.qverts.len();
							font.outline(glyph_id, HintingOptions::None, &mut sink).unwrap();

							glyph_info.insert(glyph_id, GlyphInfo2 {
								index_count: (sink.indices.len() - index_start) as _,
								first_index: index_start as _,
								vertex_offset: vert_start as _,
								qvertex_count: (sink.qverts.len() - qvert_start) as _,
								qvertex_offset: qvert_start as _,
							});

							sink.reset();
						}
					}

					println!("tris {} qtris {}", sink.indices.len() / 3, sink.qverts.len() / 3);

					let (verts, verts_future) =
						ImmutableBuffer::from_iter(sink.verts.into_iter(), BufferUsage::vertex_buffer(), queue.clone())
							.unwrap();
					let (indices, indices_future) = ImmutableBuffer::from_iter(
						sink.indices.into_iter(),
						BufferUsage::index_buffer(),
						queue.clone(),
					)
					.unwrap();
					let (qverts, qverts_future) =
						ImmutableBuffer::from_iter(sink.qverts.into_iter(), BufferUsage::vertex_buffer(), queue)
							.unwrap();

					let block_info = BlockInfo { indices, verts, qverts };
					let future = Box::new(verts_future.join(indices_future).join(qverts_future));
					send.send((block_info, glyph_info, future)).await.unwrap();
				}
			})
		});

		send
	};
}

async fn load_font(block: &'static str, queue: &Arc<Queue>) -> LoadFontReturn {
	let (send, recv) = channel::bounded(1);
	LOAD_FONT.send((block, queue.clone(), send)).await.unwrap();
	recv.recv().await.unwrap()
}

pub struct Font {
	pub scale: f32,
	pub block_info: HashMap<&'static str, BlockInfo>,
	glyph_info: HashMap<u32, GlyphInfo2>,
	cmd_pool: CpuBufferPool<DrawIndexedIndirectCommand>,
	cmd_pool2: CpuBufferPool<DrawIndirectCommand>,
	instance_pool: CpuBufferPool<ChInstance>,
}
impl Font {
	pub fn new(device: &Arc<Device>, px_per_em: f32) -> Self {
		let kfont = KFont::from_path("res/Roboto-Regular.ttf", 0).unwrap();
		let scale = px_per_em * 2.0 / kfont.metrics().units_per_em as f32;

		Self {
			scale,
			block_info: HashMap::new(),
			glyph_info: HashMap::new(),
			cmd_pool: CpuBufferPool::indirect_buffer(device.clone()),
			cmd_pool2: CpuBufferPool::indirect_buffer(device.clone()),
			instance_pool: CpuBufferPool::vertex_buffer(device.clone()),
		}
	}

	pub fn load_block(&mut self, queue: &Arc<Queue>, block: &'static str) -> impl GpuFuture {
		let (block_info, glyph_info, future) = block_on(load_font(block, queue));
		self.block_info.insert(block, block_info);
		self.glyph_info.extend(glyph_info);
		future
	}

	pub fn draw(
		&self,
		text: &str,
	) -> (
		Arc<CpuBufferPoolChunk<DrawIndexedIndirectCommand, Arc<StdMemoryPool>>>,
		Arc<CpuBufferPoolChunk<DrawIndirectCommand, Arc<StdMemoryPool>>>,
		Arc<CpuBufferPoolChunk<ChInstance, Arc<StdMemoryPool>>>,
	) {
		let hfont = HFont::new(Face::from_file("res/Roboto-Regular.ttf", 0).unwrap());
		let buffer = UnicodeBuffer::new().add_str(text);
		let output = shape(&hfont, buffer, &[]);
		let positions = output.get_glyph_positions();
		let infos = output.get_glyph_infos();

		let cmds = (0..infos.len() * 6).map(|i| {
			let GlyphInfo { codepoint, .. } = infos[i / 6];
			let GlyphInfo2 { index_count, first_index, vertex_offset, .. } = *self.glyph_info.get(&codepoint).unwrap();
			DrawIndexedIndirectCommand {
				index_count,
				instance_count: 1,
				first_index,
				vertex_offset,
				first_instance: (i / 6) as _,
			}
		});
		let cmds = self.cmd_pool.chunk(cmds).unwrap();

		let qcmds = (0..infos.len() * 6).map(|i| {
			let GlyphInfo { codepoint, .. } = infos[i / 6];
			let GlyphInfo2 { qvertex_count, qvertex_offset, .. } = *self.glyph_info.get(&codepoint).unwrap();
			DrawIndirectCommand {
				vertex_count: qvertex_count,
				instance_count: 1,
				first_vertex: qvertex_offset,
				first_instance: (i / 6) as _,
			}
		});
		let qcmds = self.cmd_pool2.chunk(qcmds).unwrap();

		let mut cursor = Vector2::zeros();
		let instances = self
			.instance_pool
			.chunk(positions.iter().map(|pos| {
				let GlyphPosition { x_advance, y_advance, x_offset, y_offset, .. } = *pos;

				let position = cursor + Vector2::new(x_offset as _, y_offset as _);
				cursor += Vector2::new(x_advance as _, y_advance as _);

				print!("{}, ", position.x);
				assert_eq!(position.y, 0.0);

				ChInstance { ch_pos: position.into() }
			}))
			.unwrap();
		println!();

		(Arc::new(cmds), Arc::new(qcmds), Arc::new(instances))
	}
}

#[derive(Debug)]
struct TriangleBuilder {
	pen: Vector2F,
	starti: usize,
	offset: usize,
	verts: Vec<TriangleVertex>,
	indices: Vec<u16>,
	qoffset: usize,
	qverts: Vec<TriangleVertex>,
}
impl TriangleBuilder {
	fn new() -> Self {
		Self {
			pen: Vector2F::zero(),
			starti: 0,
			offset: 0,
			verts: vec![TriangleVertex { v_pos: [0.0, 0.0] }, TriangleVertex { v_pos: [0.0, 0.0] }],
			indices: vec![],
			qoffset: 0,
			qverts: vec![],
		}
	}

	fn reset(&mut self) {
		self.pen = Vector2F::zero();
		self.offset = self.verts.len();
		self.verts.push(TriangleVertex { v_pos: [0.0, 0.0] });
		self.verts.push(TriangleVertex { v_pos: [0.0, 0.0] });
	}
}
impl OutlineSink for TriangleBuilder {
	fn move_to(&mut self, to: Vector2F) {
		self.pen = to;
		let vert = TriangleVertex { v_pos: [to.x(), -to.y()] };
		if self.verts.len() == 2 {
			self.verts[0] = vert;
			self.verts[1] = vert;
		} else {
			self.starti = self.verts.len();
			self.verts.push(vert);
		}
	}

	fn line_to(&mut self, to: Vector2F) {
		let index = (self.verts.len() - self.offset) as u16;
		self.verts.push(TriangleVertex { v_pos: [to.x(), -to.y()] });
		self.indices.push(0);
		self.indices.push(index - 1);
		self.indices.push(index);
		self.pen = to;
	}

	fn quadratic_curve_to(&mut self, ctrl: Vector2F, to: Vector2F) {
		let index = (self.verts.len() - self.offset) as u16;
		self.verts.push(TriangleVertex { v_pos: [to.x(), -to.y()] });
		self.indices.push(0);
		self.indices.push(index - 1);
		self.indices.push(index);
		self.pen = to;

		let lasti = self.verts.len() - 1;
		self.qverts.push(self.verts[lasti]);
		self.qverts.push(self.verts[lasti - 1]);
		self.qverts.push(TriangleVertex { v_pos: [ctrl.x(), -ctrl.y()] });
	}

	fn cubic_curve_to(&mut self, ctrl: LineSegment2F, to: Vector2F) {
		let index = (self.verts.len() - self.offset) as u16;
		self.verts.push(TriangleVertex { v_pos: [to.x(), -to.y()] });
		self.indices.push(0);
		self.indices.push(index - 1);
		self.indices.push(index);
		self.pen = to;
	}

	fn close(&mut self) {
		self.indices.push(0);
		self.indices.push((self.verts.len() - self.offset - 1) as u16);
		self.indices.push((self.starti - self.offset) as _);
		let [x, y] = self.verts[self.starti].v_pos;
		self.pen = Vector2F::new(x, y);
	}
}

pub struct BlockInfo {
	pub verts: Arc<ImmutableBuffer<[TriangleVertex]>>,
	pub indices: Arc<ImmutableBuffer<[u16]>>,
	pub qverts: Arc<ImmutableBuffer<[TriangleVertex]>>,
}

struct GlyphInfo2 {
	index_count: u32,
	first_index: u32,
	vertex_offset: u32,
	qvertex_count: u32,
	qvertex_offset: u32,
}

#[derive(Default, Copy, Clone)]
pub struct ChInstance {
	ch_pos: [f32; 2],
}
vulkano::impl_vertex!(ChInstance, ch_pos);
