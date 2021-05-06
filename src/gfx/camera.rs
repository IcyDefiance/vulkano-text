use nalgebra::{Quaternion, Vector3, Vector4};
use std::f32::consts::PI;

pub struct Camera {
	position: Vector3<f32>,
	rotation: Quaternion<f32>,
	projection: Vector4<f32>,
}
impl Camera {
	pub fn new(
		position: Vector3<f32>,
		rotation: Quaternion<f32>,
		aspect: f32,
		fovx: f32,
		znear: f32,
		zfar: f32,
	) -> Self {
		Self { position, rotation, projection: projection(aspect, fovx, znear, zfar) }
	}

	pub fn set_projection(&mut self, aspect: f32, fovx: f32, znear: f32, zfar: f32) {
		self.projection = projection(aspect, fovx, znear, zfar)
	}

	pub fn position(&self) -> &Vector3<f32> {
		&self.position
	}

	pub fn rotation(&self) -> &Quaternion<f32> {
		&self.rotation
	}

	pub fn projection(&self) -> &Vector4<f32> {
		&self.projection
	}
}

fn projection(aspect: f32, fovx: f32, znear: f32, zfar: f32) -> Vector4<f32> {
	let f = 1.0 / (fovx * (PI / 360.0)).tan();
	Vector4::new(f / aspect, f, (zfar + znear) / (znear - zfar), 2.0 * zfar * znear / (znear - zfar))
}
