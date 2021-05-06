#version 450

layout(location = 0) in vec2 uv;
layout(location = 1) in vec3 f_color;

layout(location = 0) out vec4 color;

void main() {
	if (uv.x * uv.x - uv.y > 0) discard;
	color = vec4(f_color, 1);
}
