#version 460

layout(location = 0) in vec2 v_pos;
layout(location = 1) in vec2 ch_pos;

layout(location = 0) out vec2 uv;
layout(location = 1) out vec3 f_color;

layout(push_constant) uniform PushConstant {
	vec2 pos;
	vec2 target_size;
	float scale;
} pc;

void main() {
	float u = mod(gl_VertexIndex + 2, 3.0) / 2;
	uv = vec2(u, floor(u));

	float samplex = mod(gl_DrawID, 3);
	float sampley = floor(mod(gl_DrawID, 6) / 3);

	f_color = vec3(0);
	f_color[int(samplex)] = 1.0 / 255 * (sampley * 15 + 1);

	vec2 offset = vec2((samplex - 1) / 2, (sampley - 0.5) * 2 / 3);
	gl_Position = vec4(((v_pos + ch_pos) * pc.scale + offset) / pc.target_size + pc.pos, 0, 1);
}
