#version 450

layout(location = 0) in vec2 v_pos;

layout(location = 0) out vec2 f_pos;

void main() {
	f_pos = v_pos;
	gl_Position = vec4(v_pos, 0, 1);
}
