#version 450

layout (location = 0) in vec2 i_uv;

// kinda hacky but good enough for now
layout (location = 1) in flat uint i_buffer_len;


layout (location = 0) out vec4 f_color;


layout (set = 0, binding = 0) readonly buffer PathData {
  uint val[];
} path;

layout (set = 1, binding = 0) readonly buffer Colors {
  uint len;
  vec4 colors[];
} colors;


layout (push_constant) uniform Input {
  vec2 window_dims;
} inputs;


void main() {
  uint ix = uint(floor(i_uv.x * i_buffer_len));
  uint val = path.val[ix];

  vec4 color = colors.colors[val % colors.len];

  f_color = color;
}
