#version 450

layout (set = 0, binding = 0) uniform sampler2D text;

layout(location = 0) in vec2 f_pos;

layout(location = 0) out vec4 color;

void main() {
	vec2 uv = f_pos / 2 + 0.5;

	vec2 offset = vec2(abs(dFdx(uv.x)), 0);
	float rgbl = texture(text, uv + offset).b * 255;
	vec3 rgbc = texture(text, uv).rgb * 255;
	float rgbr = texture(text, uv - offset).r * 255;

	float rgblh = mod(floor(rgbl / 16), 2);
	float rgbll = mod(mod(rgbl, 16), 2);
	vec3 rgbch = mod(floor(rgbc / 16), 2);
	vec3 rgbcl = mod(mod(rgbc, 16), 2);
	float rgbrh = mod(floor(rgbr / 16), 2);
	float rgbrl = mod(mod(rgbr, 16), 2);

	float alphaL = (rgblh + rgbll) / 2;
	vec3 alphaC = (rgbch + rgbcl) / 2;
	float alphaR = (rgbrh + rgbrl) / 2;

	vec3 colors = vec3(
		(alphaC.y + alphaC.z + alphaR) / 3,
		(alphaC.x + alphaC.y + alphaC.z) / 3,
		(alphaL + alphaC.x + alphaC.y) / 3
	);
	// vec3 colors = (rgbch + rgbcl) / 2;
	color = vec4(colors, 0);
}
