#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;

layout(location = 0) out vec3 v_normal;

// layout(set = 0, binding = 0) uniform Data {
// 	mat4 world;
// 	mat4 view;
// 	mat4 proj;
// } uniforms;

layout(push_constant) uniform PushConstant {
	vec3 camera_pos;
	vec4 camera_rot;
	vec4 camera_proj;
} pc;

vec4 quat_inv(vec4 quat) {
	return vec4(-quat.xyz, quat.w) / dot(quat, quat);
}
vec3 quat_mul(vec4 quat, vec3 vec) {
	return cross(quat.xyz, cross(quat.xyz, vec) + vec * quat.w) * 2.0 + vec;
}
vec4 perspective(vec4 proj, vec3 pos) {
	return vec4(pos.xy * proj.xy, -pos.z * proj.z + proj.w, pos.z);
}

void main() {
	vec3 position_ws = position;
	// vec3 position_ws = quat_mul(mesh_rot, position) + mesh_pos;
	vec3 position_cs = quat_mul(quat_inv(pc.camera_rot), position_ws - pc.camera_pos);
	v_normal = quat_mul(quat_inv(pc.camera_rot), normal);
	gl_Position = perspective(pc.camera_proj, position_cs);
	gl_Position.y = -gl_Position.y;

	// mat4 worldview = uniforms.view * uniforms.world;
	// v_normal = transpose(inverse(mat3(worldview))) * normal;
	// gl_Position = uniforms.proj * worldview * vec4(position, 1.0);
}
