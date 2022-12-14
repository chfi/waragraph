#version 450

layout (location = 0) in vec2 i_uv;
layout (location = 0) out vec4 f_color;

layout (set = 1, binding = 0) readonly buffer DataBuf {
  uint len;
  uint values[];
} data;

layout (set = 1, binding = 1) readonly buffer Colors {
  uint len;
  vec4 colors[];
} colors;


void main() {
  uint data_ix = uint(i_uv.x * float(data.len - 1));
  uint val = data.values[data_ix];
  vec4 color = colors.colors[val % colors.len];
  f_color = color;
}
