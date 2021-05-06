use crate::{
	examples::{Normal, Vertex},
	gfx::window::Window,
};
use std::sync::Arc;
use vulkano::{
	buffer::{BufferUsage, ImmutableBuffer},
	sync::GpuFuture,
};

// pub struct Model {
// 	pub meshes: Vec<Mesh>,
// 	pub transform: Transform,
// }
// impl Model {
// 	pub fn new(meshes: Vec<Mesh>, transform: Transform) -> Self {
// 		Self { meshes, transform }
// 	}
// }

pub struct Mesh {
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	normals: Arc<ImmutableBuffer<[Normal]>>,
	indices: Arc<ImmutableBuffer<[u16]>>,
}
impl Mesh {
	pub fn new(
		window: &Window,
		vertices: impl ExactSizeIterator<Item = Vertex>,
		normals: impl ExactSizeIterator<Item = Normal>,
		indices: impl ExactSizeIterator<Item = u16>,
	) -> (Self, impl GpuFuture) {
		let queue = &window.device_ctx().queue;
		let usage = BufferUsage { vertex_buffer: true, ..BufferUsage::none() };
		let (vertices, vertices_future) = ImmutableBuffer::from_iter(vertices, usage, queue.clone()).unwrap();
		let (normals, normals_future) = ImmutableBuffer::from_iter(normals, usage, queue.clone()).unwrap();
		let usage = BufferUsage { index_buffer: true, ..BufferUsage::none() };
		let (indices, indices_future) = ImmutableBuffer::from_iter(indices, usage, queue.clone()).unwrap();

		(Self { vertices, normals, indices }, vertices_future.join(normals_future).join(indices_future))
	}

	pub fn vertices(&self) -> &Arc<ImmutableBuffer<[Vertex]>> {
		&self.vertices
	}

	pub fn normals(&self) -> &Arc<ImmutableBuffer<[Normal]>> {
		&self.normals
	}

	pub fn indices(&self) -> &Arc<ImmutableBuffer<[u16]>> {
		&self.indices
	}
}
