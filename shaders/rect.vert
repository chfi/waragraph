#version 450

layout (location = 0) in vec2 position;
layout (location = 1) in uint i_color;

layout (location = 0) out vec4 o_color;

layout (set = 0, binding = 0) readonly buffer Colors {
  uint len;
  vec4 colors[];
} colors;

layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;

void main() {
  uint color_ix = uint(i_color);
  o_color = colors.colors[color_ix];

  vec2 pos = (2.0 * position / inputs.window_dims) - vec2(1.0);
  gl_Position = vec4(pos, 0.0, 1.0);
}
